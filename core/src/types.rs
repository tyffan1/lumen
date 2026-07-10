use crate::WindowHandle;

/// Информация о процессе, агрегированная ядром.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub exe_name: String,
    pub exe_path: String,
    pub window_title: String,
    /// Дескриптор окна, если доступен (используется для извлечения иконки).
    pub window_handle: Option<WindowHandle>,
}

/// Иконка приложения в формате RGBA.
pub struct AppIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
