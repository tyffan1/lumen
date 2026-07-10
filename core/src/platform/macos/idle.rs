use std::time::Duration;

use crate::IdleDetector;

extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: i32) -> f64;
}

const kCGEventSourceStatePrivate: i32 = -1;
const kCGAnyInputEventType: i32 = -1;

pub struct MacOsIdleDetector;

impl IdleDetector for MacOsIdleDetector {
    fn idle_duration(&self) -> Duration {
        let secs = unsafe {
            CGEventSourceSecondsSinceLastEventType(
                kCGEventSourceStatePrivate,
                kCGAnyInputEventType,
            )
        };
        if secs < 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(secs)
    }
}
