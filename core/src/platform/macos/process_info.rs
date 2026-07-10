use std::os::raw::c_void;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{msg_send, class};

use objc2_app_kit::{NSGraphicsContext, NSWorkspace};
use objc2_foundation::{NSDictionary, NSRect, NSPoint, NSSize};

use crate::{AppIcon, WindowHandle};

// ===========================================================================
// FFI helper: CFStringCreateWithBytes (CoreFoundation)
// ===========================================================================

const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

type CFStringRef = *const c_void;

extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        numBytes: isize,
        encoding: u32,
        isExternalRepresentation: u8,
    ) -> CFStringRef;
    fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut i8,
        bufferSize: isize,
        encoding: u32,
    ) -> u8;
}

fn cfstring_from_str(s: &str) -> CFStringRef {
    unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            s.as_ptr(),
            s.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
            0,
        )
    }
}

// ===========================================================================
// FFI: CoreGraphics (для конвертации NSImage → RGBA)
// ===========================================================================

type CGImageRef = *mut c_void;
type CGContextRef = *mut c_void;
type CGColorSpaceRef = *mut c_void;

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

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bitsPerComponent: usize,
        bytesPerRow: usize,
        space: CGColorSpaceRef,
        bitmapInfo: u32,
    ) -> CGContextRef;
    fn CGBitmapContextGetData(ctx: CGContextRef) -> *mut c_void;
    fn CGContextDrawImage(ctx: CGContextRef, rect: CGRect, image: CGImageRef);
    fn CGContextRelease(ctx: CGContextRef);
    fn CGColorSpaceRelease(space: CGColorSpaceRef);
    fn CGImageGetWidth(image: CGImageRef) -> usize;
    fn CGImageGetHeight(image: CGImageRef) -> usize;
}

// ===========================================================================
// FFI: AXUIElement (для window_title)
// ===========================================================================

type AXUIElementRef = *mut c_void;
type AXError = i32;

const K_AXERROR_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut *mut c_void,
    ) -> AXError;
}

// ===========================================================================
// NSString → String
// ===========================================================================

#[allow(dead_code)]
fn nsstring_to_string(s: &NSObject) -> String {
    unsafe {
        let cstr: *const i8 = msg_send![s, UTF8String];
        if cstr.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(cstr)
            .to_string_lossy()
            .into_owned()
    }
}

fn cfstring_to_string_lossy(cf: CFStringRef) -> String {
    unsafe {
        let mut buf = [0i8; 4096];
        let ok = CFStringGetCString(cf, buf.as_mut_ptr(), buf.len() as isize, K_CF_STRING_ENCODING_UTF8);
        if ok == 0 {
            return String::new();
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(
            &buf[..len]
                .iter()
                .map(|&c| c as u8)
                .collect::<Vec<_>>(),
        )
        .into_owned()
    }
}

// ===========================================================================
// exe_name_by_pid
// ===========================================================================

#[allow(dead_code)]
pub fn exe_name_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let app: *mut NSObject = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid as i32
        ];
        if app.is_null() {
            return None;
        }
        let name: *mut NSObject = msg_send![app, localizedName];
        if name.is_null() {
            return None;
        }
        Some(nsstring_to_string(&*name))
    }
}

// ===========================================================================
// exe_full_path_by_pid
// ===========================================================================

#[allow(dead_code)]
pub fn exe_full_path_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let app: *mut NSObject = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid as i32
        ];
        if app.is_null() {
            return None;
        }
        let url: *mut NSObject = msg_send![app, bundleURL];
        if url.is_null() {
            return None;
        }
        let path_obj: *mut NSObject = msg_send![url, path];
        if path_obj.is_null() {
            return None;
        }
        Some(nsstring_to_string(&*path_obj))
    }
}

// ===========================================================================
// window_title — через AXUIElement (требует Accessibility permission)
// ===========================================================================

pub(crate) fn window_title_for_pid(pid: i32) -> String {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return String::new();
        }

        let focused_attr = cfstring_from_str("AXFocusedWindow");
        if focused_attr.is_null() {
            CFRelease(app as *const c_void);
            return String::new();
        }

        let mut focused: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(app, focused_attr, &mut focused);
        CFRelease(app as *const c_void);
        CFRelease(focused_attr as *const c_void);

        if err != K_AXERROR_SUCCESS || focused.is_null() {
            return String::new();
        }

        let title_attr = cfstring_from_str("AXTitle");
        if title_attr.is_null() {
            CFRelease(focused);
            return String::new();
        }

        let mut title: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(focused, title_attr, &mut title);
        CFRelease(focused);
        CFRelease(title_attr as *const c_void);

        if err != K_AXERROR_SUCCESS || title.is_null() {
            return String::new();
        }

        let result = cfstring_to_string_lossy(title as CFStringRef);
        CFRelease(title);
        result
    }
}

#[allow(dead_code)]
pub fn window_title(handle: &WindowHandle) -> String {
    let pid = handle.0 as i32;
    window_title_for_pid(pid)
}

// ===========================================================================
// NSImage → RGBA (с даунскейлом через Core Graphics)
// ===========================================================================

fn nsimage_to_rgba(ns_image: &NSObject) -> Option<AppIcon> {
    unsafe {
        let mut proposed_rect = NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize { width: 0.0, height: 0.0 },
        };
        let cg_image: *mut c_void = msg_send![
            ns_image,
            CGImageForProposedRect: &mut proposed_rect,
            context: std::ptr::null_mut::<NSGraphicsContext>(),
            hints: std::ptr::null_mut::<NSDictionary>()
        ];
        if cg_image.is_null() {
            return None;
        }

        let src_w = CGImageGetWidth(cg_image);
        let src_h = CGImageGetHeight(cg_image);
        if src_w == 0 || src_h == 0 {
            return None;
        }
        if src_w > 4096 || src_h > 4096 {
            return None;
        }

        const TARGET: usize = 64;
        let (tw, th) = if src_w > TARGET || src_h > TARGET {
            (TARGET, TARGET)
        } else {
            (src_w, src_h)
        };

        let color_space = CGColorSpaceCreateDeviceRGB();
        if color_space.is_null() {
            return None;
        }

        let bpr = tw * 4;
        let bitmap_info: u32 = 0x4001;
        let ctx = CGBitmapContextCreate(
            std::ptr::null_mut(),
            tw,
            th,
            8,
            bpr,
            color_space,
            bitmap_info,
        );
        CGColorSpaceRelease(color_space);
        if ctx.is_null() {
            return None;
        }

        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: tw as f64,
                height: th as f64,
            },
        };
        CGContextDrawImage(ctx, rect, cg_image);

        let data = CGBitmapContextGetData(ctx);
        if data.is_null() {
            CGContextRelease(ctx);
            return None;
        }

        let total = tw * th * 4;
        let mut rgba = Vec::with_capacity(total);
        std::ptr::copy_nonoverlapping(data as *const u8, rgba.as_mut_ptr(), total);
        rgba.set_len(total);

        CGContextRelease(ctx);

        for chunk in rgba.chunks_exact_mut(4) {
            let a = chunk[3] as u16;
            if a > 0 && a < 255 {
                let scale = 255u16;
                chunk[0] = ((chunk[0] as u16 * scale) / a).min(255) as u8;
                chunk[1] = ((chunk[1] as u16 * scale) / a).min(255) as u8;
                chunk[2] = ((chunk[2] as u16 * scale) / a).min(255) as u8;
            }
        }

        Some(AppIcon {
            rgba,
            width: tw as u32,
            height: th as u32,
        })
    }
}

// ===========================================================================
// extract_exe_icon (path-based)
// ===========================================================================

#[allow(dead_code)]
pub fn extract_exe_icon(exe_path: &str) -> Option<AppIcon> {
    unsafe {
        let workspace: Retained<NSWorkspace> = NSWorkspace::sharedWorkspace();

        let path_cstring = std::ffi::CString::new(exe_path).ok()?;
        let path_ns: *mut NSObject =
            msg_send![class!(NSString), stringWithUTF8String: path_cstring.as_ptr()];
        if path_ns.is_null() {
            return None;
        }

        let icon: *mut NSObject = msg_send![&*workspace, iconForFile: &*path_ns];
        if icon.is_null() {
            return None;
        }

        nsimage_to_rgba(&*icon)
    }
}

// ===========================================================================
// extract_icon_by_window (window-handle-based)
// ===========================================================================

#[allow(dead_code)]
pub fn extract_icon_by_window(handle: &WindowHandle) -> Option<AppIcon> {
    let pid = handle.0 as i32;
    unsafe {
        let app: *mut NSObject = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app.is_null() {
            return None;
        }
        let icon: *mut NSObject = msg_send![app, icon];
        if icon.is_null() {
            return None;
        }
        nsimage_to_rgba(&*icon)
    }
}

// ===========================================================================
// ProcessInfoProvider trait impl
// ===========================================================================

#[allow(dead_code)]
pub struct MacOsProcessInfoProvider;

impl crate::traits::ProcessInfoProvider for MacOsProcessInfoProvider {
    fn exe_name_by_pid(pid: u32) -> Option<String> {
        exe_name_by_pid(pid)
    }

    fn exe_full_path_by_pid(pid: u32) -> Option<String> {
        exe_full_path_by_pid(pid)
    }

    fn window_title(handle: &WindowHandle) -> String {
        window_title(handle)
    }

    fn extract_exe_icon(exe_path: &str) -> Option<AppIcon> {
        extract_exe_icon(exe_path)
    }

    fn extract_icon_by_window(handle: &WindowHandle) -> Option<AppIcon> {
        extract_icon_by_window(handle)
    }
}