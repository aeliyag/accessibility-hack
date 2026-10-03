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
        let device_state = DeviceState::new();

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

            thread::sleep(Duration::from_millis(16));
        }
    });
}
