use device_query::{DeviceQuery, DeviceState};
use serde::Serialize;
use std::{thread, time::Duration};
use tauri::{Emitter, WebviewWindow};

#[derive(Clone, Serialize)]
pub struct MousePos {
    pub x: f64,
    pub y: f64,
}

pub fn start_global_mouse_stream(window: WebviewWindow) {
    thread::spawn(move || {
        // Prefer checked_new so missing Accessibility does not kill this thread
        // (needed for Shift+A / other hotkeys). Mouse move may be absent until granted.
        let mut device_state = DeviceState::checked_new();
        let mut last_trust_check = std::time::Instant::now();

        loop {
            if device_state.is_none() && last_trust_check.elapsed() > Duration::from_secs(2) {
                device_state = DeviceState::checked_new();
                last_trust_check = std::time::Instant::now();
            }

            if let Some(ref state) = device_state {
                let (x, y) = state.get_mouse().coords;
                // macOS CGEventGetLocation already returns logical screen points.
                // Normalize the physical device_query coordinates on other platforms.
                #[cfg(target_os = "macos")]
                let mouse_scale = 1.0;
                #[cfg(not(target_os = "macos"))]
                let mouse_scale = window.scale_factor().unwrap_or(1.0);
                if window
                    .emit(
                        "device-mouse-move",
                        MousePos {
                            x: x as f64 / mouse_scale,
                            y: y as f64 / mouse_scale,
                        },
                    )
                    .is_err()
                {
                    break;
                }
            }

            thread::sleep(Duration::from_millis(16));
        }
    });
}
