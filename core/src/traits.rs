use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::{AppIcon, TrackerEvent, WindowHandle};

/// Отслеживание смены активного (foreground) окна.
///
/// Платформенная реализация должна предоставить инфраструктуру для
/// подписки на события смены foreground-окна (например, WinEvent hook)
/// и отправлять события в канал.
pub trait ForegroundTracker: Send {
    /// Блокирующий вызов — запускает цикл отслеживания foreground.
    /// События отправляются через `tx`. Функция не возвращается
    /// (исполняет платформенный message loop).
    fn run(self, tx: Sender<TrackerEvent>);
}

/// Детектор простоя пользователя (idle time).
pub trait IdleDetector: Send {
    /// Возвращает длительность с момента последнего ввода пользователя.
    fn idle_duration(&self) -> Duration;
}

/// Детектор exclusive fullscreen-режима окна.
pub trait FullscreenDetector {
    /// Проверяет, находится ли окно `handle` в exclusive (или borderless)
    /// fullscreen-режиме.
    fn is_exclusive_fullscreen(handle: &WindowHandle) -> bool;
}

/// Провайдер информации о процессах и иконках.
pub trait ProcessInfoProvider {
    /// Возвращает имя исполняемого файла процесса по PID.
    fn exe_name_by_pid(pid: u32) -> Option<String>;
    /// Возвращает полный путь к исполняемому файлу процесса по PID.
    fn exe_full_path_by_pid(pid: u32) -> Option<String>;
    /// Возвращает заголовок окна по платформенному дескриптору.
    fn window_title(handle: &WindowHandle) -> String;
    /// Извлекает иконку приложения из пути к исполняемому файлу.
    fn extract_exe_icon(exe_path: &str) -> Option<AppIcon>;
    /// Извлекает иконку окна по его дескриптору (например, \_NET\_WM\_ICON на X11).
    fn extract_icon_by_window(handle: &WindowHandle) -> Option<AppIcon>;
}
