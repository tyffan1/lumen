/// Платформонезависимый дескриптор окна.
///
/// Windows: хранит HWND как usize (сырой указатель).
/// X11: хранит Window (XID).
/// macOS: хранит pid процесса (каст к usize).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle(pub usize);
