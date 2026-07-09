use std::sync::OnceLock;

use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoveredTitleButton {
    None,
    Close,
    Minimize,
}

#[derive(Debug, Clone)]
pub struct AppUsage {
    pub name: String,
    pub duration_secs: u64,
    pub is_active: bool,
    pub icon_rgba: Option<Vec<u8>>,
    pub icon_w: u32,
    pub icon_h: u32,
}

/// Добавлены поля:
///   text_dim         — цвет времени (приглушённый)
///   separator        — линия-разделитель под titlebar
///   active_indicator — вертикальная полоска активной строки
///   placeholder_icon — цвет кружка-заглушки для иконки
/// Удалены: titlebar (не нужен — фон един), active_bg (заменён на active_indicator)
pub struct Theme {
    pub background: Color,
    pub text: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub separator: Color,
    pub active_indicator: Color,
    pub placeholder_icon: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::from_rgba8(18, 18, 20, 255),
            text: Color::from_rgba8(228, 228, 231, 255),
            text_dim: Color::from_rgba8(110, 110, 120, 255),
            accent: Color::from_rgba8(110, 110, 200, 255),
            separator: Color::from_rgba8(38, 38, 42, 255),
            active_indicator: Color::from_rgba8(130, 130, 210, 255),
            placeholder_icon: Color::from_rgba8(48, 48, 53, 255),
        }
    }
}

const TITLEBAR_HEIGHT: f32 = 32.0;
const ROW_HEIGHT: f32 = 44.0;
const FONT_SIZE: f32 = 14.0;
const FONT_SIZE_DUR: f32 = 12.0;
const BAR_HEIGHT: f32 = 2.0;
const PADDING_X: f32 = 16.0;
const ICON_SIZE: f32 = 16.0;
const ICON_GAP: f32 = 8.0;
const INDICATOR_W: f32 = 2.0;

pub fn draw_frame(width: u32, height: u32, theme: &Theme, usage: &[AppUsage], button_hover: HoveredTitleButton) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap alloc");

    let mut paint_bg = Paint::default();
    paint_bg.set_color(theme.background);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap(),
        &paint_bg,
        Transform::identity(),
        None,
    );

    draw_titlebar_buttons(&mut pixmap, width, theme, button_hover);

    let mut paint_sep = Paint::default();
    paint_sep.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, TITLEBAR_HEIGHT, width as f32, 1.0).unwrap(),
        &paint_sep,
        Transform::identity(),
        None,
    );

    let font = font();
    let mut y = TITLEBAR_HEIGHT + 8.0;
    let max_dur = usage.iter().map(|a| a.duration_secs).max().unwrap_or(0);

    for app in usage {
        if y + ROW_HEIGHT > height as f32 {
            break;
        }

        if app.is_active {
            let mut paint_ind = Paint::default();
            paint_ind.set_color(theme.active_indicator);
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y + 4.0, INDICATOR_W, ROW_HEIGHT - 8.0).unwrap(),
                &paint_ind,
                Transform::identity(),
                None,
            );
        }

        let icon_x = PADDING_X;
        let icon_y = y + (ROW_HEIGHT - ICON_SIZE) / 2.0;
        let icon_w = if let Some(ref rgba) = app.icon_rgba {
            blit_rgba(
                &mut pixmap,
                rgba.as_slice(),
                app.icon_w,
                app.icon_h,
                icon_x as u32,
                icon_y as u32,
                ICON_SIZE as u32,
                ICON_SIZE as u32,
            );
            ICON_SIZE
        } else {
            draw_placeholder_icon(&mut pixmap, icon_x, icon_y, ICON_SIZE, theme.placeholder_icon);
            ICON_SIZE
        };

        let name_x = icon_x + icon_w + ICON_GAP;
        if let Some(f) = font {
            let base = text_baseline(y, ROW_HEIGHT, f, FONT_SIZE)
                .unwrap_or(y + ROW_HEIGHT / 2.0 + FONT_SIZE * 0.35);
            draw_text(&mut pixmap, &app.name, name_x, base, f, FONT_SIZE, theme.text);
        }

        let dur_str = fmt_duration(app.duration_secs);
        if let Some(f) = font {
            let text_w = measure_text(&dur_str, f, FONT_SIZE_DUR);
            let base = text_baseline(y, ROW_HEIGHT, f, FONT_SIZE_DUR)
                .unwrap_or(y + ROW_HEIGHT / 2.0 + FONT_SIZE_DUR * 0.35);
            draw_text(
                &mut pixmap,
                &dur_str,
                (width as f32 - text_w - PADDING_X).max(name_x + 8.0),
                base,
                f,
                FONT_SIZE_DUR,
                theme.text_dim,
            );
        }

        let bar_w = if max_dur == 0 || app.duration_secs == 0 {
            0.0
        } else {
            ((app.duration_secs as f32 / max_dur as f32) * (width as f32 - PADDING_X * 2.0)).max(4.0)
        };
        if bar_w > 0.0 {
            let bar_y = y + ROW_HEIGHT - BAR_HEIGHT - 6.0;
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = BAR_HEIGHT;
            stroke.line_cap = tiny_skia::LineCap::Round;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(PADDING_X, bar_y + BAR_HEIGHT / 2.0);
            path.line_to(PADDING_X + bar_w, bar_y + BAR_HEIGHT / 2.0);
            if let Some(p) = path.finish() {
                let mut paint = Paint::default();
                paint.set_color(theme.accent);
                paint.anti_alias = true;
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }

        y += ROW_HEIGHT;
    }

    pixmap
}

fn draw_titlebar_buttons(pixmap: &mut Pixmap, width: u32, theme: &Theme, hover: HoveredTitleButton) {
    let btn_size = 32.0;
    let x0 = width as f32 - btn_size;
    let x1 = x0 - btn_size;

    // фон при наведении
    let mut bg_paint = Paint::default();
    bg_paint.anti_alias = true;

    match hover {
        HoveredTitleButton::Close => {
            bg_paint.set_color(Color::from_rgba8(196, 43, 43, 255));
            pixmap.fill_rect(
                Rect::from_xywh(x0, 0.0, btn_size, btn_size).unwrap(),
                &bg_paint,
                Transform::identity(),
                None,
            );
        }
        HoveredTitleButton::Minimize => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(
                Rect::from_xywh(x1, 0.0, btn_size, btn_size).unwrap(),
                &bg_paint,
                Transform::identity(),
                None,
            );
        }
        HoveredTitleButton::None => {}
    }

    let mut paint = Paint::default();
    paint.set_color(theme.text_dim);
    paint.anti_alias = true;

    let mut stroke = tiny_skia::Stroke::default();
    stroke.width = 1.0;
    stroke.line_cap = tiny_skia::LineCap::Round;

    // close — тонкий крестик
    let cx = x0 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let arm = 3.5;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - arm, cy - arm);
    path.line_to(cx + arm, cy + arm);
    path.move_to(cx + arm, cy - arm);
    path.line_to(cx - arm, cy + arm);
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }

    // minimize — тонкая линия подчёркивания
    let cx = x1 + btn_size / 2.0;
    let cy = btn_size * 0.65;
    let half_w = 4.5;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - half_w, cy);
    path.line_to(cx + half_w, cy);
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_placeholder_icon(pixmap: &mut Pixmap, cx: f32, cy: f32, size: f32, color: Color) {
    let r = size / 2.0 - 1.0;
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let mut builder = tiny_skia::PathBuilder::new();
    builder.push_circle(cx + size / 2.0, cy + size / 2.0, r);
    if let Some(circle) = builder.finish() {
        pixmap.fill_path(&circle, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }
}

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {:02}m", h, m)
    } else if m > 0 {
        format!("{}m {:02}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn font() -> Option<&'static fontdue::Font> {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        let data = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")
            .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\arial.ttf"))
            .ok()?;
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    })
    .as_ref()
}

fn draw_text(pixmap: &mut Pixmap, text: &str, x: f32, y: f32, font: &fontdue::Font, size: f32, color: Color) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let cr = (color.red() * 255.0) as u32;
    let cg = (color.green() * 255.0) as u32;
    let cb = (color.blue() * 255.0) as u32;

    let mut cx = x;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        let gw = metrics.width as i32;
        let gh = metrics.height as i32;

        let origin_x = (cx + metrics.xmin as f32).round() as i32;
        let origin_y = (y - metrics.ymin as f32 - metrics.height as f32 + 1.0).round() as i32;

        for row in 0..gh {
            for col in 0..gw {
                let alpha = bitmap[(row * gw + col) as usize];
                if alpha == 0 {
                    continue;
                }

                let px = origin_x + col;
                let py = origin_y + row;

                if px >= 0 && px < pw && py >= 0 && py < ph {
                    let idx = (py as u32 * pixmap.width() + px as u32) as usize;
                    let pixel = &mut pixmap.pixels_mut()[idx];

                    let a = alpha as u32;
                    let inv_a = 255u32.wrapping_sub(a);
                    let r = ((a * cr + inv_a * pixel.red() as u32) / 255) as u8;
                    let g = ((a * cg + inv_a * pixel.green() as u32) / 255) as u8;
                    let b = ((a * cb + inv_a * pixel.blue() as u32) / 255) as u8;
                    if let Some(new_pixel) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255) {
                        *pixel = new_pixel;
                    }
                }
            }
        }

        cx += metrics.advance_width;
    }
}

fn text_baseline(row_y: f32, row_height: f32, font: &fontdue::Font, font_size: f32) -> Option<f32> {
    let line = font.horizontal_line_metrics(font_size)?;
    let text_height = line.ascent - line.descent;
    Some(row_y + (row_height - text_height) / 2.0 + line.ascent)
}

fn measure_text(text: &str, font: &fontdue::Font, size: f32) -> f32 {
    text.chars().map(|ch| font.rasterize(ch, size).0.advance_width).sum()
}

pub fn blit_to_softbuffer(pixmap: &Pixmap, out: &mut [u32]) {
    debug_assert_eq!(out.len(), (pixmap.width() * pixmap.height()) as usize);

    for (i, pixel) in pixmap.pixels().iter().enumerate() {
        let r = pixel.red() as u32;
        let g = pixel.green() as u32;
        let b = pixel.blue() as u32;
        out[i] = (r << 16) | (g << 8) | b | (0xFF << 24);
    }
}

fn blit_rgba(pixmap: &mut Pixmap, rgba: &[u8], src_w: u32, src_h: u32, dst_x: u32, dst_y: u32, dst_w: u32, dst_h: u32) {
    let painted = if src_w == dst_w && src_h == dst_h {
        rgba.to_vec()
    } else {
        resize_rgba_bilinear(rgba, src_w, src_h, dst_w, dst_h)
    };

    let pw = pixmap.width();
    let ph = pixmap.height();

    for row in 0..dst_h {
        for col in 0..dst_w {
            let src_idx = ((row * dst_w + col) * 4) as usize;
            let a = painted[src_idx + 3] as u32;
            if a == 0 {
                continue;
            }

            let px = dst_x + col;
            let py = dst_y + row;
            if px >= pw || py >= ph {
                continue;
            }

            let dst_idx = (py * pw + px) as usize;
            let dst_pixel = &mut pixmap.pixels_mut()[dst_idx];

            let sr = painted[src_idx] as u32;
            let sg = painted[src_idx + 1] as u32;
            let sb = painted[src_idx + 2] as u32;

            if a >= 254 {
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(sr as u8, sg as u8, sb as u8, 255) {
                    *dst_pixel = p;
                }
            } else {
                let inv_a = 255u32.wrapping_sub(a);
                let dr = dst_pixel.red() as u32;
                let dg = dst_pixel.green() as u32;
                let db = dst_pixel.blue() as u32;
                let r = ((a * sr + inv_a * dr) / 255) as u8;
                let g = ((a * sg + inv_a * dg) / 255) as u8;
                let b = ((a * sb + inv_a * db) / 255) as u8;
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255) {
                    *dst_pixel = p;
                }
            }
        }
    }
}

fn resize_rgba_bilinear(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let gx = (dx as f32 + 0.5) * src_w as f32 / dst_w as f32 - 0.5;
            let gy = (dy as f32 + 0.5) * src_h as f32 / dst_h as f32 - 0.5;
            let ix = gx.max(0.0).min((src_w - 1) as f32);
            let iy = gy.max(0.0).min((src_h - 1) as f32);
            let x0 = ix.floor() as u32;
            let y0 = iy.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let y1 = (y0 + 1).min(src_h - 1);
            let fx = ix - x0 as f32;
            let fy = iy - y0 as f32;

            let si = |x: u32, y: u32, ch: u32| src[((y * src_w + x) * 4 + ch) as usize] as f32;

            for ch in 0..4 {
                let c00 = si(x0, y0, ch as u32);
                let c10 = si(x1, y0, ch as u32);
                let c01 = si(x0, y1, ch as u32);
                let c11 = si(x1, y1, ch as u32);
                let top = c00 + (c10 - c00) * fx;
                let bot = c01 + (c11 - c01) * fx;
                let val = (top + (bot - top) * fy).round().max(0.0).min(255.0) as u8;
                dst[((dy * dst_w + dx) * 4 + ch as u32) as usize] = val;
            }
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_metrics() {
        let f = font().expect("font loaded");
        let test_str = "OpenCode";
        for ch in test_str.chars() {
            let (m, _) = f.rasterize(ch, FONT_SIZE);
            eprintln!(
                "ch='{ch}' xmin={} ymin={} w={} h={} advance={} bounds={{{:?}}}",
                m.xmin, m.ymin, m.width, m.height, m.advance_width, m.bounds,
            );
        }
        let mut ymins: Vec<_> = test_str.chars().map(|ch| f.rasterize(ch, FONT_SIZE).0.ymin).collect();
        ymins.sort();
        eprintln!("ymin range: {} ..= {} ({} px spread)", ymins.first().unwrap(), ymins.last().unwrap(), ymins.last().unwrap() - ymins.first().unwrap());
        let mut xmins: Vec<_> = test_str.chars().map(|ch| f.rasterize(ch, FONT_SIZE).0.xmin).collect();
        xmins.sort();
        eprintln!("xmin range: {} ..= {} ({} px spread)", xmins.first().unwrap(), xmins.last().unwrap(), xmins.last().unwrap() - xmins.first().unwrap());

        let baseline_y = 50.0;
        for ch in "OpendCode".chars() {
            let (m, _) = f.rasterize(ch, FONT_SIZE);
            let origin_y = (baseline_y - m.ymin as f32 - m.height as f32 + 1.0).round() as i32;
            let bottom = origin_y + m.height as i32 - 1;
            let expected_bottom = (baseline_y - m.ymin as f32).round() as i32;
            assert_eq!(bottom, expected_bottom, "ch='{ch}'");
        }
        eprintln!("origin_y formula verified for all chars");
    }

    #[test]
    fn fmt_duration_roundtrip() {
        assert_eq!(fmt_duration(0), "0s");
        assert_eq!(fmt_duration(1), "1s");
        assert_eq!(fmt_duration(59), "59s");
        assert_eq!(fmt_duration(60), "1m 00s");
        assert_eq!(fmt_duration(61), "1m 01s");
        assert_eq!(fmt_duration(3600), "1h 00m");
        assert_eq!(fmt_duration(3661), "1h 01m");
    }
}
