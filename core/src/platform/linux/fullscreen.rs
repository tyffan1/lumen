use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, Window};

use crate::{FullscreenDetector, WindowHandle};

/// Проверяет, содержит ли свойство `_NET_WM_STATE` окна значение
/// `_NET_WM_STATE_FULLSCREEN`. Используется внутри foreground-трекера,
/// где `conn` уже открыт и атомы уже заинтернены.
pub(crate) fn is_fullscreen_raw(
    conn: &impl Connection,
    window: Window,
    state_prop_atom: u32,
    fullscreen_value_atom: u32,
) -> bool {
    let cookie = conn
        .get_property(false, window, state_prop_atom, AtomEnum::ATOM, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok());
    let reply = match cookie {
        Some(r) => r,
        None => return false,
    };
    let Some(values) = reply.value32() else {
        return false;
    };
    for atom_id in values.iter() {
        if *atom_id == fullscreen_value_atom {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
pub struct LinuxFullscreenDetector;

impl FullscreenDetector for LinuxFullscreenDetector {
    fn is_exclusive_fullscreen(handle: &WindowHandle) -> bool {
        let window = handle.0 as Window;
        let (conn, _) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let state_atom = match conn
            .intern_atom(false, b"_NET_WM_STATE")
            .and_then(|c| c.reply())
        {
            Ok(a) => a.atom,
            Err(_) => return false,
        };
        let fullscreen_atom = match conn
            .intern_atom(false, b"_NET_WM_STATE_FULLSCREEN")
            .and_then(|c| c.reply())
        {
            Ok(a) => a.atom,
            Err(_) => return false,
        };

        is_fullscreen_raw(&conn, window, state_atom, fullscreen_atom)
    }
}
