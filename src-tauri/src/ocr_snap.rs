use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::screen_capture::{capture_roi_fingerprint, capture_roi_png, CaptureOptions};
use crate::snap_units::{
    cluster_lines, cursor_near_unit, select_unit, select_unit_with_margin, LineCluster, SnapMode,
    UnitRect, WordBox, UNIT_NEAR_MARGIN,
};

#[derive(Debug, Clone, Serialize)]
pub struct SnapRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub word_count: usize,
    /// "cache" | "empty"
    pub source: &'static str,
    /// Hot-path latency: mouse sample → emit (ms). OCR time is excluded.
    pub latency_ms: f64,
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
struct WordMapCache {
    roi: ScreenRect,
    words: Vec<WordBox>,
    lines: Vec<LineCluster>,
    sticky_line: Option<usize>,
    /// Sticky highlight for word/sentence/paragraph modes.
    sticky_unit: Option<UnitRect>,
    fingerprint: u64,
    captured_at: Instant,
    last_change_check: Instant,
    /// Mouse position when this cache was captured (for move-triggered rescan).
    capture_mouse: (f64, f64),
    /// Which ROI tier produced this cache (0=small …).
    tier: usize,
}

struct OcrJob {
    mouse_x: f64,
    mouse_y: f64,
    scale: f64,
    opts: CaptureOptions,
}

struct OcrResult {
    cache: Option<WordMapCache>,
    error: Option<String>,
    elapsed_ms: f64,
}

static AUTO_SNAP: AtomicBool = AtomicBool::new(false);
/// 0=line 1=word 2=sentence 3=paragraph
static SNAP_MODE: AtomicU8 = AtomicU8::new(0);
const DEBUG_PNG: &str = "/tmp/typoscope-ocr-debug.png";

/// Expanding OCR scan tiers (width × height), centered on the cursor.
/// Start small; expand only when no usable unit is near the cursor.
const ROI_TIERS: [(f64, f64); 3] = [
    (360.0, 140.0),  // small
    (700.0, 200.0),  // medium
    (1100.0, 280.0), // large
];

/// Stable empty-state box at the cursor when no unit is near.
/// All modes use a fixed box so auto-snap ON is visually distinct from free slit.
const EMPTY_BOX_WORD: (f64, f64) = (120.0, 32.0);
const EMPTY_BOX_LINE: (f64, f64) = (560.0, 44.0);
const EMPTY_BOX_SENTENCE: (f64, f64) = (320.0, 42.0);
const EMPTY_BOX_PARAGRAPH: (f64, f64) = (460.0, 120.0);

const EDGE_MARGIN: f64 = 28.0;
const MAX_CACHE_AGE: Duration = Duration::from_secs(4);
const CHANGE_CHECK_EVERY: Duration = Duration::from_millis(350);
const LOOP_TICK: Duration = Duration::from_millis(8);
/// Rescan when the cursor has moved this far from the capture mouse position.
const MOVE_RESCAN_PX: f64 = 64.0;
/// While over blank / no near unit, retry OCR at least this often (escalate map).
const EMPTY_RESCAN_EVERY: Duration = Duration::from_millis(280);
/// Stick to a line only while the cursor is still inside its band (± this).
const LINE_BAND_PAD: f64 = 6.0;
/// Tiny pad so hysteresis only kills neighbor jitter — never holds after leave.
const UNIT_STICK_PAD: f64 = 4.0;
/// Leave mapped lines → async recapture when cursor is this far outside all bands.
const LINE_LEAVE_MARGIN: f64 = 28.0;
/// Extra horizontal slack for line-mode "am I over this line's text?"
const LINE_X_PAD: f64 = 36.0;

fn unit_contains_cursor(unit: &UnitRect, mx: f64, my: f64, pad: f64) -> bool {
    cursor_near_unit(unit, mx, my, pad)
}

fn empty_box_size(mode: SnapMode) -> Option<(f64, f64)> {
    match mode {
        SnapMode::Word => Some(EMPTY_BOX_WORD),
        SnapMode::Line => Some(EMPTY_BOX_LINE),
        SnapMode::Sentence => Some(EMPTY_BOX_SENTENCE),
        SnapMode::Paragraph => Some(EMPTY_BOX_PARAGRAPH),
    }
}

fn line_near_cursor(line: &LineCluster, mx: f64, my: f64) -> bool {
    if my < line.top - LINE_BAND_PAD || my > line.bottom + LINE_BAND_PAD {
        return false;
    }
    let left = line
        .words
        .iter()
        .map(|w| w.x)
        .fold(f64::INFINITY, f64::min);
    let right = line
        .words
        .iter()
        .map(|w| w.x + w.width)
        .fold(f64::NEG_INFINITY, f64::max);
    mx >= left - LINE_X_PAD && mx <= right + LINE_X_PAD
}

fn snap_mode_from_u8(v: u8) -> SnapMode {
    match v {
        1 => SnapMode::Word,
        2 => SnapMode::Sentence,
        3 => SnapMode::Paragraph,
        _ => SnapMode::Line,
    }
}

fn snap_mode_to_u8(mode: SnapMode) -> u8 {
    match mode {
        SnapMode::Line => 0,
        SnapMode::Word => 1,
        SnapMode::Sentence => 2,
        SnapMode::Paragraph => 3,
    }
}

pub fn set_auto_snap_enabled(enabled: bool) {
    AUTO_SNAP.store(enabled, Ordering::SeqCst);
}

pub fn auto_snap_enabled() -> bool {
    AUTO_SNAP.load(Ordering::SeqCst)
}

pub fn current_snap_mode() -> SnapMode {
    snap_mode_from_u8(SNAP_MODE.load(Ordering::SeqCst))
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

#[tauri::command]
pub fn set_snap_mode(mode: String) -> String {
    let parsed = SnapMode::from_str_lossy(&mode);
    SNAP_MODE.store(snap_mode_to_u8(parsed), Ordering::SeqCst);
    parsed.as_str().to_string()
}

#[tauri::command]
pub fn get_snap_mode() -> String {
    current_snap_mode().as_str().to_string()
}

pub fn start_auto_snap_loop(app: AppHandle) {
    let (job_tx, job_rx): (Sender<OcrJob>, Receiver<OcrJob>) = mpsc::channel();
    let (res_tx, res_rx): (Sender<OcrResult>, Receiver<OcrResult>) = mpsc::channel();

    // OCR never runs on the mouse/snap hot path.
    thread::spawn(move || {
        while let Ok(job) = job_rx.recv() {
            let started = Instant::now();
            let outcome = rebuild_cache(job.mouse_x, job.mouse_y, job.scale, job.opts);
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let result = match outcome {
                Ok(cache) => OcrResult {
                    cache,
                    error: None,
                    elapsed_ms,
                },
                Err(error) => OcrResult {
                    cache: None,
                    error: Some(error),
                    elapsed_ms,
                },
            };
            if res_tx.send(result).is_err() {
                break;
            }
        }
    });

    thread::spawn(move || {
        let mut cache: Option<WordMapCache> = None;
        let mut last_emitted: Option<SnapRect> = None;
        let mut was_enabled = false;
        let mut ocr_inflight = false;
        let mut latency_samples: Vec<f64> = Vec::new();
        let mut last_empty_rescan = Instant::now()
            .checked_sub(EMPTY_RESCAN_EVERY)
            .unwrap_or_else(Instant::now);
        let mut last_mode = current_snap_mode();

        loop {
            if !auto_snap_enabled() {
                if was_enabled {
                    cache = None;
                    last_emitted = None;
                    ocr_inflight = false;
                    was_enabled = false;
                    // Drain any stale OCR result.
                    while res_rx.try_recv().is_ok() {}
                }
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            was_enabled = true;

            let tick_start = Instant::now();
            let Some(window) = app.get_webview_window("main") else {
                thread::sleep(Duration::from_millis(200));
                continue;
            };

            let mode = current_snap_mode();
            if mode != last_mode {
                if let Some(c) = cache.as_mut() {
                    c.sticky_line = None;
                    c.sticky_unit = None;
                }
                last_mode = mode;
            }

            // Apply finished OCR without blocking.
            match res_rx.try_recv() {
                Ok(result) => {
                    ocr_inflight = false;
                    if let Some(err) = result.error {
                        eprintln!(
                            "[typoscope ocr] async recapture failed ({:.0}ms): {err}",
                            result.elapsed_ms
                        );
                        let _ = window.emit("ocr-snap-error", err);
                    } else if let Some(new_cache) = result.cache {
                        eprintln!(
                            "[typoscope ocr] tier={} ROI=({:.0},{:.0},{:.0},{:.0}) words={} lines={} ocr={:.0}ms fingerprint={:#x}",
                            new_cache.tier,
                            new_cache.roi.x,
                            new_cache.roi.y,
                            new_cache.roi.w,
                            new_cache.roi.h,
                            new_cache.words.len(),
                            new_cache.lines.len(),
                            result.elapsed_ms,
                            new_cache.fingerprint
                        );
                        cache = Some(new_cache);
                    } else {
                        // Explicit empty map — clear stale cache so empty-box / free-slit wins.
                        cache = None;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }

            let (mouse_x, mouse_y) = cg_mouse_location_top_left();
            let below_id = overlay_cg_window_id(&window);
            let opts = CaptureOptions {
                below_window_id: below_id,
            };
            let scale = window.scale_factor().unwrap_or(2.0);

            let outside_roi = cache
                .as_ref()
                .map(|c| !c.roi.contains_with_margin(mouse_x, mouse_y, EDGE_MARGIN))
                .unwrap_or(true);
            let outside_lines = cache
                .as_ref()
                .map(|c| !cursor_near_mapped_lines(c, mouse_y, LINE_LEAVE_MARGIN))
                .unwrap_or(true);
            let aged_out = cache
                .as_ref()
                .map(|c| c.captured_at.elapsed() >= MAX_CACHE_AGE)
                .unwrap_or(false);
            let moved_far = cache
                .as_ref()
                .map(|c| {
                    let (cx, cy) = c.capture_mouse;
                    ((cx - mouse_x).powi(2) + (cy - mouse_y).powi(2)).sqrt() >= MOVE_RESCAN_PX
                })
                .unwrap_or(false);
            let no_near_unit = cache
                .as_ref()
                .map(|c| select_unit(&c.lines, mode, mouse_x, mouse_y).is_none())
                .unwrap_or(true);
            let empty_rescan_due = no_near_unit && last_empty_rescan.elapsed() >= EMPTY_RESCAN_EVERY;

            let mut needs_recapture =
                cache.is_none() || outside_roi || outside_lines || aged_out || moved_far || empty_rescan_due;

            if !needs_recapture && below_id.is_some() {
                if let Some(c) = cache.as_mut() {
                    if c.last_change_check.elapsed() >= CHANGE_CHECK_EVERY {
                        let sample_w = c.roi.w.min(240.0);
                        let sample_h = c.roi.h.min(80.0);
                        let sx = c.roi.x + (c.roi.w - sample_w) / 2.0;
                        let sy = c.roi.y + (c.roi.h - sample_h) / 2.0;
                        match capture_roi_fingerprint(sx, sy, sample_w, sample_h, opts) {
                            Ok(fp) if fp != c.fingerprint => needs_recapture = true,
                            _ => {}
                        }
                        c.last_change_check = Instant::now();
                    }
                }
            }

            if needs_recapture && !ocr_inflight {
                ocr_inflight = true;
                if empty_rescan_due {
                    last_empty_rescan = Instant::now();
                }
                let _ = job_tx.send(OcrJob {
                    mouse_x,
                    mouse_y,
                    scale,
                    opts,
                });
            }

            let snapped = cache
                .as_mut()
                .and_then(|c| snap_from_cache(c, mouse_x, mouse_y, &window, tick_start));

            let rect = match snapped {
                Some(r) => Some(r),
                None => empty_box_at_cursor(mode, mouse_x, mouse_y, &window, tick_start),
            };

            if let Some(rect) = rect {
                latency_samples.push(rect.latency_ms);
                if latency_samples.len() >= 60 {
                    let avg =
                        latency_samples.iter().sum::<f64>() / latency_samples.len() as f64;
                    let max = latency_samples
                        .iter()
                        .cloned()
                        .fold(0.0_f64, f64::max);
                    eprintln!(
                        "[typoscope ocr] hot-path latency avg={avg:.2}ms max={max:.2}ms (mouse→emit, OCR excluded)"
                    );
                    latency_samples.clear();
                }

                let changed = match &last_emitted {
                    None => true,
                    Some(prev) => {
                        (prev.x - rect.x).abs() > 0.25
                            || (prev.y - rect.y).abs() > 0.25
                            || (prev.width - rect.width).abs() > 0.5
                            || (prev.height - rect.height).abs() > 0.5
                            || prev.source != rect.source
                    }
                };
                if changed {
                    last_emitted = Some(rect.clone());
                    eprintln!(
                        "[typoscope ocr] emit source={} mode={} {:.0}x{:.0} words={} @({:.0},{:.0})",
                        rect.source,
                        mode.as_str(),
                        rect.width,
                        rect.height,
                        rect.word_count,
                        rect.x,
                        rect.y
                    );
                    let _ = window.emit("ocr-snap-rect", rect);
                }
            } else if last_emitted.is_some() {
                // Line mode + blank: release to free mouse-following slit.
                last_emitted = None;
                let _ = window.emit("ocr-snap-clear", ());
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

fn window_origin_logical(window: &WebviewWindow) -> Option<(f64, f64)> {
    let scale = window.scale_factor().ok()?;
    let origin = window.outer_position().ok()?;
    Some((origin.x as f64 / scale, origin.y as f64 / scale))
}

fn cursor_near_mapped_lines(cache: &WordMapCache, mouse_y: f64, margin: f64) -> bool {
    cache.lines.iter().any(|line| {
        mouse_y >= line.top - margin && mouse_y <= line.bottom + margin
    })
}

fn empty_box_at_cursor(
    mode: SnapMode,
    mouse_x: f64,
    mouse_y: f64,
    window: &WebviewWindow,
    tick_start: Instant,
) -> Option<SnapRect> {
    let (width, height) = empty_box_size(mode)?;
    let (origin_x, origin_y) = window_origin_logical(window)?;
    Some(SnapRect {
        x: (mouse_x - width / 2.0) - origin_x,
        y: (mouse_y - height / 2.0) - origin_y,
        width,
        height,
        word_count: 0,
        source: "empty",
        latency_ms: tick_start.elapsed().as_secs_f64() * 1000.0,
    })
}

fn centered_roi(mouse_x: f64, mouse_y: f64, roi_w: f64, roi_h: f64) -> ScreenRect {
    let (screen_w, screen_h) = cg_main_display_size();
    let w = roi_w.min(screen_w).max(80.0);
    let h = roi_h.min(screen_h).max(40.0);
    let x = (mouse_x - w / 2.0).clamp(0.0, (screen_w - w).max(0.0));
    let y = (mouse_y - h / 2.0).clamp(0.0, (screen_h - h).max(0.0));
    ScreenRect { x, y, w, h }
}

fn scan_roi_tier(
    mouse_x: f64,
    mouse_y: f64,
    roi: ScreenRect,
    scale: f64,
    opts: CaptureOptions,
    tier: usize,
) -> Result<Option<WordMapCache>, String> {
    let debug_path = PathBuf::from(format!("{DEBUG_PNG}.tier{tier}"));
    let (cap_w, _cap_h) = capture_roi_png(roi.x, roi.y, roi.w, roi.h, &debug_path, opts)?;

    let sample_w = roi.w.min(240.0);
    let sample_h = roi.h.min(80.0);
    let fingerprint = capture_roi_fingerprint(
        roi.x + (roi.w - sample_w) / 2.0,
        roi.y + (roi.h - sample_h) / 2.0,
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
        cap_w as f64 / roi.w
    } else {
        scale
    };
    if let Some(meta_line) = stderr.lines().find(|l| l.starts_with("OCR_META ")) {
        if let Ok(meta) = serde_json::from_str::<CaptureMeta>(&meta_line["OCR_META ".len()..]) {
            if let Some(iw) = meta.image_width {
                if iw > 0 {
                    px_per_point = iw as f64 / roi.w;
                }
            }
        }
    }

    let words: Vec<WordBox> = raw_words
        .into_iter()
        .filter(|w| w.confidence >= 0.35 && w.width > 2.0 && w.height > 4.0)
        .map(|w| WordBox {
            text: w.text,
            confidence: w.confidence,
            x: roi.x + w.x / px_per_point,
            y: roi.y + w.y / px_per_point,
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
        sticky_unit: None,
        fingerprint,
        captured_at: Instant::now(),
        last_change_check: Instant::now(),
        capture_mouse: (mouse_x, mouse_y),
        tier,
    }))
}

/// Expand-until-found: scan small → medium → large around the cursor; stop early
/// when a snap unit for the current mode is under/near the cursor.
fn rebuild_cache(
    mouse_x: f64,
    mouse_y: f64,
    scale: f64,
    opts: CaptureOptions,
) -> Result<Option<WordMapCache>, String> {
    let mode = current_snap_mode();
    let mut last_any: Option<WordMapCache> = None;
    let mut last_err: Option<String> = None;

    for (tier, &(w, h)) in ROI_TIERS.iter().enumerate() {
        let roi = centered_roi(mouse_x, mouse_y, w, h);
        match scan_roi_tier(mouse_x, mouse_y, roi, scale, opts, tier) {
            Ok(Some(cache)) => {
                // Soft hit: stop expanding once a unit is reasonably near the cursor.
                let hit = select_unit_with_margin(
                    &cache.lines,
                    mode,
                    mouse_x,
                    mouse_y,
                    UNIT_NEAR_MARGIN * 2.0,
                )
                .is_some();
                if hit {
                    return Ok(Some(cache));
                }
                last_any = Some(cache);
            }
            Ok(None) => {
                // No words in this tier — expand.
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    if let Some(cache) = last_any {
        return Ok(Some(cache));
    }
    if let Some(err) = last_err {
        return Err(err);
    }
    Ok(None)
}

fn snap_from_cache(
    cache: &mut WordMapCache,
    mouse_x: f64,
    mouse_y: f64,
    window: &WebviewWindow,
    tick_start: Instant,
) -> Option<SnapRect> {
    if cache.lines.is_empty() {
        return None;
    }

    let mode = current_snap_mode();

    let unit = if mode == SnapMode::Line {
        cache.sticky_unit = None;
        // Prefer sticky line while cursor remains on it; else nearest line under cursor.
        let line_idx = match cache.sticky_line {
            Some(prev)
                if prev < cache.lines.len()
                    && line_near_cursor(&cache.lines[prev], mouse_x, mouse_y) =>
            {
                prev
            }
            _ => cache
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line_near_cursor(line, mouse_x, mouse_y))
                .min_by(|(_, a), (_, b)| {
                    (a.cy - mouse_y)
                        .abs()
                        .partial_cmp(&(b.cy - mouse_y).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)?,
        };
        cache.sticky_line = Some(line_idx);
        // Already verified proximity via line_near_cursor — don't re-reject with a tight AABB.
        select_unit_with_margin(
            &cache.lines,
            mode,
            mouse_x,
            cache.lines[line_idx].cy,
            10_000.0,
        )?
    } else {
        cache.sticky_line = None;
        // Keep sticky only while cursor is still on/very near it (anti-jitter pad).
        if let Some(prev) = cache.sticky_unit.clone() {
            if unit_contains_cursor(&prev, mouse_x, mouse_y, UNIT_STICK_PAD) {
                prev
            } else {
                match select_unit(&cache.lines, mode, mouse_x, mouse_y) {
                    Some(next) => {
                        cache.sticky_unit = Some(next.clone());
                        next
                    }
                    None => {
                        cache.sticky_unit = None;
                        return None;
                    }
                }
            }
        } else {
            match select_unit(&cache.lines, mode, mouse_x, mouse_y) {
                Some(next) => {
                    cache.sticky_unit = Some(next.clone());
                    next
                }
                None => {
                    cache.sticky_unit = None;
                    return None;
                }
            }
        }
    };

    let (origin_x, origin_y) = window_origin_logical(window)?;

    Some(SnapRect {
        x: unit.x - origin_x,
        y: unit.y - origin_y,
        width: unit.width,
        height: unit.height,
        word_count: unit.word_count,
        source: "cache",
        latency_ms: tick_start.elapsed().as_secs_f64() * 1000.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_inside_fresh_cache_does_not_need_recapture() {
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

    #[test]
    fn leaving_mapped_lines_triggers_recapture_condition() {
        let words = vec![WordBox {
            text: "Hello".into(),
            confidence: 0.9,
            x: 100.0,
            y: 200.0,
            width: 50.0,
            height: 20.0,
        }];
        let cache = WordMapCache {
            roi: ScreenRect {
                x: 0.0,
                y: 0.0,
                w: 1200.0,
                h: 400.0,
            },
            lines: cluster_lines(&words),
            words,
            sticky_line: None,
            sticky_unit: None,
            fingerprint: 0,
            captured_at: Instant::now(),
            last_change_check: Instant::now(),
            capture_mouse: (125.0, 210.0),
            tier: 0,
        };
        assert!(cursor_near_mapped_lines(&cache, 210.0, LINE_LEAVE_MARGIN));
        assert!(!cursor_near_mapped_lines(&cache, 320.0, LINE_LEAVE_MARGIN));
    }

    #[test]
    fn empty_box_sizes_by_mode() {
        assert_eq!(empty_box_size(SnapMode::Word), Some(EMPTY_BOX_WORD));
        assert_eq!(empty_box_size(SnapMode::Line), Some(EMPTY_BOX_LINE));
        assert_eq!(empty_box_size(SnapMode::Sentence), Some(EMPTY_BOX_SENTENCE));
        assert_eq!(empty_box_size(SnapMode::Paragraph), Some(EMPTY_BOX_PARAGRAPH));
    }

    #[test]
    fn line_near_cursor_requires_xy_overlap() {
        let words = vec![WordBox {
            text: "Hello".into(),
            confidence: 0.9,
            x: 100.0,
            y: 200.0,
            width: 50.0,
            height: 20.0,
        }];
        let lines = cluster_lines(&words);
        assert!(line_near_cursor(&lines[0], 120.0, 210.0));
        assert!(!line_near_cursor(&lines[0], 400.0, 210.0));
        assert!(!line_near_cursor(&lines[0], 120.0, 300.0));
    }

    #[test]
    fn roi_tiers_expand_outward() {
        assert!(ROI_TIERS[0].0 < ROI_TIERS[1].0);
        assert!(ROI_TIERS[1].0 < ROI_TIERS[2].0);
        assert!(ROI_TIERS[0].1 < ROI_TIERS[1].1);
    }
}
