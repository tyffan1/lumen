mod foreground;
mod idle;
mod fullscreen;
pub(crate) mod process_info;
pub(crate) mod icon;

pub use foreground::WindowsForegroundTracker;
pub use idle::WindowsIdleDetector;
