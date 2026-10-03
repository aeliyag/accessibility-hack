//! Screen ROI capture for OCR.
//!
//! Prefer in-process `CGWindowListCreateImage` (still present at runtime on
//! macOS 15+ even though the Swift SDK marks it unavailable). Fall back to
//! `/usr/sbin/screencapture -R` with top-left point coordinates.

use std::path::Path;
use std::process::Command;

#[cfg(target_os = "macos")]
pub fn capture_roi_png(roi_x: f64, roi_y: f64, roi_w: f64, roi_h: f64, dest: &Path) -> Result<(u32, u32), String> {
    if let Ok(size) = capture_roi_cg(roi_x, roi_y, roi_w, roi_h, dest) {
        return Ok(size);
    }
    capture_roi_screencapture(roi_x, roi_y, roi_w, roi_h, dest)
}

#[cfg(target_os = "macos")]
fn capture_roi_cg(roi_x: f64, roi_y: f64, roi_w: f64, roi_h: f64, dest: &Path) -> Result<(u32, u32), String> {
    use std::ffi::CString;
    use std::os::raw::{c_int, c_void};

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

    type CGImageRef = *const c_void;
    type CFURLRef = *const c_void;
    type CFStringRef = *const c_void;
    type CGImageDestinationRef = *mut c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCreateImage(
            screen_bounds: CGRect,
            list_option: u32,
            window_id: u32,
            image_option: u32,
        ) -> CGImageRef;
        fn CGImageGetWidth(image: CGImageRef) -> usize;
        fn CGImageGetHeight(image: CGImageRef) -> usize;
        fn CGImageRelease(image: CGImageRef);
    }

    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        fn CGImageDestinationCreateWithURL(
            url: CFURLRef,
            type_: CFStringRef,
            count: usize,
            options: *const c_void,
        ) -> CGImageDestinationRef;
        fn CGImageDestinationAddImage(
            dest: CGImageDestinationRef,
            image: CGImageRef,
            properties: *const c_void,
        );
        fn CGImageDestinationFinalize(dest: CGImageDestinationRef) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(
            allocator: *const c_void,
            buffer: *const u8,
            buf_len: isize,
            is_directory: u8,
        ) -> CFURLRef;
        fn CFRelease(cf: *const c_void);
        static kCFAllocatorDefault: *const c_void;
    }

    // UTType.png.identifier bridged as public.png
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            cStr: *const i8,
            encoding: u32,
        ) -> CFStringRef;
    }
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
    // kCGWindowListOptionOnScreenOnly = (1 << 0) = 1
    // kCGNullWindowID = 0
    // kCGWindowImageBestResolution = (1 << 3) = 8
    // kCGWindowImageBoundsIgnoreFraming = (1 << 0) = 1
    const LIST_ON_SCREEN_ONLY: u32 = 1;
    const IMAGE_BEST_RES: u32 = 8;
    const IMAGE_IGNORE_FRAMING: u32 = 1;

    let rect = CGRect {
        origin: CGPoint { x: roi_x, y: roi_y },
        size: CGSize {
            width: roi_w,
            height: roi_h,
        },
    };

    unsafe {
        let image = CGWindowListCreateImage(
            rect,
            LIST_ON_SCREEN_ONLY,
            0,
            IMAGE_BEST_RES | IMAGE_IGNORE_FRAMING,
        );
        if image.is_null() {
            return Err("CGWindowListCreateImage returned null".into());
        }

        let width = CGImageGetWidth(image) as u32;
        let height = CGImageGetHeight(image) as u32;
        if width == 0 || height == 0 {
            CGImageRelease(image);
            return Err("CGWindowListCreateImage returned empty image".into());
        }

        let path = CString::new(dest.to_string_lossy().as_bytes())
            .map_err(|_| "dest path NUL".to_string())?;
        let url = CFURLCreateFromFileSystemRepresentation(
            kCFAllocatorDefault,
            path.as_ptr() as *const u8,
            path.as_bytes().len() as isize,
            0,
        );
        if url.is_null() {
            CGImageRelease(image);
            return Err("CFURLCreate failed".into());
        }

        let uti = CString::new("public.png").unwrap();
        let type_str =
            CFStringCreateWithCString(kCFAllocatorDefault, uti.as_ptr(), K_CF_STRING_ENCODING_UTF8);
        let dest_ref = CGImageDestinationCreateWithURL(url, type_str, 1, std::ptr::null());
        CFRelease(type_str);
        CFRelease(url);

        if dest_ref.is_null() {
            CGImageRelease(image);
            return Err("CGImageDestinationCreateWithURL failed".into());
        }

        CGImageDestinationAddImage(dest_ref, image, std::ptr::null());
        let ok = CGImageDestinationFinalize(dest_ref) != 0;
        // Finalize consumes dest_ref.
        let _ = dest_ref;
        CGImageRelease(image);

        if !ok {
            return Err("CGImageDestinationFinalize failed".into());
        }

        let _ = c_int::from(ok as i8);
        Ok((width, height))
    }
}

fn capture_roi_screencapture(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    dest: &Path,
) -> Result<(u32, u32), String> {
    let region = format!(
        "{},{},{},{}",
        roi_x.round() as i32,
        roi_y.round() as i32,
        roi_w.round() as i32,
        roi_h.round() as i32
    );

    let output = Command::new("screencapture")
        .args(["-x", "-R", &region])
        .arg(dest)
        .output()
        .map_err(|e| format!("screencapture spawn failed: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() {
            format!("screencapture failed for rect {region}")
        } else {
            format!("screencapture: {stderr}")
        });
    }

    if !dest.exists() {
        return Err(format!(
            "screencapture produced no file for rect {region}. {}",
            if stderr.is_empty() {
                "Grant Screen Recording to Typoscope / your terminal."
            } else {
                stderr.as_str()
            }
        ));
    }

    // Image size unknown without decoding; OCR helper reports it.
    Ok((0, 0))
}
