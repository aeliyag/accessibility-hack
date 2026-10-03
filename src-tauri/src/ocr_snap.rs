use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::screen_capture::{capture_roi_fingerprint, capture_roi_png, CaptureOptions};
use crate::snap_units::{cluster_lines, dist_to_rect, select_unit, LineCluster, SnapMode, WordBox};

#[derive(Debug, Clone, Serialize)]
pub struct SnapRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub bands: Vec<(f64, f64, f64, f64)>,
    pub word_count: usize,
    pub mode: &'static str,
    pub source: &'static str,
    pub latency_ms: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureMeta {
    image_width: u32,
    image_height: u32,
}

/// Global screen points, origin top-left. Never mix these with Retina image pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ScreenRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl ScreenRect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.w && y <= self.y + self.h
    }

    fn around(self, x: f64, y: f64, width: f64, height: f64) -> Self {
        let w = width.min(self.w);
        let h = height.min(self.h);
        Self {
            x: (x - w / 2.0).clamp(self.x, self.x + self.w - w).floor(),
            y: (y - h / 2.0).clamp(self.y, self.y + self.h - h).floor(),
            w,
            h,
        }
    }
}

struct WordMapCache {
    roi: ScreenRect,
    lines: Vec<LineCluster>,
    fingerprint: Option<u64>,
    fingerprint_roi: ScreenRect,
    captured_at: Instant,
    last_change_check: Instant,
}

struct OcrJob {
    mouse: (f64, f64),
    display: ScreenRect,
    opts: CaptureOptions,
    mode: SnapMode,
    generation: u64,
}
struct OcrResult {
    outcome: Result<Option<WordMapCache>, String>,
    generation: u64,
    mouse: (f64, f64),
    elapsed_ms: f64,
}

static AUTO_SNAP: AtomicBool = AtomicBool::new(false);
static SNAP_MODE: AtomicU8 = AtomicU8::new(0);
const CHANGE_CHECK_EVERY: Duration = Duration::from_millis(450);
const MAX_CACHE_AGE: Duration = Duration::from_secs(3);
const REFRESH_GRACE: Duration = Duration::from_secs(2);
const RETRY_EVERY: Duration = Duration::from_millis(250);
const MIN_SCAN_INTERVAL: Duration = Duration::from_millis(100);
const REACQUIRE_DISTANCE: f64 = 32.0;
const LOOP_TICK: Duration = Duration::from_millis(12);

pub fn auto_snap_enabled() -> bool {
    AUTO_SNAP.load(Ordering::SeqCst)
}

pub fn current_snap_mode() -> SnapMode {
    match SNAP_MODE.load(Ordering::SeqCst) {
        1 => SnapMode::Word,
        2 => SnapMode::Sentence,
        3 => SnapMode::Paragraph,
        _ => SnapMode::Line,
    }
}

#[tauri::command]
pub fn set_auto_snap(enabled: bool) -> bool {
    AUTO_SNAP.store(enabled, Ordering::SeqCst);
    enabled
}
#[tauri::command]
pub fn get_auto_snap() -> bool {
    auto_snap_enabled()
}
#[tauri::command]
pub fn set_snap_mode(mode: String) -> String {
    let mode = SnapMode::from_str_lossy(&mode);
    // Paragraph selection is deferred from the MVP, including old clients.
    let mode = if mode == SnapMode::Paragraph {
        SnapMode::Word
    } else {
        mode
    };
    SNAP_MODE.store(
        match mode {
            SnapMode::Line => 0,
            SnapMode::Word => 1,
            SnapMode::Sentence => 2,
            SnapMode::Paragraph => 3,
        },
        Ordering::SeqCst,
    );
    mode.as_str().to_owned()
}
#[tauri::command]
pub fn get_snap_mode() -> String {
    current_snap_mode().as_str().to_owned()
}

pub fn start_auto_snap_loop(app: AppHandle) {
    let (jobs, work) = mpsc::channel::<OcrJob>();
    let (results, ready) = mpsc::channel::<OcrResult>();
    thread::spawn(move || {
        while let Ok(job) = work.recv() {
            let started = Instant::now();
            let generation = job.generation;
            let mouse = job.mouse;
            let outcome = rebuild_cache(job);
            if results
                .send(OcrResult {
                    outcome,
                    generation,
                    mouse,
                    elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
                })
                .is_err()
            {
                break;
            }
        }
    });

    thread::spawn(move || {
        let mut cache: Option<WordMapCache> = None;
        let mut inflight = None;
        let mut generation = 0u64;
        let mut was_enabled = false;
        let mut last_mode = current_snap_mode();
        let mut last_attempt = Instant::now() - RETRY_EVERY;
        let mut last_completed = Instant::now() - RETRY_EVERY;
        let mut last_scan_mouse = None;
        let mut refresh_reason = None;
        let mut missed_refreshes = 0;
        let mut last_emitted: Option<SnapRect> = None;

        loop {
            let enabled = auto_snap_enabled();
            let mode = current_snap_mode();
            if enabled != was_enabled || mode != last_mode {
                generation += 1;
                cache = None;
                inflight = None;
                last_emitted = None;
                last_attempt = Instant::now() - RETRY_EVERY;
                last_completed = Instant::now() - RETRY_EVERY;
                last_scan_mouse = None;
                refresh_reason = None;
                missed_refreshes = 0;
                was_enabled = enabled;
                last_mode = mode;
                let _ = app.emit("ocr-snap-clear", ());
            }
            if !enabled {
                while ready.try_recv().is_ok() {}
                thread::sleep(Duration::from_millis(80));
                continue;
            }
            let started = Instant::now();
            let Some(window) = app.get_webview_window("main") else {
                break;
            };
            let mouse = cg_mouse_location_top_left();
            let display = match display_bounds(&window) {
                Some(display) => display,
                None => {
                    thread::sleep(RETRY_EVERY);
                    continue;
                }
            };
            let opts = CaptureOptions {
                below_window_id: overlay_cg_window_id(&window),
            };

            while let Ok(result) = ready.try_recv() {
                // A result captured for a previous toggle/mode must not restore stale words.
                if result.generation != generation {
                    continue;
                }
                inflight = None;
                last_completed = Instant::now();
                match result.outcome {
                    Ok(new_cache) => {
                        let hit = new_cache
                            .as_ref()
                            .and_then(|c| select_unit(&c.lines, mode, mouse.0, mouse.1))
                            .is_some();
                        let nearest = new_cache.as_ref().map(|c| nearest_word_distance(c, mouse));
                        eprintln!(
                            "[typoscope ocr] result mode={} words={} request=({:.0},{:.0}) cursor=({:.0},{:.0}) hit={} nearest={:?}pt OCR={:.0}ms",
                            mode.as_str(),
                            new_cache.as_ref().map(|c| c.lines.iter().map(|l| l.words.len()).sum::<usize>()).unwrap_or(0),
                            result.mouse.0, result.mouse.1, mouse.0, mouse.1, hit, nearest, result.elapsed_ms
                        );
                        if apply_refresh(&mut cache, new_cache, mode, mouse, &mut missed_refreshes)
                        {
                            refresh_reason = Some("verify-miss");
                            eprintln!("[typoscope ocr] retaining current selection for one confirmation scan");
                        }
                    }
                    Err(error) => {
                        if apply_refresh(&mut cache, None, mode, mouse, &mut missed_refreshes) {
                            refresh_reason = Some("verify-miss");
                        }
                        eprintln!("[typoscope ocr] {error}");
                        let _ = window.emit("ocr-snap-error", error);
                    }
                }
            }

            // Check changes even while a word is selected: scrolling moves its geometry.
            let mut changed = false;
            if let Some(c) = cache.as_mut() {
                if inflight.is_none() && c.last_change_check.elapsed() >= CHANGE_CHECK_EVERY {
                    let probe = c.fingerprint_roi;
                    match fingerprint(probe, opts) {
                        Ok(fp) => changed = c.fingerprint.map(|old| old != fp).unwrap_or(true),
                        Err(error) => {
                            changed = true;
                            let _ = window.emit("ocr-snap-error", error);
                        }
                    }
                    c.last_change_check = Instant::now();
                }
            }
            if changed {
                // Refresh in the background; do not flash the empty box while
                // an otherwise valid selection is waiting for OCR.
                refresh_reason = Some("content-change");
                last_completed = Instant::now() - RETRY_EVERY;
            }
            let reason = refresh_reason.or_else(|| scan_reason(cache.as_ref(), mode, mouse));
            if display.contains(mouse.0, mouse.1)
                && inflight.is_none()
                && scan_due(
                    reason,
                    mouse,
                    last_scan_mouse,
                    last_attempt.elapsed(),
                    last_completed.elapsed(),
                )
            {
                eprintln!(
                    "[typoscope ocr] scan reason={} mode={} cursor=({:.0},{:.0})",
                    reason.unwrap_or("unknown"),
                    mode.as_str(),
                    mouse.0,
                    mouse.1
                );
                inflight = Some(generation);
                refresh_reason = None;
                last_attempt = Instant::now();
                last_scan_mouse = Some(mouse);
                if jobs
                    .send(OcrJob {
                        mouse,
                        display,
                        opts,
                        mode,
                        generation,
                    })
                    .is_err()
                {
                    break;
                }
            }

            let origin = window_origin_logical(&window).unwrap_or((display.x, display.y));
            let unit = cache.as_ref().and_then(|c| {
                // Never hold a failed/slow recapture's positions indefinitely.
                if c.captured_at.elapsed() > MAX_CACHE_AGE + REFRESH_GRACE {
                    return None;
                }
                select_unit(&c.lines, mode, mouse.0, mouse.1)
            });
            let rect = if let Some(unit) = unit {
                SnapRect {
                    x: unit.x - origin.0,
                    y: unit.y - origin.1,
                    width: unit.width,
                    height: unit.height,
                    bands: unit
                        .bands
                        .iter()
                        .map(|&(x, y, w, h)| (x - origin.0, y - origin.1, w, h))
                        .collect(),
                    word_count: unit.word_count,
                    mode: mode.as_str(),
                    source: "cache",
                    latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                }
            } else {
                let (width, height) = match mode {
                    SnapMode::Word => (120.0, 32.0),
                    SnapMode::Line => (560.0, 44.0),
                    SnapMode::Sentence => (320.0, 42.0),
                    SnapMode::Paragraph => (460.0, 120.0),
                };
                SnapRect {
                    x: mouse.0 - width / 2.0 - origin.0,
                    y: mouse.1 - height / 2.0 - origin.1,
                    width,
                    height,
                    bands: vec![],
                    word_count: 0,
                    mode: mode.as_str(),
                    source: "empty",
                    latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                }
            };
            if last_emitted
                .as_ref()
                .map(|p| rect_changed(p, &rect))
                .unwrap_or(true)
            {
                let _ = window.emit("ocr-snap-rect", &rect);
                last_emitted = Some(rect);
            }
            thread::sleep(LOOP_TICK);
        }
    });
}

/// A cached image covering the pointer is not proof that OCR mapped its text.
/// Missing words must request a new scan even inside a fresh, unchanged ROI.
fn scan_reason(
    cache: Option<&WordMapCache>,
    mode: SnapMode,
    mouse: (f64, f64),
) -> Option<&'static str> {
    let Some(cache) = cache else {
        return Some("no-cache");
    };
    if !cache.roi.contains(mouse.0, mouse.1) {
        return Some("outside-roi");
    }
    if select_unit(&cache.lines, mode, mouse.0, mouse.1).is_none() {
        return Some("cursor-miss");
    }
    if cache.captured_at.elapsed() >= MAX_CACHE_AGE {
        return Some("expired");
    }
    None
}

fn scan_due(
    reason: Option<&str>,
    mouse: (f64, f64),
    last_mouse: Option<(f64, f64)>,
    since_started: Duration,
    since_completed: Duration,
) -> bool {
    if reason.is_none() || since_started < MIN_SCAN_INTERVAL {
        return false;
    }
    // Follow the latest pointer immediately after an old-position scan finishes.
    // At rest over true blank space, keep a bounded retry cadence instead.
    let moved = last_mouse
        .map(|(x, y)| (mouse.0 - x).hypot(mouse.1 - y) >= REACQUIRE_DISTANCE)
        .unwrap_or(true);
    moved
        || matches!(reason, Some("outside-roi" | "expired" | "content-change"))
        || since_completed >= RETRY_EVERY
}

/// Swap valid OCR maps atomically. One transient miss must not cancel a selected
/// word, but two misses (or an expired grace period) remove stale geometry.
fn apply_refresh(
    cache: &mut Option<WordMapCache>,
    next: Option<WordMapCache>,
    mode: SnapMode,
    mouse: (f64, f64),
    misses: &mut u8,
) -> bool {
    let old_matches = cache
        .as_ref()
        .map(|c| {
            c.captured_at.elapsed() <= MAX_CACHE_AGE + REFRESH_GRACE
                && select_unit(&c.lines, mode, mouse.0, mouse.1).is_some()
        })
        .unwrap_or(false);
    let next_matches = next
        .as_ref()
        .and_then(|c| select_unit(&c.lines, mode, mouse.0, mouse.1))
        .is_some();
    if old_matches && !next_matches && *misses == 0 {
        *misses = 1;
        true
    } else {
        *cache = next;
        *misses = 0;
        false
    }
}

fn nearest_word_distance(cache: &WordMapCache, mouse: (f64, f64)) -> f64 {
    cache
        .lines
        .iter()
        .flat_map(|l| &l.words)
        .map(|w| dist_to_rect(mouse.0, mouse.1, w.x, w.y, w.width, w.height))
        .fold(f64::INFINITY, f64::min)
}

fn rect_changed(a: &SnapRect, b: &SnapRect) -> bool {
    (a.x - b.x).abs() > 0.25
        || (a.y - b.y).abs() > 0.25
        || (a.width - b.width).abs() > 0.5
        || (a.height - b.height).abs() > 0.5
        || a.bands != b.bands
        || a.source != b.source
        || a.mode != b.mode
}

fn overlay_cg_window_id(window: &WebviewWindow) -> Option<u32> {
    let ptr = window.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    let id = unsafe { (&*(ptr as *const objc2_app_kit::NSWindow)).windowNumber() };
    (id > 0).then_some(id as u32)
}

fn display_bounds(window: &WebviewWindow) -> Option<ScreenRect> {
    let monitor = window.primary_monitor().ok()??;
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let pos = monitor.position();
    if scale <= 0.0 || size.width == 0 || size.height == 0 {
        return None;
    }
    Some(ScreenRect {
        x: pos.x as f64 / scale,
        y: pos.y as f64 / scale,
        w: size.width as f64 / scale,
        h: size.height as f64 / scale,
    })
}

fn window_origin_logical(window: &WebviewWindow) -> Option<(f64, f64)> {
    let scale = window.scale_factor().ok()?;
    let origin = window.outer_position().ok()?;
    Some((origin.x as f64 / scale, origin.y as f64 / scale))
}

fn cg_mouse_location_top_left() -> (f64, f64) {
    use std::ffi::c_void;
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: *const c_void);
    }
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return (0.0, 0.0);
        }
        let p = CGEventGetLocation(event);
        CFRelease(event);
        (p.x, p.y)
    }
}

fn fingerprint_region(roi: ScreenRect, mouse: (f64, f64)) -> ScreenRect {
    // A clock or animated advert elsewhere on the display must not cancel a
    // sentence. Check the text around the scan anchor; the cache TTL handles
    // changes elsewhere, and pointer misses immediately acquire fresh text.
    roi.around(mouse.0, mouse.1, 480.0, 180.0)
}

fn fingerprint(roi: ScreenRect, opts: CaptureOptions) -> Result<u64, String> {
    capture_roi_fingerprint(roi.x, roi.y, roi.w, roi.h, opts)
}

fn map_image_words(
    words: Vec<WordBox>,
    roi: ScreenRect,
    pixels: (u32, u32),
) -> Result<Vec<WordBox>, String> {
    if pixels.0 == 0 || pixels.1 == 0 || roi.w <= 0.0 || roi.h <= 0.0 {
        return Err("OCR capture has invalid dimensions".into());
    }
    let sx = roi.w / pixels.0 as f64;
    let sy = roi.h / pixels.1 as f64;
    Ok(words
        .into_iter()
        .filter(|w| w.confidence >= 0.35 && w.width > 2.0 && w.height > 4.0)
        .map(|mut w| {
            w.x = roi.x + w.x * sx;
            w.y = roi.y + w.y * sy;
            w.width *= sx;
            w.height *= sy;
            w
        })
        .collect())
}

// externalBin is copied beside the executable in both cargo and bundled builds.
// Resolving at runtime keeps OCR working when the .app moves to another machine.
fn ocr_helper_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| format!("Locate app: {e}"))?;
    let directory = executable
        .parent()
        .ok_or("App executable has no directory")?;
    let helper = directory.join("typoscope-ocr");
    if !helper.is_file() {
        return Err(format!("Bundled OCR helper missing: {}", helper.display()));
    }
    Ok(helper)
}

fn scan_roi(
    roi: ScreenRect,
    opts: CaptureOptions,
    tier: usize,
    mouse: (f64, f64),
) -> Result<Option<WordMapCache>, String> {
    let path = std::env::temp_dir().join(format!(
        "typoscope-ocr-{}-tier{tier}.png",
        std::process::id()
    ));
    let dimensions = capture_roi_png(roi.x, roi.y, roi.w, roi.h, &path, opts)?;
    let captured_at = Instant::now();
    let fingerprint_roi = fingerprint_region(roi, mouse);
    let fp = fingerprint(fingerprint_roi, opts).ok();
    let output = Command::new(ocr_helper_path()?).arg(&path).output();
    let _ = std::fs::remove_file(&path);
    let output = output.map_err(|e| format!("OCR helper failed to start: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("OCR: {}", stderr.trim()));
    }
    let raw: Vec<WordBox> =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("OCR response: {e}"))?;
    let pixels = stderr
        .lines()
        .find_map(|line| line.strip_prefix("OCR_META "))
        .and_then(|json| serde_json::from_str::<CaptureMeta>(json).ok())
        .map(|m| (m.image_width, m.image_height))
        .unwrap_or(dimensions);
    let raw_count = raw.len();
    let words = map_image_words(raw, roi, pixels)?;
    eprintln!("[typoscope ocr] capture tier={} ROI=({:.0},{:.0},{:.0},{:.0}) image={}x{} raw={} accepted={}",
        tier, roi.x, roi.y, roi.w, roi.h, pixels.0, pixels.1, raw_count, words.len());
    if words.is_empty() {
        return Ok(None);
    }
    Ok(Some(WordMapCache {
        roi,
        lines: cluster_lines(&words),
        fingerprint: fp,
        fingerprint_roi,
        captured_at,
        last_change_check: Instant::now(),
    }))
}

fn scan_regions(job: &OcrJob) -> Vec<ScreenRect> {
    if matches!(job.mode, SnapMode::Sentence | SnapMode::Paragraph) {
        // A line wrap is not a sentence boundary: include all visible context.
        vec![job.display]
    } else {
        vec![
            job.display.around(job.mouse.0, job.mouse.1, 700.0, 240.0),
            job.display,
        ]
    }
}
fn rebuild_cache(job: OcrJob) -> Result<Option<WordMapCache>, String> {
    let mut last = None;
    for (tier, roi) in scan_regions(&job).into_iter().enumerate() {
        if let Some(cache) = scan_roi(roi, job.opts, tier, job.mouse)? {
            let hit = select_unit(&cache.lines, job.mode, job.mouse.0, job.mouse.1).is_some();
            last = Some(cache);
            if hit {
                break;
            }
        }
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn word(x: f64, y: f64) -> WordBox {
        WordBox {
            text: "test".into(),
            confidence: 0.99,
            x,
            y,
            width: 100.0,
            height: 32.0,
        }
    }
    #[test]
    fn retina_word_positions_cover_top_middle_and_bottom() {
        let roi = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 1512.0,
            h: 982.0,
        };
        for y in [40.0, 480.0, 920.0] {
            let words = map_image_words(vec![word(800.0, y * 2.0)], roi, (3024, 1964)).unwrap();
            assert_eq!(words[0].y, y);
            assert_eq!(words[0].x, 400.0);
            let selected =
                select_unit(&cluster_lines(&words), SnapMode::Word, 425.0, y + 8.0).unwrap();
            assert_eq!(selected.word_count, 1);
        }
    }
    #[test]
    fn bottom_edge_roi_stays_inside_logical_display() {
        let display = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 1512.0,
            h: 982.0,
        };
        let roi = display.around(1450.0, 970.0, 700.0, 240.0);
        assert_eq!(roi.y, 742.0);
        assert_eq!(roi.x, 812.0);
        let words = map_image_words(vec![word(1200.0, 440.0)], roi, (1400, 480)).unwrap();
        assert_eq!(words[0].y, 962.0);
        assert_eq!(words[0].x, 1412.0);
    }
    #[test]
    fn non_uniform_image_scaling_uses_height_for_y() {
        let roi = ScreenRect {
            x: 50.0,
            y: 700.0,
            w: 500.0,
            h: 200.0,
        };
        let words = map_image_words(vec![word(200.0, 150.0)], roi, (1000, 200)).unwrap();
        assert_eq!(words[0].x, 150.0);
        assert_eq!(words[0].y, 850.0);
    }
    #[test]
    fn sentence_and_paragraph_scan_all_visible_lines() {
        let display = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 1512.0,
            h: 982.0,
        };
        for mode in [SnapMode::Sentence, SnapMode::Paragraph] {
            let job = OcrJob {
                mouse: (600.0, 950.0),
                display,
                opts: CaptureOptions::default(),
                mode,
                generation: 0,
            };
            assert_eq!(scan_regions(&job), vec![display]);
        }
    }
    fn cached_first_paragraph() -> WordMapCache {
        WordMapCache {
            roi: ScreenRect {
                x: 0.0,
                y: 0.0,
                w: 1512.0,
                h: 982.0,
            },
            lines: cluster_lines(&[word(200.0, 120.0)]),
            fingerprint: Some(1),
            fingerprint_roi: ScreenRect {
                x: 0.0,
                y: 40.0,
                w: 480.0,
                h: 180.0,
            },
            captured_at: Instant::now(),
            last_change_check: Instant::now(),
        }
    }

    #[test]
    fn entering_unmapped_paragraph_requests_scan_inside_fresh_roi() {
        let cache = cached_first_paragraph();
        for mode in [SnapMode::Word, SnapMode::Sentence] {
            assert_eq!(scan_reason(Some(&cache), mode, (220.0, 130.0)), None);
            // Another paragraph occupies y=500, but the previous OCR missed it.
            assert_eq!(
                scan_reason(Some(&cache), mode, (220.0, 500.0)),
                Some("cursor-miss")
            );
        }
    }

    #[test]
    fn old_position_result_does_not_delay_new_pointer_scan() {
        assert!(scan_due(
            Some("cursor-miss"),
            (220.0, 500.0),
            Some((220.0, 130.0)),
            Duration::from_millis(180),
            Duration::ZERO
        ));
        assert!(scan_due(
            Some("no-cache"),
            (220.0, 500.0),
            Some((220.0, 130.0)),
            Duration::from_millis(180),
            Duration::ZERO
        ));
    }

    #[test]
    fn blank_space_retries_are_bounded_but_do_not_wait_for_cache_expiry() {
        assert!(!scan_due(
            Some("cursor-miss"),
            (220.0, 500.0),
            Some((220.0, 500.0)),
            Duration::from_millis(200),
            Duration::from_millis(10)
        ));
        assert!(scan_due(
            Some("cursor-miss"),
            (220.0, 500.0),
            Some((220.0, 500.0)),
            Duration::from_millis(450),
            Duration::from_millis(250)
        ));
        assert!(!scan_due(
            None,
            (220.0, 500.0),
            Some((220.0, 130.0)),
            Duration::from_secs(1),
            Duration::from_secs(1)
        ));
    }
    #[test]
    fn transient_ocr_miss_keeps_selection_until_confirmed() {
        let mut cache = Some(cached_first_paragraph());
        let mut misses = 0;
        assert!(apply_refresh(
            &mut cache,
            None,
            SnapMode::Sentence,
            (220.0, 130.0),
            &mut misses
        ));
        assert!(select_unit(
            &cache.as_ref().unwrap().lines,
            SnapMode::Sentence,
            220.0,
            130.0
        )
        .is_some());
        assert!(!apply_refresh(
            &mut cache,
            None,
            SnapMode::Sentence,
            (220.0, 130.0),
            &mut misses
        ));
        assert!(cache.is_none());
    }

    #[test]
    fn fresh_match_replaces_old_selection_without_cancellation() {
        let mut cache = Some(cached_first_paragraph());
        let mut misses = 1;
        assert!(!apply_refresh(
            &mut cache,
            Some(cached_first_paragraph()),
            SnapMode::Word,
            (220.0, 130.0),
            &mut misses
        ));
        assert!(cache.is_some());
        assert_eq!(misses, 0);
    }

    #[test]
    fn confirmation_never_retains_geometry_from_paragraph_we_left() {
        let mut cache = Some(cached_first_paragraph());
        let mut misses = 0;
        assert!(!apply_refresh(
            &mut cache,
            None,
            SnapMode::Word,
            (220.0, 500.0),
            &mut misses
        ));
        assert!(cache.is_none());
    }

    #[test]
    fn fingerprint_ignores_distant_ads_and_menu_bar() {
        let display = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 1512.0,
            h: 982.0,
        };
        let probe = fingerprint_region(display, (220.0, 700.0));
        assert!(probe.contains(220.0, 700.0));
        assert!(!probe.contains(1200.0, 700.0));
        assert!(!probe.contains(1490.0, 10.0));
    }
}
