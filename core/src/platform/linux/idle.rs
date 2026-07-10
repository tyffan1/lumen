use std::sync::Mutex;
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::IdleDetector;

pub struct LinuxIdleDetector {
    conn: Option<Mutex<RustConnection>>,
    screen_num: usize,
}

impl LinuxIdleDetector {
    pub fn new() -> Self {
        let result = x11rb::connect(None).ok();
        let (conn, screen_num) = match result {
            Some(c) => (Some(Mutex::new(c.0)), c.1),
            None => (None, 0),
        };
        Self { conn, screen_num }
    }
}

impl IdleDetector for LinuxIdleDetector {
    fn idle_duration(&self) -> Duration {
        let guard = match &self.conn {
            Some(c) => c.lock().unwrap(),
            None => return Duration::ZERO,
        };
        let root = guard.setup().roots[self.screen_num].root;
        let cookie = match guard.screensaver_query_info(root) {
            Ok(c) => c,
            Err(_) => return Duration::ZERO,
        };
        let reply = match cookie.reply() {
            Ok(r) => r,
            Err(_) => return Duration::ZERO,
        };
        Duration::from_millis(reply.ms_since_user_input as u64)
    }
}
