use std::time::Duration;

use crate::IdleDetector;

extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: i32) -> f64;
}

const K_CG_EVENT_SOURCE_STATE_PRIVATE: i32 = -1;
const K_CG_ANY_INPUT_EVENT_TYPE: i32 = -1;

pub struct MacOsIdleDetector;

impl IdleDetector for MacOsIdleDetector {
    fn idle_duration(&self) -> Duration {
        let secs = unsafe {
            CGEventSourceSecondsSinceLastEventType(
                K_CG_EVENT_SOURCE_STATE_PRIVATE,
                K_CG_ANY_INPUT_EVENT_TYPE,
            )
        };
        if secs < 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(secs)
    }
}