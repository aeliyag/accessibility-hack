use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoScrollResult {
    pub scrollable: bool,
    pub scrolled: bool,
    pub permission_denied: bool,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::AutoScrollResult;
    use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::CFString;
    use core_graphics::event::{CGEvent, ScrollEventUnit};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::os::raw::c_void;
    use std::ptr;

    const SCROLLABLE_ROLES: &[&str] = &[
        "AXScrollArea",
        "AXList",
        "AXOutline",
        "AXTable",
        "AXWebArea",
        "AXTextArea",
        "AXSplitGroup",
        "AXGroup",
        "AXWindow",
        "AXSheet",
        "AXDrawer",
    ];

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut c_void;
        fn AXUIElementCopyElementAtPosition(
            system: *mut c_void,
            x: f32,
            y: f32,
            element: *mut *mut c_void,
        ) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: *mut c_void,
            attribute: *const __CFString,
            value: *mut *mut c_void,
        ) -> i32;
    }

    #[repr(C)]
    struct __CFString;

    fn mouse_screen_point() -> Option<(f32, f32)> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
        let event = CGEvent::new(source).ok()?;
        let point = event.location();
        Some((point.x as f32, point.y as f32))
    }

    /// Reads a CFString without taking ownership; the caller still releases `value`.
    unsafe fn cf_string_value(value: *mut c_void) -> Option<String> {
        if value.is_null() || CFGetTypeID(value as CFTypeRef) != CFString::type_id() {
            return None;
        }
        Some(CFString::wrap_under_get_rule(value as _).to_string())
    }

    /// Reads a CFBoolean without taking ownership; the caller still releases `value`.
    unsafe fn cf_bool_value(value: *mut c_void) -> Option<bool> {
        if value.is_null() || CFGetTypeID(value as CFTypeRef) != CFBoolean::type_id() {
            return None;
        }
        Some(value as CFTypeRef == CFBoolean::true_value().as_CFTypeRef())
    }

    unsafe fn copy_attribute(element: *mut c_void, name: &str) -> Option<*mut c_void> {
        let attr = CFString::new(name);
        let mut value: *mut c_void = ptr::null_mut();
        let status = AXUIElementCopyAttributeValue(
            element,
            attr.as_concrete_TypeRef() as *const __CFString,
            &mut value,
        );
        if status != 0 || value.is_null() {
            return None;
        }
        Some(value)
    }

    unsafe fn role_of(element: *mut c_void) -> Option<String> {
        let value = copy_attribute(element, "AXRole")?;
        let role = cf_string_value(value);
        CFRelease(value as _);
        role
    }

    unsafe fn has_vertical_scrollbar(element: *mut c_void) -> bool {
        let value = match copy_attribute(element, "AXVerticalScrollBar") {
            Some(value) => value,
            None => return false,
        };
        let enabled = if let Some(enabled_attr) = copy_attribute(value, "AXEnabled") {
            let enabled = cf_bool_value(enabled_attr).unwrap_or(true);
            CFRelease(enabled_attr as _);
            enabled
        } else {
            true
        };
        CFRelease(value as _);
        enabled
    }

    unsafe fn is_scrollable_element(element: *mut c_void) -> bool {
        if let Some(role) = role_of(element) {
            if SCROLLABLE_ROLES.iter().any(|candidate| *candidate == role) {
                if has_vertical_scrollbar(element) {
                    return true;
                }
                if role == "AXWebArea" || role == "AXList" || role == "AXTable" {
                    return true;
                }
            }
        }

        has_vertical_scrollbar(element)
    }

    unsafe fn scrollable_at_point(x: f32, y: f32) -> Result<bool, bool> {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return Err(true);
        }

        let mut element: *mut c_void = ptr::null_mut();
        let status = AXUIElementCopyElementAtPosition(system, x, y, &mut element);
        CFRelease(system as _);

        if status != 0 || element.is_null() {
            return Err(true);
        }

        let mut current = element;
        let mut scrollable = false;

        for _ in 0..16 {
            if is_scrollable_element(current) {
                scrollable = true;
                break;
            }

            let parent = match copy_attribute(current, "AXParent") {
                Some(parent) => parent,
                None => break,
            };

            CFRelease(current as _);
            current = parent;
        }

        CFRelease(current as _);
        Ok(scrollable)
    }

    fn post_scroll_down_pixels(pixels: i32) -> bool {
        if pixels <= 0 {
            return true;
        }

        let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            Ok(source) => source,
            Err(_) => return false,
        };

        // Negative wheel values scroll content down on macOS.
        let event = match CGEvent::new_scroll_event(
            source,
            ScrollEventUnit::PIXEL,
            1,
            -pixels,
            0,
            0,
        ) {
            Ok(event) => event,
            Err(_) => return false,
        };

        event.post(core_graphics::event::CGEventTapLocation::HID);
        true
    }

    pub fn auto_scroll_step(pixels: i32, check_target: bool) -> AutoScrollResult {
        if !check_target {
            return AutoScrollResult {
                scrollable: true,
                scrolled: post_scroll_down_pixels(pixels),
                permission_denied: false,
            };
        }

        let target = mouse_screen_point().map(|(x, y)| unsafe { scrollable_at_point(x, y) });

        match target {
            None | Some(Err(_)) => AutoScrollResult {
                scrollable: true,
                scrolled: post_scroll_down_pixels(pixels),
                permission_denied: matches!(target, Some(Err(true))),
            },
            Some(Ok(false)) => AutoScrollResult {
                scrollable: false,
                scrolled: false,
                permission_denied: false,
            },
            Some(Ok(true)) => AutoScrollResult {
                scrollable: true,
                scrolled: post_scroll_down_pixels(pixels),
                permission_denied: false,
            },
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::AutoScrollResult;

    pub fn auto_scroll_step(_pixels: i32, _check_target: bool) -> AutoScrollResult {
        AutoScrollResult {
            scrollable: false,
            scrolled: false,
            permission_denied: false,
        }
    }
}

// Accessibility queries can block (and may target this app's own windows), so keep
// them off the main thread instead of running inside the webview's IPC callback.
#[tauri::command]
pub async fn auto_scroll_step(pixels: i32, check_target: bool) -> Result<AutoScrollResult, String> {
    tauri::async_runtime::spawn_blocking(move || platform::auto_scroll_step(pixels, check_target))
        .await
        .map_err(|error| error.to_string())
}
