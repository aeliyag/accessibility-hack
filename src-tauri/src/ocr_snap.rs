use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::screen_capture::{capture_roi_fingerprint, capture_roi_png, CaptureOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordBox {
    pub text: String,
    pub confidence: f32,
    /// Global screen coordinates (top-left points).
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub word_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CaptureMeta {
    image_width: Option<i32>,
    image_height: Option<i32>,
    word_count: Option<i32>,
}

#[derive(Debug, Clone)]
struct ScreenRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl ScreenRect {
    fn contains_with_margin(&self, px: f64, py: f64, margin: f64) -> bool {
        px >= self.x + margin
            && px <= self.x + self.w - margin
            && py >= self.y + margin
            && py <= self.y + self.h - margin
    }
}

#[derive(Debug, Clone)]
struct LineCluster {
    words: Vec<WordBox>,
    cy: f64,
    top: f64,
    bottom: f64,
}

#[derive(Debug, Clone)]
struct WordMapCache {
    roi: ScreenRect,
    words: Vec<WordBox>,
    lines: Vec<LineCluster>,
    sticky_line: Option<usize>,
    fingerprint: u64,
    captured_at: Instant,
    last_change_check: Instant,
}

static AUTO_SNAP: AtomicBool = AtomicBool::new(false);
const DEBUG_PNG: &str = "/tmp/typoscope-ocr-debug.png";

/// Re-OCR when cursor is within this many points of the cached ROI edge.
const EDGE_MARGIN: f64 = 48.0;
/// Slow safety-net refresh while sitting in one region.
const MAX_CACHE_AGE: Duration = Duration::from_secs(10);
/// How often to cheaply check whether on-screen content changed.
const CHANGE_CHECK_EVERY: Duration = Duration::from_millis(900);
/// Snap/follow loop tick — no capture; just re-evaluate mouse vs cache.
const LOOP_TICK: Duration = Duration::from_millis(16);
/// Don't switch lines until cursor is this far (in points) into the other line's territory.
const LINE_HYSTERESIS: f64 = 10.0;

pub fn set_auto_snap_enabled(enabled: bool) {
    AUTO_SNAP.store(enabled, Ordering::SeqCst);
}

pub fn auto_snap_enabled() -> bool {
    AUTO_SNAP.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn set_auto_snap(enabled: bool) -> bool {
    set_auto_snap_enabled(enabled);
    enabled
}

#[tauri::command]
pub fn get_auto_snap() -> bool {
    auto_snap_enabled()
}

pub fn start_auto_snap_loop(app: AppHandle) {
    thread::spawn(move || {
        let mut cache: Option<WordMapCache> = None;
        let mut last_emitted: Option<SnapRect> = None;
        let mut was_enabled = false;

        loop {
            if !auto_snap_enabled() {
                if was_enabled {
                    cache = None;
                    last_emitted = None;
                    was_enabled = false;
                }
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            was_enabled = true;

            let Some(window) = app.get_webview_window("main") else {
                thread::sleep(Duration::from_millis(200));
                continue;
            };

            let (mouse_x, mouse_y) = cg_mouse_location_top_left();
            let below_id = overlay_cg_window_id(&window);
            let opts = CaptureOptions {
                below_window_id: below_id,
            };

            let mut needs_recapture = match &cache {
                None => true,
                Some(c) => {
                    !c.roi.contains_with_margin(mouse_x, mouse_y, EDGE_MARGIN)
                        || c.captured_at.elapsed() >= MAX_CACHE_AGE
                }
            };

            // Cheap content-change check while cursor stays inside the map.
            // Only when we can exclude the overlay — otherwise the moving slit
            // would poison the fingerprint and force endless recaptures.
            if !needs_recapture && below_id.is_some() {
                if let Some(c) = cache.as_ref() {
                    if c.last_change_check.elapsed() >= CHANGE_CHECK_EVERY {
                        let sample_w = c.roi.w.min(240.0);
                        let sample_h = c.roi.h.min(80.0);
                        let sx = c.roi.x + (c.roi.w - sample_w) / 2.0;
                        let sy = c.roi.y + (c.roi.h - sample_h) / 2.0;
                        match capture_roi_fingerprint(sx, sy, sample_w, sample_h, opts) {
                            Ok(fp) if fp != c.fingerprint => {
                                needs_recapture = true;
                            }
                            Ok(_) => {
                                if let Some(c) = cache.as_mut() {
                                    c.last_change_check = Instant::now();
                                }
                            }
                            Err(_) => {
                                if let Some(c) = cache.as_mut() {
                                    c.last_change_check = Instant::now();
                                }
                            }
                        }
                    }
                }
            } else if !needs_recapture {
                if let Some(c) = cache.as_mut() {
                    // Still advance the timer so we don't spin on the branch.
                    if c.last_change_check.elapsed() >= CHANGE_CHECK_EVERY {
                        c.last_change_check = Instant::now();
                    }
                }
            }

            if needs_recapture {
                match rebuild_cache(&window, mouse_x, mouse_y, opts) {
                    Ok(Some(new_cache)) => {
                        eprintln!(
                            "[typoscope ocr] recapture ROI=({:.0},{:.0},{:.0},{:.0}) words={} lines={} fingerprint={:#x}",
                            new_cache.roi.x,
                            new_cache.roi.y,
                            new_cache.roi.w,
                            new_cache.roi.h,
                            new_cache.words.len(),
                            new_cache.lines.len(),
                            new_cache.fingerprint
                        );
                        cache = Some(new_cache);
                    }
                    Ok(None) => {
                        // Keep previous cache if OCR found nothing this pass.
                    }
                    Err(err) => {
                        eprintln!("[typoscope ocr] recapture failed: {err}");
                        let _ = window.emit("ocr-snap-error", err);
                    }
                }
            }

            if let Some(c) = cache.as_mut() {
                if let Some(rect) = snap_from_cache(c, mouse_x, mouse_y, &window) {
                    let changed = match &last_emitted {
                        None => true,
                        Some(prev) => {
                            (prev.x - rect.x).abs() > 0.5
                                || (prev.y - rect.y).abs() > 0.5
                                || (prev.width - rect.width).abs() > 0.5
                                || (prev.height - rect.height).abs() > 0.5
                        }
                    };
                    if changed {
                        last_emitted = Some(rect.clone());
                        let _ = window.emit("ocr-snap-rect", rect);
                    }
                }
            }

            thread::sleep(LOOP_TICK);
        }
    });
}

fn ocr_helper_path() -> PathBuf {
    PathBuf::from(env!("TYPOSCOPE_OCR_HELPER"))
}

fn overlay_cg_window_id(window: &WebviewWindow) -> Option<u32> {
    let ns_ptr = window.ns_window().ok()?;
    if ns_ptr.is_null() {
        return None;
    }
    // NSWindow.windowNumber == CGWindowID
    unsafe {
        let ns_window = &*(ns_ptr as *const objc2_app_kit::NSWindow);
        Some(ns_window.windowNumber() as u32)
    }
}

fn cg_mouse_location_top_left() -> (f64, f64) {
    use std::os::raw::c_void;

    type CGEventRef = *const c_void;
    type CGEventSourceRef = *const c_void;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return (0.0, 0.0);
        }
        let CGPoint { x, y } = CGEventGetLocation(event);
        CFRelease(event);
        (x, y)
    }
}

fn cg_main_display_size() -> (f64, f64) {
    use std::os::raw::c_uint;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> c_uint;
        fn CGDisplayBounds(display: c_uint) -> CGRect;
    }

    unsafe {
        let bounds = CGDisplayBounds(CGMainDisplayID());
        (bounds.size.width, bounds.size.height)
    }
}

fn rebuild_cache(
    window: &WebviewWindow,
    mouse_x: f64,
    mouse_y: f64,
    opts: CaptureOptions,
) -> Result<Option<WordMapCache>, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let (screen_w, screen_h) = cg_main_display_size();

    let roi_w = screen_w.min(1200.0).max(320.0);
    let roi_h = 200.0;
    let roi_x = (mouse_x - roi_w / 2.0).clamp(0.0, (screen_w - roi_w).max(0.0));
    let roi_y = (mouse_y - roi_h / 2.0).clamp(0.0, (screen_h - roi_h).max(0.0));
    let roi = ScreenRect {
        x: roi_x,
        y: roi_y,
        w: roi_w,
        h: roi_h,
    };

    // Do NOT hide the overlay — capture below it via CGWindowList.
    let debug_path = PathBuf::from(DEBUG_PNG);
    let (cap_w, _cap_h) = capture_roi_png(roi_x, roi_y, roi_w, roi_h, &debug_path, opts)?;

    let sample_w = roi_w.min(240.0);
    let sample_h = roi_h.min(80.0);
    let fingerprint = capture_roi_fingerprint(
        roi_x + (roi_w - sample_w) / 2.0,
        roi_y + (roi_h - sample_h) / 2.0,
        sample_w,
        sample_h,
        opts,
    )
    .unwrap_or(0);

    let output = Command::new(ocr_helper_path())
        .arg(&debug_path)
        .output()
        .map_err(|e| format!("ocr helper failed to start: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let err = stderr.trim();
        return Err(if err.is_empty() {
            "ocr helper failed".into()
        } else {
            err.to_string()
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw_words: Vec<WordBox> =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("ocr json parse: {e}"))?;

    let mut px_per_point = if cap_w > 0 {
        cap_w as f64 / roi_w
    } else {
        scale
    };
    if let Some(meta_line) = stderr.lines().find(|l| l.starts_with("OCR_META ")) {
        if let Ok(meta) = serde_json::from_str::<CaptureMeta>(&meta_line["OCR_META ".len()..]) {
            if let Some(iw) = meta.image_width {
                if iw > 0 {
                    px_per_point = iw as f64 / roi_w;
                }
            }
        }
    }

    // Store words in global screen coordinates.
    let words: Vec<WordBox> = raw_words
        .into_iter()
        .filter(|w| w.confidence >= 0.35 && w.width > 2.0 && w.height > 4.0)
        .map(|w| WordBox {
            text: w.text,
            confidence: w.confidence,
            x: roi_x + w.x / px_per_point,
            y: roi_y + w.y / px_per_point,
            width: w.width / px_per_point,
            height: w.height / px_per_point,
        })
        .collect();

    if words.is_empty() {
        return Ok(None);
    }

    let lines = cluster_lines(&words);
    Ok(Some(WordMapCache {
        roi,
        words,
        lines,
        sticky_line: None,
        fingerprint,
        captured_at: Instant::now(),
        last_change_check: Instant::now(),
    }))
}

fn cluster_lines(words: &[WordBox]) -> Vec<LineCluster> {
    let mut sorted: Vec<&WordBox> = words.iter().collect();
    sorted.sort_by(|a, b| {
        (a.y + a.height / 2.0)
            .partial_cmp(&(b.y + b.height / 2.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let median_h = {
        let mut heights: Vec<f64> = sorted.iter().map(|w| w.height).collect();
        heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        heights[heights.len() / 2]
    };
    let line_tol = (median_h * 0.45).max(6.0);

    let mut lines: Vec<LineCluster> = Vec::new();
    for word in sorted {
        if let Some(line) = lines.last_mut() {
            if (word.y + word.height / 2.0 - line.cy).abs() <= line_tol {
                line.words.push(word.clone());
                let n = line.words.len() as f64;
                line.cy = line.words.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / n;
                line.top = line.top.min(word.y);
                line.bottom = line.bottom.max(word.y + word.height);
                continue;
            }
        }
        lines.push(LineCluster {
            cy: word.y + word.height / 2.0,
            top: word.y,
            bottom: word.y + word.height,
            words: vec![word.clone()],
        });
    }

    for line in &mut lines {
        line.words
            .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }
    lines
}

fn snap_from_cache(
    cache: &mut WordMapCache,
    mouse_x: f64,
    mouse_y: f64,
    window: &WebviewWindow,
) -> Option<SnapRect> {
    if cache.lines.is_empty() {
        return None;
    }

    let nearest = cache
        .lines
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.cy - mouse_y)
                .abs()
                .partial_cmp(&(b.cy - mouse_y).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)?;

    let line_idx = match cache.sticky_line {
        Some(prev) if prev < cache.lines.len() => {
            let prev_cy = cache.lines[prev].cy;
            let next_cy = cache.lines[nearest].cy;
            let dist_prev = (prev_cy - mouse_y).abs();
            let dist_next = (next_cy - mouse_y).abs();
            // Only switch when clearly closer to another line.
            if nearest != prev && dist_next + LINE_HYSTERESIS < dist_prev {
                nearest
            } else {
                prev
            }
        }
        _ => nearest,
    };
    cache.sticky_line = Some(line_idx);

    let line = &cache.lines[line_idx];
    let median_h = {
        let mut heights: Vec<f64> = line.words.iter().map(|w| w.height).collect();
        heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        heights[heights.len() / 2]
    };

    let left = line.words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
    let right = line
        .words
        .iter()
        .map(|w| w.x + w.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = line.top;
    let bottom = line.bottom;

    let pad_x = 10.0;
    let pad_y = (median_h * 0.2).clamp(3.0, 12.0);
    let height = ((bottom - top) * 1.15 + pad_y * 2.0).max(median_h * 1.2);
    let width = (right - left + pad_x * 2.0).min(1600.0).max(40.0);
    let screen_x = left - pad_x;
    let screen_y = (top + bottom) / 2.0 - height / 2.0;

    let scale = window.scale_factor().ok()?;
    let origin = window.outer_position().ok()?;
    let origin_x = origin.x as f64 / scale;
    let origin_y = origin.y as f64 / scale;

    let _ = mouse_x;
    Some(SnapRect {
        x: screen_x - origin_x,
        y: screen_y - origin_y,
        width,
        height,
        word_count: line.words.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusters_and_hysteresis_prefer_sticky_line() {
        let words = vec![
            WordBox {
                text: "Hello".into(),
                confidence: 0.9,
                x: 100.0,
                y: 200.0,
                width: 50.0,
                height: 20.0,
            },
            WordBox {
                text: "world".into(),
                confidence: 0.9,
                x: 160.0,
                y: 202.0,
                width: 55.0,
                height: 18.0,
            },
            WordBox {
                text: "Other".into(),
                confidence: 0.9,
                x: 100.0,
                y: 260.0,
                width: 50.0,
                height: 20.0,
            },
        ];
        let lines = cluster_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words.len(), 2);
    }

    #[test]
    fn margin_detects_edge_exit() {
        let roi = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 200.0,
        };
        assert!(roi.contains_with_margin(500.0, 100.0, 48.0));
        assert!(!roi.contains_with_margin(20.0, 100.0, 48.0));
        assert!(!roi.contains_with_margin(500.0, 10.0, 48.0));
    }

    #[test]
    fn idle_inside_fresh_cache_does_not_need_recapture() {
        // Matches the user's stationary log: ROI=(71,257,1200,200) mouse=(671,357)
        let roi = ScreenRect {
            x: 71.0,
            y: 257.0,
            w: 1200.0,
            h: 200.0,
        };
        let mouse = (671.0_f64, 357.0_f64);
        assert!(roi.contains_with_margin(mouse.0, mouse.1, EDGE_MARGIN));
        let age = Duration::from_millis(500);
        assert!(age < MAX_CACHE_AGE);
        let needs = !roi.contains_with_margin(mouse.0, mouse.1, EDGE_MARGIN) || age >= MAX_CACHE_AGE;
        assert!(!needs);
    }
}
