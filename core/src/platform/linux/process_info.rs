use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, Window};

use crate::{AppIcon, ProcessInfoProvider, WindowHandle};

pub fn exe_name_by_pid(pid: u32) -> Option<String> {
    let comm_path = format!("/proc/{pid}/comm");
    let name = std::fs::read_to_string(&comm_path).ok()?;
    Some(name.trim().to_string())
}

pub fn exe_full_path_by_pid(pid: u32) -> Option<String> {
    let exe_path = format!("/proc/{pid}/exe");
    std::fs::read_link(&exe_path)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

pub fn window_title(handle: &WindowHandle) -> String {
    let window = handle.0 as Window;
    let (conn, _) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let utf8_string = conn
        .intern_atom(false, b"UTF8_STRING")
        .and_then(|c| c.reply())
        .map(|a| a.atom)
        .unwrap_or(0);

    let net_wm_name = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .and_then(|c| c.reply())
        .map(|a| a.atom)
        .unwrap_or(0);

    if utf8_string != 0 && net_wm_name != 0 {
        let cookie = conn
            .get_property(false, window, net_wm_name, utf8_string, 0, u32::MAX)
            .ok()
            .and_then(|c| c.reply().ok());
        if let Some(reply) = cookie {
            if !reply.value.is_empty() {
                if let Ok(s) = String::from_utf8(reply.value.to_vec()) {
                    return s;
                }
            }
        }
    }

    let wm_name = conn
        .intern_atom(false, b"WM_NAME")
        .and_then(|c| c.reply())
        .map(|a| a.atom)
        .unwrap_or(0);

    if wm_name != 0 {
        let cookie = conn
            .get_property(false, window, wm_name, AtomEnum::STRING, 0, u32::MAX)
            .ok()
            .and_then(|c| c.reply().ok());
        if let Some(reply) = cookie {
            if !reply.value.is_empty() {
                return String::from_utf8_lossy(&reply.value).to_string();
            }
        }
    }

    String::new()
}

pub fn extract_exe_icon(_exe_path: &str) -> Option<AppIcon> {
    None
}

pub fn extract_icon_by_window(handle: &WindowHandle) -> Option<AppIcon> {
    let window = handle.0 as Window;
    let (conn, _) = x11rb::connect(None).ok()?;

    let net_wm_icon = conn
        .intern_atom(false, b"_NET_WM_ICON")
        .and_then(|c| c.reply())
        .ok()?
        .atom;

    let cookie = conn
        .get_property(false, window, net_wm_icon, AtomEnum::CARDINAL, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok())?;

    if cookie.value.is_empty() || cookie.format != 32 {
        return None;
    }

    let data: Vec<u32> = cookie.value32()?.collect();
    if data.len() < 2 {
        return None;
    }

    // Find the icon closest to 32x32 among all variants in the property
    let target = 32u32;
    let mut best: Option<(usize, usize, u32)> = None; // (start_idx, pixel_count, size_diff)
    let mut off = 0usize;
    while off + 1 < data.len() {
        let w = data[off];
        let h = data[off + 1];
        let px = (w * h) as usize;
        if w == 0 || h == 0 || off + 2 + px > data.len() {
            break;
        }
        let diff = w.abs_diff(target) + h.abs_diff(target);
        let replace = match best {
            Some((_, _, cur)) => diff < cur,
            None => true,
        };
        if replace {
            best = Some((off, px, diff));
        }
        off += 2 + px;
    }

    let (start, pixel_count, _) = best?;
    let w = data[start];
    let h = data[start + 1];
    let pixels_start = start + 2;

    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for i in 0..pixel_count {
        let px = data[pixels_start + i];
        let a = ((px >> 24) & 0xff) as u8;
        let r = ((px >> 16) & 0xff) as u8;
        let g = ((px >> 8) & 0xff) as u8;
        let b = (px & 0xff) as u8;
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    Some(AppIcon {
        rgba,
        width: w,
        height: h,
    })
}

#[allow(dead_code)]
pub struct LinuxProcessInfoProvider;

impl ProcessInfoProvider for LinuxProcessInfoProvider {
    fn exe_name_by_pid(pid: u32) -> Option<String> {
        exe_name_by_pid(pid)
    }

    fn exe_full_path_by_pid(pid: u32) -> Option<String> {
        exe_full_path_by_pid(pid)
    }

    fn window_title(handle: &WindowHandle) -> String {
        window_title(handle)
    }

    fn extract_exe_icon(exe_path: &str) -> Option<AppIcon> {
        extract_exe_icon(exe_path)
    }

    fn extract_icon_by_window(handle: &WindowHandle) -> Option<AppIcon> {
        extract_icon_by_window(handle)
    }
}
