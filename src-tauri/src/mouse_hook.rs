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
        let device_state = DeviceState::new();
        let mut previous_keys = HashSet::new();

        loop {
            let (x, y) = device_state.get_mouse().coords;
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

            let current_keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            for key in current_keys.difference(&previous_keys) {
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
                    _ => None,
                };

                if let Some(shortcut) = shortcut {
                    if window.emit("device-key-down", shortcut).is_err() {
                        return;
                    }
                }
            }

            previous_keys = current_keys;
            thread::sleep(Duration::from_millis(16));
        }
    });
}
