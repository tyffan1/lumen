// Выбор ExtractIconExW вместо SHGetFileInfoW:
// SHGetFileInfoW требует SHFILEINFOW (заполнение→разбор), при ошибке
// возвращает ноль без явного HICON. ExtractIconExW чище: явно возвращает
// массив HICON и количество извлечённых иконок, не требует лишних структур.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDIBits, GetDC, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetIconInfo, ICONINFO, HICON,
};

pub struct AppIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn extract_exe_icon(exe_path: &str) -> Option<AppIcon> {
    unsafe {
        let path_wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();

        let mut hicon = HICON::default();
        // Запрашиваем LargeIcon (32×32 на 96 DPI) — качественнее для downscale
        let count = ExtractIconExW(
            windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            0,
            Some(&mut hicon),
            None,
            1,
        );

        if count != 1 || hicon.is_invalid() {
            return None;
        }

        let result = icon_to_rgba(hicon);
        let _ = DestroyIcon(hicon);
        result
    }
}

fn icon_to_rgba(hicon: HICON) -> Option<AppIcon> {
    unsafe {
        let mut info = ICONINFO::default();
        if GetIconInfo(hicon, &mut info).is_err() {
            return None;
        }

        let mut bm: BITMAP = std::mem::zeroed();
        let bm_size = std::mem::size_of::<BITMAP>() as i32;
        if GetObjectW(info.hbmColor, bm_size, Some(&mut bm as *mut _ as *mut _)) != bm_size {
            cleanup(info.hbmColor, info.hbmMask);
            return None;
        }

        let w = bm.bmWidth as u32;
        let h = bm.bmHeight as u32;
        if w == 0 || h == 0 || w > 256 || h > 256 {
            cleanup(info.hbmColor, info.hbmMask);
            return None;
        }

        let row_size = w as usize * 4;
        let mut pixels = vec![0u8; (h as usize) * row_size];

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w as i32;
        bmi.bmiHeader.biHeight = -(h as i32);
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = 0;

        let dc = GetDC(HWND::default());
        if dc.is_invalid() {
            cleanup(info.hbmColor, info.hbmMask);
            return None;
        }

        GetDIBits(
            dc,
            info.hbmColor,
            0,
            h,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = ReleaseDC(HWND::default(), dc);

        // AND-маска (1 bpp): бит=1 → прозрачный.
        // Для современных 32bpp-иконок маска обычно нулевая (альфа уже
        // встроена в канал), применяем только если маска непустая.
        let mask_buf = {
            let mask_row_stride = ((w + 31) / 32) * 4;
            let mut buf = vec![0u8; (mask_row_stride * h) as usize];
            let mut mask_bmi: BITMAPINFO = std::mem::zeroed();
            mask_bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            mask_bmi.bmiHeader.biWidth = w as i32;
            mask_bmi.bmiHeader.biHeight = -(h as i32);
            mask_bmi.bmiHeader.biPlanes = 1;
            mask_bmi.bmiHeader.biBitCount = 1;
            mask_bmi.bmiHeader.biCompression = 0;

            let dc2 = GetDC(HWND::default());
            if !dc2.is_invalid() {
                GetDIBits(
                    dc2,
                    info.hbmMask,
                    0,
                    h,
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut mask_bmi,
                    DIB_RGB_COLORS,
                );
                let _ = ReleaseDC(HWND::default(), dc2);
            }
            buf
        };

        if mask_buf.iter().any(|&b| b != 0) {
            let mask_row_stride = ((w + 31) / 32) * 4;
            for y in 0..h {
                for x in 0..w {
                    let byte_idx = (y * mask_row_stride + x / 8) as usize;
                    let bit_idx = 7 - (x % 8);
                    if mask_buf[byte_idx] & (1 << bit_idx) != 0 {
                        let px = ((y * w + x) * 4) as usize;
                        pixels[px + 3] = 0;
                    }
                }
            }
        }

        cleanup(info.hbmColor, info.hbmMask);

        // BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        Some(AppIcon {
            rgba: pixels,
            width: w,
            height: h,
        })
    }
}

unsafe fn cleanup(hbm_color: HBITMAP, hbm_mask: HBITMAP) {
    let _ = DeleteObject(hbm_color);
    let _ = DeleteObject(hbm_mask);
}
