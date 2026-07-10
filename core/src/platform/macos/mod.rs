// Platform-specific implementations for macOS.
// This module is only compiled when targeting macOS.

pub mod foreground;
pub mod fullscreen;
pub mod idle;
pub mod process_info;

#[allow(unused_imports)]
pub use foreground::MacOsForegroundTracker;

#[allow(unused_imports)]
pub use fullscreen::is_fullscreen_for_pid;

#[allow(unused_imports)]
pub use idle::MacOsIdleDetector;

#[allow(unused_imports)]
pub use process_info::{
    extract_exe_icon, extract_icon_by_window, window_title, MacOsProcessInfoProvider,
};

#[allow(unused_imports)]
use crate::traits::ProcessInfoProvider; // reserved for future trait-based dispatcher