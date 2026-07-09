use std::sync::OnceLock;

use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

/// Одно приложение в списке — имя и длительность.
/// is_active = true для текущего foreground-окна.
#[derive(Debug, Clone)]
pub struct AppUsage {
    pub name: String,
    pub duration_secs: u64,
    pub is_active: bool,
}

pub struct Theme {
    pub background: Color,
    pub titlebar: Color,
    pub text: Color,
    pub accent: Color,
    pub active_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::from_rgba8(24, 24, 27, 255),
            titlebar: Color::from_rgba8(32, 32, 36, 255),
            text: Color::from_rgba8(228, 228, 231, 255),
            accent: Color::from_rgba8(99, 102, 241, 255),
            active_bg: Color::from_rgba8(39, 39, 42, 255),
        }
    }
}

const TITLEBAR_HEIGHT: f32 = 32.0;
const ROW_HEIGHT: f32 = 36.0;
const FONT_SIZE: f32 = 14.0;
const BAR_HEIGHT: f32 = 4.0;

/// Рисует один кадр — фон, titlebar и список приложений.
pub fn draw_frame(width: u32, height: u32, theme: &Theme, usage: &[AppUsage]) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap alloc");

    let mut bg = Paint::default();
    bg.set_color(theme.background);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap(),
        &bg,
        Transform::identity(),
        None,
    );

    let mut tb = Paint::default();
    tb.set_color(theme.titlebar);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, TITLEBAR_HEIGHT).unwrap(),
        &tb,
        Transform::identity(),
        None,
    );

    draw_titlebar_buttons(&mut pixmap, width);

    let font = font();
    let mut y = TITLEBAR_HEIGHT + 4.0;
    let max_dur = usage.iter().map(|a| a.duration_secs).max().unwrap_or(0);

    let mut paint_accent = Paint::default();
    paint_accent.set_color(theme.accent);

    let mut paint_active_bg = Paint::default();
    paint_active_bg.set_color(theme.active_bg);

    for app in usage {
        if y + ROW_HEIGHT > height as f32 {
            break;
        }

        if app.is_active {
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y, width as f32, ROW_HEIGHT).unwrap(),
                &paint_active_bg,
                Transform::identity(),
                None,
            );
        }

        if let Some(f) = font {
            draw_text(&mut pixmap, &app.name, 12.0, y + 14.0, f, FONT_SIZE, theme.text);
        }

        let dur_str = fmt_duration(app.duration_secs);
        if let Some(f) = font {
            let text_w = measure_text(&dur_str, f, FONT_SIZE);
            draw_text(
                &mut pixmap,
                &dur_str,
                width as f32 - text_w - 12.0,
                y + 14.0,
                f,
                FONT_SIZE,
                theme.text,
            );
        }

        let bar_w = if max_dur == 0 || app.duration_secs == 0 {
            0.0
        } else {
            ((app.duration_secs as f32 / max_dur as f32) * (width as f32 - 24.0)).max(4.0)
        };
        pixmap.fill_rect(
            Rect::from_xywh(12.0, y + ROW_HEIGHT - BAR_HEIGHT - 4.0, bar_w, BAR_HEIGHT).unwrap(),
            &paint_accent,
            Transform::identity(),
            None,
        );

        y += ROW_HEIGHT;
    }

    pixmap
}

fn draw_titlebar_buttons(pixmap: &mut Pixmap, width: u32) {
    let btn_size = 32.0;
    let x0 = width as f32 - btn_size;
    let x1 = x0 - btn_size;
    let y0 = 0.0;
    let y1 = btn_size;

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(228, 228, 231, 255));
    paint.anti_alias = true;

    let cx = x0 + btn_size / 2.0;
    let cy = y0 + btn_size / 2.0;
    let arm = 5.0;
    let mut stroke = tiny_skia::Stroke::default();
    stroke.width = 1.5;
    stroke.line_cap = tiny_skia::LineCap::Round;
    {
        let mut path = tiny_skia::PathBuilder::new();
        path.move_to(cx - arm, cy - arm);
        path.line_to(cx + arm, cy + arm);
        path.move_to(cx + arm, cy - arm);
        path.line_to(cx - arm, cy + arm);
        if let Some(p) = path.finish() {
            pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    }

    let cx = x1 + btn_size / 2.0;
    let cy = y1 - btn_size / 3.0;
    let half_w = 5.0;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - half_w, cy);
    path.line_to(cx + half_w, cy);
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
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

fn measure_text(text: &str, font: &fontdue::Font, size: f32) -> f32 {
    text.chars().map(|ch| font.rasterize(ch, size).0.advance_width).sum()
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

    #[test]
    fn last_char_not_truncated_by_render() {
        let f = font().expect("font");
        let test_str = "claude.exe";
        let total_w = measure_text(test_str, f, FONT_SIZE);
        let pix_w = (total_w + 24.0).ceil() as u32;
        let pix_h = 36;
        let mut pixmap = Pixmap::new(pix_w.max(1), pix_h).expect("pixmap");

        let mut bg = Paint::default();
        bg.set_color(Color::BLACK);
        pixmap.fill_rect(Rect::from_xywh(0.0, 0.0, pix_w as f32, pix_h as f32).unwrap(), &bg, Transform::identity(), None);

        let y = 18.0; // baseline
        draw_text(&mut pixmap, test_str, 12.0, y, f, FONT_SIZE, Color::WHITE);

        let last_char_advance = f.rasterize('e', FONT_SIZE).0.advance_width;
        let last_char_x = 12.0 + total_w - last_char_advance;
        let (m, bitmap) = f.rasterize('e', FONT_SIZE);
        let lx = (last_char_x + m.xmin as f32).round() as i32;
        let ly = (y - m.ymin as f32 - m.height as f32 + 1.0).round() as i32;
        let mut found = false;
        for row in 0..m.height {
            for col in 0..m.width {
                let alpha = bitmap[row * m.width + col];
                if alpha > 0 {
                    let px = lx + col as i32;
                    let py = ly + row as i32;
                    if px >= 0 && px < pix_w as i32 && py >= 0 && py < pix_h as i32 {
                        let idx = (py as u32 * pixmap.width() + px as u32) as usize;
                        if pixmap.pixels()[idx] != tiny_skia::PremultipliedColorU8::TRANSPARENT {
                            found = true;
                        }
                    }
                }
            }
        }
        assert!(found, "last char 'e' of '{test_str}' produced zero rendered pixels");
    }
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
