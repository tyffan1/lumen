//! lumen-core: событийный трекинг активности окон, простоя и fullscreen-режима.
//!
//! Ключевая идея: НЕ поллить в цикле. Используем WinEvent hook,
//! который будит поток только при смене foreground-окна.
//! Это даёт почти нулевую нагрузку в состоянии простоя.

mod foreground;
mod idle;
mod fullscreen;
mod process_info;
mod icon;
pub mod config;

pub use foreground::{ForegroundTracker, ForegroundEvent};
pub use idle::IdleWatcher;
pub use fullscreen::is_exclusive_fullscreen;
pub use process_info::ProcessInfo;
pub use icon::{AppIcon, extract_exe_icon};
pub use config::Config;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

/// Событие, которое core отдаёт наверх (в storage / ui слой).
#[derive(Debug, Clone)]
pub enum TrackerEvent {
    /// Сменился активный процесс/окно
    WindowChanged(ProcessInfo),
    /// Пользователь ушёл в AFK (порог задаётся в IdleWatcher)
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
/// low_priority: выставить IDLE_PRIORITY_CLASS текущему процессу,
/// чтобы не мешать другим приложениям (актуально в играх).
pub fn spawn_tracker(low_priority: bool, config: Arc<Mutex<Config>>) -> Receiver<TrackerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();

    let tx_fg = tx.clone();
    std::thread::spawn(move || {
        if low_priority {
            process_info::lower_own_priority();
        }

        let tracker = ForegroundTracker::new(tx_fg);
        tracker.run();
    });

    let tx_idle = tx.clone();
    let config_idle = config.clone();
    std::thread::spawn(move || {
        let watcher = IdleWatcher::new();
        watcher.run(tx_idle, config_idle);
    });

    rx
}
