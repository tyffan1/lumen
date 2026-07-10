use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, GWL_STYLE, WS_CAPTION, WS_THICKFRAME,
};

use crate::FullscreenDetector;
use crate::WindowHandle;

/// Сырая проверка fullscreen по HWND — используется внутри
/// foreground-трекера, где HWND уже доступен напрямую.
pub(crate) fn is_exclusive_fullscreen_raw(hwnd: HWND) -> bool {
    unsafe {
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return false;
        }

        if window_rect != mi.rcMonitor {
            return false;
        }

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if style == 0 {
            return false;
        }
        let style = style as u32;

        let has_caption = (style & WS_CAPTION.0) == WS_CAPTION.0;
        let has_thickframe = (style & WS_THICKFRAME.0) == WS_THICKFRAME.0;

        if has_caption && has_thickframe {
            return false;
        }

        true
    }
}

#[allow(dead_code)]
pub struct WindowsFullscreenDetector;

impl FullscreenDetector for WindowsFullscreenDetector {
    fn is_exclusive_fullscreen(handle: &WindowHandle) -> bool {
        let hwnd = HWND(handle.0 as *mut _);
        is_exclusive_fullscreen_raw(hwnd)
    }
}
