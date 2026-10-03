use device_query::{DeviceQuery, DeviceState, Keycode};
use serde::Serialize;
use std::{collections::HashSet, thread, time::Duration};
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
        let mut previous_keys = HashSet::new();
        let mut last_trust_check = std::time::Instant::now();

        loop {
            if device_state.is_none() && last_trust_check.elapsed() > Duration::from_secs(2) {
                device_state = DeviceState::checked_new();
                last_trust_check = std::time::Instant::now();
            }

            if let Some(ref state) = device_state {
                let (x, y) = state.get_mouse().coords;
                if window
                    .emit(
                        "device-mouse-move",
                        MousePos {
                            x: x as f64,
                            y: y as f64,
                        },
                    )
                    .is_err()
                {
                    break;
                }

                let current_keys: HashSet<Keycode> = state.get_keys().into_iter().collect();

                for key in current_keys.difference(&previous_keys) {
                    let shift_held = current_keys.contains(&Keycode::LShift)
                        || current_keys.contains(&Keycode::RShift);
                    let shortcut = match key {
                        Keycode::Up => Some("arrowup"),
                        Keycode::Down => Some("arrowdown"),
                        Keycode::G => Some("g"),
                        Keycode::H => Some("h"),
                        Keycode::LeftBracket => Some("["),
                        Keycode::RightBracket => Some("]"),
                        Keycode::Key1 | Keycode::Numpad1 => Some("1"),
                        Keycode::T => Some("t"),
                        Keycode::D => Some("d"),
                        Keycode::A if shift_held => Some("shifta"),
                        // Snap-mode keys (App ignores these unless auto-snap is on).
                        Keycode::W => Some("w"),
                        Keycode::L => Some("l"),
                        Keycode::S => Some("s"),
                        Keycode::P => Some("p"),
                        Keycode::M => Some("m"),
                        _ => None,
                    };

                    if let Some(shortcut) = shortcut {
                        if window.emit("device-key-down", shortcut).is_err() {
                            return;
                        }
                    }
                }

                previous_keys = current_keys;
            }

            thread::sleep(Duration::from_millis(16));
        }
    });
}
