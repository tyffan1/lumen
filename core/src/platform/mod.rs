#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::TrackerEvent;
use crate::config::Config;
#[allow(unused_imports)]
use crate::traits::ProcessInfoProvider;

// ---------------------------------------------------------------------------
// lower_own_priority
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn lower_own_priority() {
    windows::process_info::lower_own_priority();
}

#[cfg(target_os = "linux")]
pub fn lower_own_priority() {
    unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19); }
}

#[cfg(target_os = "macos")]
pub fn lower_own_priority() {
    unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19); }
}

// ---------------------------------------------------------------------------
// extract_exe_icon
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn extract_exe_icon(exe_path: &str) -> Option<crate::AppIcon> {
    windows::icon::extract_exe_icon(exe_path)
}

#[cfg(target_os = "linux")]
pub fn extract_exe_icon(exe_path: &str) -> Option<crate::AppIcon> {
    linux::extract_exe_icon(exe_path)
}

#[cfg(target_os = "macos")]
pub fn extract_exe_icon(exe_path: &str) -> Option<crate::AppIcon> {
    macos::extract_exe_icon(exe_path)
}

// ---------------------------------------------------------------------------
// extract_icon_by_window
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn extract_icon_by_window(handle: &crate::WindowHandle) -> Option<crate::AppIcon> {
    windows::process_info::WindowsProcessInfoProvider::extract_icon_by_window(handle)
}

#[cfg(target_os = "linux")]
pub fn extract_icon_by_window(handle: &crate::WindowHandle) -> Option<crate::AppIcon> {
    linux::extract_icon_by_window(handle)
}

#[cfg(target_os = "macos")]
pub fn extract_icon_by_window(handle: &crate::WindowHandle) -> Option<crate::AppIcon> {
    macos::extract_icon_by_window(handle)
}

// ---------------------------------------------------------------------------
// spawn_tracker_impl
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn spawn_tracker_impl(
    low_priority: bool,
    config: Arc<Mutex<Config>>,
) -> Receiver<TrackerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();

    if low_priority {
        lower_own_priority();
    }

    let tx_fg = tx.clone();
    std::thread::spawn(move || {
        let tracker = windows::WindowsForegroundTracker;
        crate::ForegroundTracker::run(tracker, tx_fg);
    });

    std::thread::spawn(move || {
        let detector = windows::WindowsIdleDetector;
        let watcher = crate::IdleWatcher::new(detector);
        watcher.run(tx, config);
    });

    rx
}

#[cfg(target_os = "linux")]
pub fn spawn_tracker_impl(
    low_priority: bool,
    config: Arc<Mutex<Config>>,
) -> Receiver<TrackerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();

    if low_priority {
        lower_own_priority();
    }

    let tx_fg = tx.clone();
    std::thread::spawn(move || {
        let tracker = linux::LinuxForegroundTracker;
        crate::ForegroundTracker::run(tracker, tx_fg);
    });

    std::thread::spawn(move || {
        let detector = linux::LinuxIdleDetector::new();
        let watcher = crate::IdleWatcher::new(detector);
        watcher.run(tx, config);
    });

    rx
}

#[cfg(target_os = "macos")]
pub fn spawn_tracker_impl(
    low_priority: bool,
    config: Arc<Mutex<Config>>,
) -> Receiver<TrackerEvent> {
    let (tx, rx) = std::sync::mpsc::channel();

    if low_priority {
        lower_own_priority();
    }

    let tx_fg = tx.clone();
    std::thread::spawn(move || {
        let tracker = macos::MacOsForegroundTracker;
        crate::ForegroundTracker::run(tracker, tx_fg);
    });

    std::thread::spawn(move || {
        let detector = macos::MacOsIdleDetector;
        let watcher = crate::IdleWatcher::new(detector);
        watcher.run(tx, config);
    });

    rx
}
