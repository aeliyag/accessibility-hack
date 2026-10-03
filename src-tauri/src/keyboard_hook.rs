use device_query::{DeviceQuery, DeviceState, Keycode};
use std::{collections::HashSet, thread, time::Duration};
use tauri::{Emitter, WebviewWindow};

fn poll_shortcut(key: Keycode, shift: bool) -> Option<&'static str> {
    match key {
        Keycode::M if shift => Some("shift+m"),
        Keycode::X if shift => Some("shift+x"),
        Keycode::H if shift => Some("shift+h"),
        Keycode::R if shift => Some("shift+r"),
        Keycode::LeftBracket if shift => Some("shift+["),
        Keycode::RightBracket if shift => Some("shift+]"),
        Keycode::Up => Some("arrowup"),
        Keycode::Down => Some("arrowdown"),
        Keycode::Key1 | Keycode::Numpad1 => Some("1"),
        Keycode::D => Some("d"),
        _ => None,
    }
}

pub fn start_keyboard_hook(window: WebviewWindow) {
    // Poll-only keyboard hook. Global event grab (rdev) was destabilizing the
    // transparent overlay when edit-mode pointer capture and click-through toggled.
    thread::spawn(move || {
        let device_state = DeviceState::new();
        let mut previous_keys = HashSet::new();

        loop {
            let current_keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            let shift = current_keys.contains(&Keycode::LShift)
                || current_keys.contains(&Keycode::RShift);

            for key in current_keys.difference(&previous_keys) {
                if let Some(shortcut) = poll_shortcut(*key, shift) {
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
