use std::cell::Cell;
use std::sync::mpsc::Sender;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window,
};

use super::fullscreen::is_fullscreen_raw;
use super::process_info::{exe_full_path_by_pid, exe_name_by_pid};
use crate::{ProcessInfo, TrackerEvent, WindowHandle};

thread_local! {
    static PREV_FULLSCREEN: Cell<bool> = const { Cell::new(false) };
}

x11rb::atom_manager! {
    Atoms: AtomCollection {
        net_active_window: b"_NET_ACTIVE_WINDOW",
        net_wm_name:      b"_NET_WM_NAME",
        net_wm_pid:       b"_NET_WM_PID",
        net_wm_state:     b"_NET_WM_STATE",
        net_wm_state_fullscreen: b"_NET_WM_STATE_FULLSCREEN",
        wm_name:          b"WM_NAME",
        utf8_string:      b"UTF8_STRING",
    }
}

pub struct LinuxForegroundTracker;

impl crate::ForegroundTracker for LinuxForegroundTracker {
    fn run(self, tx: Sender<TrackerEvent>) {
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(_) => return,
        };

        let atoms = match Atoms::new(&conn) {
            Ok(a) => a,
            Err(_) => return,
        };

        let root = conn.setup().roots[screen_num].root;

        let _ = conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::default().event_mask(EventMask::PROPERTY_CHANGE),
        );
        let _ = conn.flush();

        emit_current(&conn, root, &atoms, &tx);

        loop {
            let event = match conn.wait_for_event() {
                Ok(e) => e,
                Err(_) => break,
            };
            if let x11rb::protocol::xproto::Event::PropertyNotify(ev) = event {
                if ev.atom == atoms.net_active_window {
                    emit_current(&conn, root, &atoms, &tx);
                }
            }
        }
    }
}

fn emit_current(conn: &impl Connection, root: Window, atoms: &Atoms, tx: &Sender<TrackerEvent>) {
    let active = match get_active_window(conn, root, atoms) {
        Some(w) if w != 0 => w,
        _ => return,
    };

    let pid = match get_pid(conn, active, atoms) {
        Some(p) if p != 0 => p,
        _ => return,
    };

    let exe_name = exe_name_by_pid(pid).unwrap_or_else(|| "unknown".to_string());
    let exe_path = exe_full_path_by_pid(pid).unwrap_or_default();
    let window_title = get_window_title(conn, active, atoms);

    let info = ProcessInfo {
        pid,
        exe_name,
        exe_path,
        window_title,
        window_handle: Some(WindowHandle(active as usize)),
    };

    let is_fs = is_fullscreen_raw(conn, active, atoms.net_wm_state, atoms.net_wm_state_fullscreen);
    let was_prev_fs = PREV_FULLSCREEN.with(|c| c.replace(is_fs));

    let _ = tx.send(TrackerEvent::WindowChanged(info.clone()));

    if is_fs {
        let _ = tx.send(TrackerEvent::FullscreenEntered(info));
    } else if was_prev_fs {
        let _ = tx.send(TrackerEvent::FullscreenExited);
    }
}

fn get_active_window(conn: &impl Connection, root: Window, atoms: &Atoms) -> Option<Window> {
    let cookie = conn
        .get_property(false, root, atoms.net_active_window, AtomEnum::WINDOW, 0, 1)
        .ok()?;
    let reply = cookie.reply().ok()?;
    reply.value32()?.first().copied()
}

fn get_pid(conn: &impl Connection, window: Window, atoms: &Atoms) -> Option<u32> {
    let cookie = conn
        .get_property(false, window, atoms.net_wm_pid, AtomEnum::CARDINAL, 0, 1)
        .ok()?;
    let reply = cookie.reply().ok()?;
    reply.value32()?.first().copied()
}

fn get_window_title(conn: &impl Connection, window: Window, atoms: &Atoms) -> String {
    let cookie = conn
        .get_property(
            false,
            window,
            atoms.net_wm_name,
            atoms.utf8_string,
            0,
            u32::MAX,
        )
        .ok()
        .and_then(|c| c.reply().ok());
    if let Some(reply) = cookie {
        if !reply.value.is_empty() {
            if let Ok(s) = String::from_utf8(reply.value.to_vec()) {
                return s;
            }
        }
    }

    let cookie = conn
        .get_property(false, window, atoms.wm_name, AtomEnum::STRING, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok());
    if let Some(reply) = cookie {
        if !reply.value.is_empty() {
            return String::from_utf8_lossy(&reply.value).to_string();
        }
    }

    String::new()
}


