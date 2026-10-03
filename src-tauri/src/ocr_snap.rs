use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordBox {
    pub text: String,
    pub confidence: f32,
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

static AUTO_SNAP: AtomicBool = AtomicBool::new(false);

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
        let mut last_good: Option<SnapRect> = None;

        loop {
            if !auto_snap_enabled() {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let Some(window) = app.get_webview_window("main") else {
                thread::sleep(Duration::from_millis(200));
                continue;
            };

            match capture_and_snap(&window) {
                Ok(Some(rect)) => {
                    last_good = Some(rect.clone());
                    let _ = window.emit("ocr-snap-rect", rect);
                }
                Ok(None) => {
                    if let Some(rect) = &last_good {
                        let _ = window.emit("ocr-snap-rect", rect.clone());
                    }
                }
                Err(err) => {
                    let _ = window.emit("ocr-snap-error", err);
                }
            }

            thread::sleep(Duration::from_millis(180));
        }
    });
}

fn ocr_helper_path() -> PathBuf {
    PathBuf::from(env!("TYPOSCOPE_OCR_HELPER"))
}

fn cg_mouse_location() -> (f64, f64) {
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

fn capture_and_snap(window: &WebviewWindow) -> Result<Option<SnapRect>, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no primary monitor".to_string())?;
    let monitor_size = monitor.size();
    let monitor_pos = monitor.position();
    let screen_w = monitor_size.width as f64 / scale;
    let screen_h = monitor_size.height as f64 / scale;

    // CG mouse: bottom-left origin, points.
    let (cg_x, cg_y_bottom) = cg_mouse_location();
    let mouse_x = cg_x;
    let mouse_y = screen_h - cg_y_bottom;

    let roi_w = screen_w.min(1400.0).max(400.0);
    let roi_h = 220.0;
    let roi_x = (mouse_x - roi_w / 2.0).clamp(0.0, (screen_w - roi_w).max(0.0));
    let roi_y = (mouse_y - roi_h / 2.0).clamp(0.0, (screen_h - roi_h).max(0.0));

    let capture_x = monitor_pos.x as f64 / scale + roi_x;
    let capture_y = monitor_pos.y as f64 / scale + roi_y;

    // Hide overlay so masks/underlay don't pollute OCR.
    let _ = window.hide();
    thread::sleep(Duration::from_millis(16));

    let tmp = std::env::temp_dir().join(format!("typoscope-ocr-{}.png", std::process::id()));
    let region = format!(
        "{},{},{},{}",
        capture_x.round() as i32,
        capture_y.round() as i32,
        roi_w.round() as i32,
        roi_h.round() as i32
    );

    let capture_status = Command::new("screencapture")
        .args(["-x", "-R", &region])
        .arg(&tmp)
        .status()
        .map_err(|e| format!("screencapture failed: {e}"))?;

    let _ = window.show();

    if !capture_status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(
            "screencapture failed — grant Screen Recording to Typoscope / your terminal".into(),
        );
    }

    let raw_words = run_ocr(&tmp);
    let _ = std::fs::remove_file(&tmp);
    let raw_words = raw_words?;

    // screencapture PNGs are typically in physical pixels on Retina.
    let origin = window.outer_position().map_err(|e| e.to_string())?;
    let origin_x = origin.x as f64 / scale;
    let origin_y = origin.y as f64 / scale;

    let words: Vec<WordBox> = raw_words
        .into_iter()
        .map(|w| WordBox {
            text: w.text,
            confidence: w.confidence,
            x: roi_x + w.x / scale - origin_x,
            y: roi_y + w.y / scale - origin_y,
            width: w.width / scale,
            height: w.height / scale,
        })
        .collect();

    let cursor_local_x = mouse_x - origin_x;
    let cursor_local_y = mouse_y - origin_y;
    Ok(compute_snap_rect(&words, cursor_local_x, cursor_local_y))
}

fn run_ocr(image_path: &PathBuf) -> Result<Vec<WordBox>, String> {
    let output = Command::new(ocr_helper_path())
        .arg(image_path)
        .output()
        .map_err(|e| format!("ocr helper failed: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ocr helper error: {err}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).map_err(|e| format!("ocr json parse: {e}"))
}

pub fn compute_snap_rect(words: &[WordBox], cursor_x: f64, cursor_y: f64) -> Option<SnapRect> {
    let mut words: Vec<&WordBox> = words
        .iter()
        .filter(|w| w.confidence >= 0.35 && w.width > 2.0 && w.height > 4.0)
        .collect();
    if words.is_empty() {
        return None;
    }

    words.sort_by(|a, b| {
        (a.y + a.height / 2.0)
            .partial_cmp(&(b.y + b.height / 2.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let median_h = {
        let mut heights: Vec<f64> = words.iter().map(|w| w.height).collect();
        heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        heights[heights.len() / 2]
    };
    let line_tol = (median_h * 0.45).max(6.0);

    let mut lines: Vec<Vec<&WordBox>> = Vec::new();
    for word in words {
        if let Some(line) = lines.last_mut() {
            let line_cy =
                line.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / line.len() as f64;
            let word_cy = word.y + word.height / 2.0;
            if (word_cy - line_cy).abs() <= line_tol {
                line.push(word);
                continue;
            }
        }
        lines.push(vec![word]);
    }

    let (_, line) = lines.iter().enumerate().min_by(|(_, a), (_, b)| {
        let a_cy = a.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / a.len() as f64;
        let b_cy = b.iter().map(|w| w.y + w.height / 2.0).sum::<f64>() / b.len() as f64;
        (a_cy - cursor_y)
            .abs()
            .partial_cmp(&(b_cy - cursor_y).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let mut line = line.clone();
    line.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let left = line.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
    let right = line
        .iter()
        .map(|w| w.x + w.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = line.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
    let bottom = line
        .iter()
        .map(|w| w.y + w.height)
        .fold(f64::NEG_INFINITY, f64::max);

    let pad_x = 10.0;
    let pad_y = (median_h * 0.2).clamp(3.0, 12.0);
    let height = ((bottom - top) * 1.15 + pad_y * 2.0).max(median_h * 1.2);
    let width = (right - left + pad_x * 2.0).min(1600.0).max(40.0);
    let x = left - pad_x;
    let y = (top + bottom) / 2.0 - height / 2.0;
    let _ = cursor_x;

    Some(SnapRect {
        x,
        y,
        width,
        height,
        word_count: line.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_line_union_width() {
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
        let snap = compute_snap_rect(&words, 170.0, 210.0).unwrap();
        assert_eq!(snap.word_count, 2);
        assert!(snap.width > 100.0);
        assert!(snap.y < 210.0 && snap.y + snap.height > 210.0);
    }
}
