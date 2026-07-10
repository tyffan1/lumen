use std::time::Duration;

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use crate::IdleDetector;

pub struct WindowsIdleDetector;

impl IdleDetector for WindowsIdleDetector {
    fn idle_duration(&self) -> Duration {
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
}
