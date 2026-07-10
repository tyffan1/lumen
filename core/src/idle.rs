use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{IdleDetector, TrackerEvent};
use crate::config::Config;

/// Универсальный watcher простоя, работающий с любым `IdleDetector`.
///
/// Содержит платформонезависимую логику опроса, сравнения с порогом
/// и отправки событий IdleStarted / IdleEnded.
pub struct IdleWatcher<D: IdleDetector> {
    detector: D,
    poll_interval: Duration,
}

impl<D: IdleDetector> IdleWatcher<D> {
    pub fn new(detector: D) -> Self {
        Self {
            detector,
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

            let idle_for = self.detector.idle_duration();
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
