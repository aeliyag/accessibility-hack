//! Screen ROI capture for OCR.
//!
//! In-process `CGWindowListCreateImage` with global top-left logical coordinates.
//! Capture errors are surfaced; capturing our own overlay is never a fallback.
//!
//! Pass `below_window_id` (NSWindow.windowNumber) to capture only content
//! *under* the overlay so we never need to hide/show the panel.

use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub struct CaptureOptions {
    /// CGWindowID / NSWindow.windowNumber. When set, capture uses
    /// `kCGWindowListOptionOnScreenBelowWindow` so the overlay is excluded.
    pub below_window_id: Option<u32>,
}

#[cfg(target_os = "macos")]
pub fn capture_roi_png(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    dest: &Path,
    opts: CaptureOptions,
) -> Result<(u32, u32), String> {
    // A fallback desktop screenshot includes our opaque mask and corrupts OCR.
    // Surface permission/capture failures instead of feeding the overlay to Vision.
    capture_roi_cg(roi_x, roi_y, roi_w, roi_h, dest, opts)
}

/// Tiny fingerprint of a ROI for cheap content-change detection (no PNG write).
#[cfg(target_os = "macos")]
pub fn capture_roi_fingerprint(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    opts: CaptureOptions,
) -> Result<u64, String> {
    fingerprint_roi_cg(roi_x, roi_y, roi_w, roi_h, opts)
}

#[cfg(target_os = "macos")]
fn capture_roi_cg(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    dest: &Path,
    opts: CaptureOptions,
) -> Result<(u32, u32), String> {
    use std::ffi::CString;

    let image = create_cg_image(roi_x, roi_y, roi_w, roi_h, opts)?;
    unsafe {
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
        CFRelease(dest_ref);
        CGImageRelease(image);

        if !ok {
            return Err("CGImageDestinationFinalize failed".into());
        }
        Ok((width, height))
    }
}

#[cfg(target_os = "macos")]
fn fingerprint_roi_cg(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    opts: CaptureOptions,
) -> Result<u64, String> {
    use std::os::raw::c_void;
    use std::slice;

    let image = create_cg_image(roi_x, roi_y, roi_w, roi_h, opts)?;
    unsafe {
        let width = CGImageGetWidth(image);
        let height = CGImageGetHeight(image);
        if width == 0 || height == 0 {
            CGImageRelease(image);
            return Err("empty fingerprint image".into());
        }

        let provider = CGImageGetDataProvider(image);
        if provider.is_null() {
            CGImageRelease(image);
            return Err("no data provider".into());
        }
        let data_ref = CGDataProviderCopyData(provider);
        if data_ref.is_null() {
            CGImageRelease(image);
            return Err("copy data failed".into());
        }
        let len = CFDataGetLength(data_ref) as usize;
        let ptr = CFDataGetBytePtr(data_ref);
        let bytes = slice::from_raw_parts(ptr, len);

        // FNV-1a over a sparse sample so this stays cheap.
        let mut hash: u64 = 0xcbf29ce484222325;
        let step = (bytes.len() / 512).max(16);
        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += step;
        }
        // Mix in dimensions so resizes count as changes.
        hash ^= (width as u64).wrapping_shl(32) ^ height as u64;

        CFRelease(data_ref as *const c_void);
        CGImageRelease(image);
        Ok(hash)
    }
}

#[cfg(target_os = "macos")]
fn create_cg_image(
    roi_x: f64,
    roi_y: f64,
    roi_w: f64,
    roi_h: f64,
    opts: CaptureOptions,
) -> Result<*const std::os::raw::c_void, String> {
    let rect = CGRect {
        origin: CGPoint { x: roi_x, y: roi_y },
        size: CGSize {
            width: roi_w,
            height: roi_h,
        },
    };

    const ON_SCREEN_ONLY: u32 = 1 << 0;
    const ON_SCREEN_BELOW_WINDOW: u32 = 1 << 2;
    let (list_option, window_id) = match opts.below_window_id {
        Some(id) => (ON_SCREEN_BELOW_WINDOW, id),
        None => (ON_SCREEN_ONLY, 0u32),
    };
    const IMAGE_BEST_RES: u32 = 8;
    const IMAGE_IGNORE_FRAMING: u32 = 1;

    unsafe {
        let image = CGWindowListCreateImage(
            rect,
            list_option,
            window_id,
            IMAGE_BEST_RES | IMAGE_IGNORE_FRAMING,
        );
        if image.is_null() {
            return Err("CGWindowListCreateImage returned null".into());
        }
        Ok(image)
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}
#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}
#[cfg(target_os = "macos")]
#[repr(C)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[cfg(target_os = "macos")]
type CGImageRef = *const std::os::raw::c_void;
#[cfg(target_os = "macos")]
type CFURLRef = *const std::os::raw::c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const std::os::raw::c_void;
#[cfg(target_os = "macos")]
type CGImageDestinationRef = *mut std::os::raw::c_void;
#[cfg(target_os = "macos")]
type CGDataProviderRef = *const std::os::raw::c_void;
#[cfg(target_os = "macos")]
type CFDataRef = *const std::os::raw::c_void;

#[cfg(target_os = "macos")]
const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

#[cfg(target_os = "macos")]
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
    fn CGImageGetDataProvider(image: CGImageRef) -> CGDataProviderRef;
    fn CGDataProviderCopyData(provider: CGDataProviderRef) -> CFDataRef;
}

#[cfg(target_os = "macos")]
#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageDestinationCreateWithURL(
        url: CFURLRef,
        type_: CFStringRef,
        count: usize,
        options: *const std::os::raw::c_void,
    ) -> CGImageDestinationRef;
    fn CGImageDestinationAddImage(
        dest: CGImageDestinationRef,
        image: CGImageRef,
        properties: *const std::os::raw::c_void,
    );
    fn CGImageDestinationFinalize(dest: CGImageDestinationRef) -> u8;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: *const std::os::raw::c_void,
        buffer: *const u8,
        buf_len: isize,
        is_directory: u8,
    ) -> CFURLRef;
    fn CFRelease(cf: *const std::os::raw::c_void);
    fn CFStringCreateWithCString(
        alloc: *const std::os::raw::c_void,
        cStr: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFDataGetLength(theData: CFDataRef) -> isize;
    fn CFDataGetBytePtr(theData: CFDataRef) -> *const u8;
    static kCFAllocatorDefault: *const std::os::raw::c_void;
}
