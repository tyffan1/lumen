use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use crate::{Config, TrackerEvent};

pub struct IdleWatcher {
    poll_interval: Duration,
}

impl IdleWatcher {
    pub fn new() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
        }
    }

    /// Блокирующий цикл — запускать в отдельном потоке.
    /// Читает порог простоя из Config на каждой итерации.
    pub fn run(self, tx: Sender<TrackerEvent>, config: Arc<Mutex<Config>>) {
        let mut is_idle = false;

        loop {
            std::thread::sleep(self.poll_interval);

            let threshold = {
                let cfg = config.lock().unwrap();
                Duration::from_secs(cfg.idle_threshold_mins as u64 * 60)
            };

            let idle_for = idle_duration();
            let now_idle = idle_for >= threshold;

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
