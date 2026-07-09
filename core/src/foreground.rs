use std::cell::RefCell;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetWindowThreadProcessId, TranslateMessage,
    EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use crate::process_info::exe_name_by_pid;
use crate::{ProcessInfo, TrackerEvent};

thread_local! {
    static TX: RefCell<Option<Sender<TrackerEvent>>> = RefCell::new(None);
}

pub type ForegroundEvent = TrackerEvent;

pub struct ForegroundTracker {
    tx: Sender<TrackerEvent>,
}

impl ForegroundTracker {
    pub fn new(tx: Sender<TrackerEvent>) -> Self {
        Self { tx }
    }

    /// Блокирующий вызов: регистрирует хук и крутит message loop,
    /// пока поток жив. CPU расходуется только когда ОС реально
    /// присылает событие смены foreground-окна — в простое поток спит.
    pub fn run(self) {
        TX.with(|cell| *cell.borrow_mut() = Some(self.tx.clone()));

        let hook: HWINEVENTHOOK = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };

        emit_current_foreground(&self.tx);

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
        }
    }
}

extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if hwnd.0.is_null() {
        return;
    }
    TX.with(|cell| {
        if let Some(tx) = cell.borrow().as_ref() {
            emit_window(tx, hwnd);
        }
    });
}

fn emit_current_foreground(tx: &Sender<TrackerEvent>) {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    if !hwnd.0.is_null() {
        emit_window(tx, hwnd);
    }
}

fn emit_window(tx: &Sender<TrackerEvent>, hwnd: HWND) {
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return;
    }

    let exe_name = exe_name_by_pid(pid).unwrap_or_else(|| "unknown".to_string());
    let window_title = window_title(hwnd);

    let info = ProcessInfo { pid, exe_name, window_title };

    if crate::fullscreen::is_exclusive_fullscreen(hwnd) {
        let _ = tx.send(TrackerEvent::FullscreenEntered(info.clone()));
    }

    let _ = tx.send(TrackerEvent::WindowChanged(info));
}

fn window_title(hwnd: HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..read as usize])
    }
}
