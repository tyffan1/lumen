use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, QueryFullProcessImageNameW, SetPriorityClass,
    IDLE_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_WIN32,
};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

use crate::{ProcessInfoProvider, WindowHandle};

/// Понижаем приоритет собственного процесса.
pub fn lower_own_priority() {
    unsafe {
        let handle = GetCurrentProcess();
        let _ = SetPriorityClass(handle, IDLE_PRIORITY_CLASS);
    }
}

fn query_full_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;

        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_err() {
            return None;
        }

        let name_len = buf[..size as usize]
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(size as usize);
        Some(String::from_utf16_lossy(&buf[..name_len]))
    }
}

pub(super) fn exe_full_path_by_pid(pid: u32) -> Option<String> {
    query_full_path(pid)
}

pub(super) fn exe_name_by_pid(pid: u32) -> Option<String> {
    let full_path = query_full_path(pid)?;
    Some(
        Path::new(&full_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(full_path),
    )
}

fn window_title(handle: &WindowHandle) -> String {
    let hwnd = HWND(handle.0 as *mut _);
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..read as usize])
    }
}

#[allow(dead_code)]
pub struct WindowsProcessInfoProvider;

impl ProcessInfoProvider for WindowsProcessInfoProvider {
    fn exe_name_by_pid(pid: u32) -> Option<String> {
        exe_name_by_pid(pid)
    }

    fn exe_full_path_by_pid(pid: u32) -> Option<String> {
        exe_full_path_by_pid(pid)
    }

#[allow(dead_code)]
fn window_title(handle: &WindowHandle) -> String {
        window_title(handle)
    }

    fn extract_exe_icon(exe_path: &str) -> Option<crate::AppIcon> {
        super::icon::extract_exe_icon(exe_path)
    }

    fn extract_icon_by_window(_handle: &WindowHandle) -> Option<crate::AppIcon> {
        None
    }
}
