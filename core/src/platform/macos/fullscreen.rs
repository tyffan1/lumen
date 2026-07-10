use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{FullscreenDetector, WindowHandle};

type AXUIElementRef = *mut c_void;
type CFTypeRef = *mut c_void;
type CFBooleanRef = *const c_void;
type CFStringRef = *const c_void;
type AXError = i32;

const K_AXERROR_SUCCESS: AXError = 0;
const K_AXERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25206;
const K_AXERROR_NO_VALUE: AXError = -25208;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
}

extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFBooleanGetValue(boolean: CFBooleanRef) -> u8;
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        numBytes: isize,
        encoding: u32,
        isExternalRepresentation: u8,
    ) -> CFStringRef;
}

static ACCESSIBILITY_WARNED: AtomicBool = AtomicBool::new(false);

fn ax_log_warning_once() {
    if ACCESSIBILITY_WARNED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        eprintln!(
            "lumen[macos]: AXUIElement вернул ошибку — скорее всего нет Accessibility permission. \
             Выдайте разрешение в System Settings → Privacy & Security → Accessibility."
        );
    }
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

pub fn is_fullscreen_for_pid(pid: i32) -> bool {
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return false;
    }

    let attr = cfstring_from_str("AXFullScreen");
    if attr.is_null() {
        unsafe {
            CFRelease(app as *const c_void);
        }
        return false;
    }

    let mut value: CFTypeRef = std::ptr::null_mut();
    let err = unsafe { AXUIElementCopyAttributeValue(app, attr, &mut value) };

    unsafe {
        CFRelease(app as *const c_void);
        CFRelease(attr as *const c_void);
    }

    match err {
        K_AXERROR_SUCCESS => {
            if value.is_null() {
                return false;
            }
            let is_fullscreen = unsafe { CFBooleanGetValue(value as CFBooleanRef) != 0 };
            unsafe {
                CFRelease(value);
            }
            is_fullscreen
        }
        K_AXERROR_ATTRIBUTE_UNSUPPORTED | K_AXERROR_NO_VALUE => false,
        _ => {
            ax_log_warning_once();
            false
        }
    }
}

#[allow(dead_code)]
pub struct MacOsFullscreenDetector;

impl FullscreenDetector for MacOsFullscreenDetector {
    fn is_exclusive_fullscreen(handle: &WindowHandle) -> bool {
        let pid = handle.0 as i32;
        is_fullscreen_for_pid(pid)
    }
}