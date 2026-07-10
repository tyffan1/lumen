//! lumen-core: событийный трекинг активности окон, простоя и fullscreen-режима.

mod types;
mod window_handle;
mod traits;
pub mod config;
mod idle;
mod platform;

pub use types::{AppIcon, ProcessInfo};
pub use window_handle::WindowHandle;
pub use traits::{ForegroundTracker, IdleDetector, FullscreenDetector, ProcessInfoProvider};
pub use idle::IdleWatcher;

// Платформозависимые функции — re-export через #[cfg].
#[cfg(target_os = "windows")]
pub use config::Config;
pub use platform::extract_exe_icon;
pub use platform::extract_icon_by_window;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

/// Событие, которое core отдаёт наверх (в storage / ui слой).
#[derive(Debug, Clone)]
pub enum TrackerEvent {
    /// Сменился активный процесс/окно
    WindowChanged(ProcessInfo),
    /// Пользователь ушёл в AFK
    IdleStarted,
    /// Вернулся из AFK
    IdleEnded,
    /// Активное окно ушло в exclusive fullscreen (вероятно игра)
    FullscreenEntered(ProcessInfo),
    FullscreenExited,
}

/// Запускает всё необходимое (hook + idle watcher) в отдельных потоках
/// и возвращает Receiver, из которого UI/storage слой читает события.
///
/// low_priority: выставить IDLE_PRIORITY_CLASS текущему процессу.
pub fn spawn_tracker(low_priority: bool, config: Arc<Mutex<Config>>) -> Receiver<TrackerEvent> {
    platform::spawn_tracker_impl(low_priority, config)
}
