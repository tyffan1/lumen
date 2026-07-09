use std::sync::mpsc::Sender;
use std::time::Duration;

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use crate::TrackerEvent;

pub struct IdleWatcher {
    threshold: Duration,
    poll_interval: Duration,
}

impl IdleWatcher {
    pub fn new(threshold: Duration) -> Self {
        Self {
            threshold,
            poll_interval: Duration::from_secs(5),
        }
    }

    /// Блокирующий цикл — запускать в отдельном потоке (тот же, что и
    /// ForegroundTracker, либо отдельный лёгкий поток на std::thread::sleep).
    pub fn run(self, tx: Sender<TrackerEvent>) {
        let mut is_idle = false;

        loop {
            std::thread::sleep(self.poll_interval);

            let idle_for = idle_duration();
            let now_idle = idle_for >= self.threshold;

            if now_idle && !is_idle {
                is_idle = true;
                let _ = tx.send(TrackerEvent::IdleStarted);
            } else if !now_idle && is_idle {
                is_idle = false;
                let _ = tx.send(TrackerEvent::IdleEnded);
            }
        }
    }
}

fn idle_duration() -> Duration {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            let now = GetTickCount();
            let elapsed_ms = now.wrapping_sub(info.dwTime);
            Duration::from_millis(elapsed_ms as u64)
        } else {
            Duration::ZERO
        }
    }
}
