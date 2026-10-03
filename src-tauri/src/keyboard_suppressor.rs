#[cfg(target_os = "macos")]
mod macos {
    use std::{
        ffi::c_void,
        ptr,
        sync::{mpsc, mpsc::SyncSender, OnceLock},
        thread,
    };
    use tauri::{Emitter, WebviewWindow};

    type CGEventRef = *mut c_void;
    type CFMachPortRef = *mut c_void;
    type CFRunLoopRef = *mut c_void;
    type CFRunLoopSourceRef = *mut c_void;

    const KEY_DOWN: u32 = 10;
    const KEY_UP: u32 = 11;
    const AUTOREPEAT_FIELD: u32 = 8;
    const KEYCODE_FIELD: u32 = 9;
    const SHIFT_FLAG: u64 = 0x0002_0000;

    static SHORTCUT_SENDER: OnceLock<SyncSender<&'static str>> = OnceLock::new();

    fn shortcut_for_keycode(keycode: i64) -> Option<&'static str> {
        // Hardware keycodes are layout-independent on macOS.
        match keycode {
            46 => Some("shift+m"),
            7 => Some("shift+x"),
            4 => Some("shift+h"),
            15 => Some("shift+r"),
            33 => Some("shift+["),
            30 => Some("shift+]"),
            _ => None,
        }
    }

    type EventTapCallback = unsafe extern "C" fn(
        proxy: *mut c_void,
        event_type: u32,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: EventTapCallback,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        fn CGEventGetFlags(event: CGEventRef) -> u64;
        fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopCommonModes: *const c_void;
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(
            run_loop: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: *const c_void,
        );
        fn CFRunLoopRun();
    }

    unsafe extern "C" fn filter_shortcuts(
        _proxy: *mut c_void,
        event_type: u32,
        event: CGEventRef,
        _user_info: *mut c_void,
    ) -> CGEventRef {
        if event.is_null() || (event_type != KEY_DOWN && event_type != KEY_UP) {
            return event;
        }

        let flags = CGEventGetFlags(event);
        let keycode = CGEventGetIntegerValueField(event, KEYCODE_FIELD);

        if flags & SHIFT_FLAG != 0 {
            let Some(shortcut) = shortcut_for_keycode(keycode) else {
                return event;
            };

            if event_type == KEY_DOWN
                && CGEventGetIntegerValueField(event, AUTOREPEAT_FIELD) == 0
            {
                if let Some(sender) = SHORTCUT_SENDER.get() {
                    let _ = sender.try_send(shortcut);
                }
            }

            // Returning null consumes only the shortcut's character key. Shift
            // itself and all mouse/pointer events continue to the active app.
            return ptr::null_mut();
        }

        event
    }

    pub fn start(window: WebviewWindow) {
        let (sender, receiver) = mpsc::sync_channel(32);
        let _ = SHORTCUT_SENDER.set(sender);

        let event_window = window.clone();
        thread::spawn(move || {
            while let Ok(shortcut) = receiver.recv() {
                if event_window.emit("device-key-down", shortcut).is_err() {
                    break;
                }
            }
        });

        thread::spawn(move || unsafe {
            let keyboard_mask = (1_u64 << KEY_DOWN) | (1_u64 << KEY_UP);
            let tap = CGEventTapCreate(
                1, // kCGSessionEventTap
                0, // kCGHeadInsertEventTap
                0, // kCGEventTapOptionDefault (may suppress matched events)
                keyboard_mask,
                filter_shortcuts,
                ptr::null_mut(),
            );

            if tap.is_null() {
                eprintln!(
                    "Typoscope: shortcut suppression unavailable; using polling fallback"
                );
                crate::keyboard_hook::start_keyboard_hook(window);
                return;
            }

            let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
            if source.is_null() {
                eprintln!("Typoscope: could not create keyboard suppression run loop");
                return;
            }

            CFRunLoopAddSource(
                CFRunLoopGetCurrent(),
                source,
                kCFRunLoopCommonModes,
            );
            CGEventTapEnable(tap, true);
            CFRunLoopRun();
        });
    }
}

pub fn start_keyboard_suppressor(window: tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    macos::start(window);

    #[cfg(not(target_os = "macos"))]
    crate::keyboard_hook::start_keyboard_hook(window);
}
