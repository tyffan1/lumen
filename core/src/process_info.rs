use std::path::Path;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, QueryFullProcessImageNameW, SetPriorityClass,
    IDLE_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_WIN32,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub exe_name: String,
    pub exe_path: String,
    pub window_title: String,
}

/// Понижаем приоритет собственного процесса, чтобы трекер
/// физически не мог соревноваться за CPU с игрой/тяжёлым софтом.
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

        let name_len = buf[..size as usize].iter().position(|&c| c == 0).unwrap_or(size as usize);
        Some(String::from_utf16_lossy(&buf[..name_len]))
    }
}

/// Возвращает полный путь к .exe процесса по PID.
pub fn exe_full_path_by_pid(pid: u32) -> Option<String> {
    query_full_path(pid)
}

/// Возвращает только имя файла .exe (без пути) по PID.
pub fn exe_name_by_pid(pid: u32) -> Option<String> {
    let full_path = query_full_path(pid)?;
    Some(
        Path::new(&full_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(full_path),
    )
}
