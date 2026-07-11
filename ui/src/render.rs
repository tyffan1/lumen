use std::sync::OnceLock;

use chrono::NaiveDate;
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppView {
    List,
    Settings,
    Chart,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoveredTitleButton {
    None,
    Close,
    Minimize,
    Settings,
    Chart,
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

pub struct Theme {
    pub background: Color,
    pub text: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub separator: Color,
    pub active_indicator: Color,
    pub placeholder_icon: Color,
    pub hover_bg: Color,
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
            hover_bg: Color::from_rgba8(255, 255, 255, 12),
        }
    }
}

pub const TITLEBAR_HEIGHT: f32 = 32.0;
pub const SEARCH_HEIGHT: f32 = 28.0;
pub const ROW_HEIGHT: f32 = 56.0;
pub const DONUT_HEIGHT: f32 = 160.0;
pub const SCROLLBAR_W: f32 = 4.0;

/// Y-координата, с которой начинается скроллируемый список (под titlebar + search),
/// с учётом HiDPI scale.
pub fn list_top(scale: f32) -> f32 {
    (TITLEBAR_HEIGHT * scale).round() + (SEARCH_HEIGHT * scale).round() + (8.0 * scale).round()
}

pub fn scrollbar_x(width: u32, scale: f32) -> f32 {
    width as f32 - 20.0 * scale
}
const FONT_SIZE: f32 = 14.0;
const FONT_SIZE_DUR: f32 = 12.0;
const BAR_HEIGHT: f32 = 2.0;
const PADDING_X: f32 = 16.0;
const ICON_SIZE: f32 = 26.0;
const ICON_GAP: f32 = 8.0;
const INDICATOR_W: f32 = 2.0;

pub fn draw_frame(
    width: u32, height: u32, scale: f32,
    theme: &Theme, usage: &[AppUsage],
    button_hover: HoveredTitleButton,
    hovered_row: Option<usize>, hover_intensity: f32,
    search_query: &str, search_focused: bool, cursor_visible: bool,
    view: AppView, autostart: bool, show_seconds: bool, start_minimized: bool,
    idle_threshold_mins: u32, confirm_clear: bool,
    chart_data: &[(String, u64)], scroll_offset: f32, donut_data: &[(String, u64)],
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap alloc");

    let mut paint_bg = Paint::default();
    paint_bg.set_color(theme.background);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap(),
        &paint_bg, Transform::identity(), None,
    );

    #[cfg(target_os = "macos")]
    draw_titlebar_buttons_macos(&mut pixmap, width, scale, theme, button_hover);
    #[cfg(not(target_os = "macos"))]
    draw_titlebar_buttons(&mut pixmap, width, scale, theme, button_hover);

    let font_reg = font();
    let font_bld = font_bold();

    let th = (TITLEBAR_HEIGHT * scale).round();

    let mut paint_sep = Paint::default();
    paint_sep.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, th, width as f32, 1.0).unwrap(),
        &paint_sep, Transform::identity(), None,
    );

    match view {
        AppView::List => draw_list_content(
            &mut pixmap, width, height, theme, usage,
            hovered_row, hover_intensity, font_reg, font_bld,
            search_query, search_focused, cursor_visible, show_seconds,
            scroll_offset, donut_data, scale,
        ),
        AppView::Settings => draw_settings(
            &mut pixmap, width, height, theme, font_reg,
            autostart, show_seconds, start_minimized, idle_threshold_mins,
            hovered_row, hover_intensity, confirm_clear, scale,
        ),
        AppView::Chart => draw_chart(
            &mut pixmap, width, height, theme, font_reg, font_bld,
            chart_data, hovered_row, hover_intensity, scale,
        ),
    }

    pixmap
}

fn draw_list_content(
    pixmap: &mut Pixmap, width: u32, height: u32, theme: &Theme,
    usage: &[AppUsage], hovered_row: Option<usize>, hover_intensity: f32,
    font_reg: Option<&fontdue::Font>, font_bld: Option<&fontdue::Font>,
    search_query: &str, search_focused: bool, cursor_visible: bool,
    show_seconds: bool, scroll_offset: f32, donut_data: &[(String, u64)], scale: f32,
) {
    let px = PADDING_X * scale;
    let fs = FONT_SIZE * scale;
    let fsd = FONT_SIZE_DUR * scale;
    let th = (TITLEBAR_HEIGHT * scale).round();
    let sh = (SEARCH_HEIGHT * scale).round();
    let lt = th + sh + (8.0 * scale).round();
    let rh = (ROW_HEIGHT * scale).round();
    let is = (ICON_SIZE * scale).round();
    let ig = (ICON_GAP * scale).round();
    let iw = (INDICATOR_W * scale).max(1.0);
    let donut_h = DONUT_HEIGHT * scale;

    let viewport_bottom = (height as f32) - donut_h;
    let first_row = (scroll_offset / rh).floor() as usize;
    let fractional = scroll_offset - first_row as f32 * rh;
    let mut y = lt - fractional;
    let max_dur = usage.iter().map(|a| a.duration_secs).max().unwrap_or(0);
    let fade_zone = 36.0 * scale;

    for i in first_row..usage.len() {
        if y >= viewport_bottom { break; }
        let row_alpha = ((viewport_bottom - y) / fade_zone).clamp(0.0, 1.0);
        let app = &usage[i];

        if app.is_active {
            let mut paint_ind = Paint::default();
            paint_ind.set_color(theme.active_indicator);
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y + 4.0 * scale, iw, rh - 8.0 * scale).unwrap(),
                &paint_ind, Transform::identity(), None,
            );
        }

        if Some(i) == hovered_row && hover_intensity > 0.0 {
            let a = (theme.hover_bg.alpha() * hover_intensity * 255.0) as u8;
            let mut hp = Paint::default();
            hp.set_color(Color::from_rgba8(
                (theme.hover_bg.red() * 255.0) as u8,
                (theme.hover_bg.green() * 255.0) as u8,
                (theme.hover_bg.blue() * 255.0) as u8, a,
            ));
            hp.anti_alias = true;
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y, width as f32, rh).unwrap(),
                &hp, Transform::identity(), None,
            );
        }

        let icon_x = px;
        let icon_y = y + (rh - is) / 2.0;
        let icon_w = if let Some(ref rgba) = app.icon_rgba {
            blit_rgba(pixmap, rgba.as_slice(), app.icon_w, app.icon_h,
                      icon_x as u32, icon_y as u32, is as u32, is as u32);
            is
        } else {
            draw_placeholder_icon(pixmap, icon_x, icon_y, is, theme.placeholder_icon);
            is
        };

        let name_x = icon_x + icon_w + ig;
        let display_name = app.name.strip_suffix(".exe").unwrap_or(&app.name);
        if let Some(f) = font_bld {
            let base = text_baseline(y, rh, f, fs).unwrap_or(y + rh / 2.0 + fs * 0.35);
            draw_text(pixmap, display_name, name_x, base, f, fs, theme.text);
        }

        let dur_str = fmt_duration(app.duration_secs, show_seconds);
        if let Some(f) = font_reg {
            let text_w = measure_text(&dur_str, f, fsd);
            let base = text_baseline(y, rh, f, fsd).unwrap_or(y + rh / 2.0 + fsd * 0.35);
            draw_text(
                pixmap, &dur_str,
                (width as f32 - text_w - px - 16.0 * scale).max(name_x + 8.0 * scale),
                base, f, fsd, theme.text_dim,
            );
        }

        let bar_w = if max_dur == 0 || app.duration_secs == 0 {
            0.0
        } else {
            ((app.duration_secs as f32 / max_dur as f32) * (width as f32 - px * 2.0)).max(4.0 * scale)
        };
        if bar_w > 0.0 {
            let bh = (BAR_HEIGHT * scale).max(1.0);
            let bar_y = y + rh - bh - 6.0 * scale;
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = bh;
            stroke.line_cap = tiny_skia::LineCap::Round;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(px, bar_y + bh / 2.0);
            path.line_to(px + bar_w, bar_y + bh / 2.0);
            if let Some(p) = path.finish() {
                let mut paint = Paint::default();
                paint.set_color(theme.accent);
                paint.anti_alias = true;
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }

        if row_alpha < 1.0 {
            let a = ((1.0 - row_alpha) * 180.0) as u8;
            let mut dim = Paint::default();
            dim.set_color(Color::from_rgba8(0, 0, 0, a));
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y, width as f32, rh).unwrap(),
                &dim, Transform::identity(), None,
            );
        }

        y += rh;
    }

    // поле поиска (поверх списка)
    let search_y = th;
    if let Some(f) = font_reg {
        let base = text_baseline(search_y, sh, f, fsd).unwrap_or(search_y + sh / 2.0 + fsd * 0.35);
        let display = if search_query.is_empty() && !search_focused { "Search..." } else { search_query };
        let color = if search_query.is_empty() && !search_focused { theme.text_dim } else { theme.text };
        draw_text(pixmap, display, px, base, f, fsd, color);

        if !search_query.is_empty() {
            let clear_cx = (width as f32) - px - 10.0 * scale;
            let clear_cy = search_y + sh / 2.0;
            let arm = 3.0 * scale;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(clear_cx - arm, clear_cy - arm);
            path.line_to(clear_cx + arm, clear_cy + arm);
            path.move_to(clear_cx + arm, clear_cy - arm);
            path.line_to(clear_cx - arm, clear_cy + arm);
            if let Some(p) = path.finish() {
                let mut stroke = tiny_skia::Stroke::default();
                stroke.width = 1.0 * scale;
                stroke.line_cap = tiny_skia::LineCap::Round;
                let mut paint = Paint::default();
                paint.set_color(theme.text_dim);
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }

        if search_focused && cursor_visible {
            let text_w = measure_text(search_query, f, fsd);
            let cursor_x = px + text_w + 1.0 * scale;
            let cursor_y0 = base - 8.0 * scale;
            let cursor_y1 = base + 2.0 * scale;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(cursor_x, cursor_y0);
            path.line_to(cursor_x, cursor_y1);
            if let Some(p) = path.finish() {
                let mut stroke = tiny_skia::Stroke::default();
                stroke.width = 1.0 * scale;
                let mut paint = Paint::default();
                paint.set_color(theme.text);
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    let mut pu = Paint::default();
    pu.set_color(if search_focused { theme.accent } else { theme.separator });
    pixmap.fill_rect(
        Rect::from_xywh(px, th + sh - 1.0, (width as f32) - px * 2.0, 1.0).unwrap(),
        &pu, Transform::identity(), None,
    );

    draw_scrollbar(pixmap, width, lt, viewport_bottom, scroll_offset, usage.len(), scale);
    draw_fade_gradient(pixmap, width, viewport_bottom, theme, lt, scale);

    let mut donut_bg = Paint::default();
    donut_bg.set_color(theme.background);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, viewport_bottom, width as f32, (height as f32) - viewport_bottom).unwrap(),
        &donut_bg, Transform::identity(), None,
    );

    draw_donut(pixmap, width, height, theme, font_reg, font_bld, donut_data, scale);
}

fn draw_settings(
    pixmap: &mut Pixmap, width: u32, _height: u32, theme: &Theme,
    font: Option<&fontdue::Font>, autostart: bool, show_seconds: bool,
    start_minimized: bool, idle_threshold_mins: u32, hovered_row: Option<usize>,
    hover_intensity: f32, confirm_clear: bool, scale: f32,
) {
    if let Some(f) = font {
        let hdr_y = SETTINGS_TOP * scale;
        draw_text(pixmap, "Settings", PADDING_X * scale, hdr_y, f, FONT_SIZE * scale, theme.text);

        draw_settings_checkbox_row(pixmap, width, theme, f, 0, autostart, "Launch at startup", hovered_row, hover_intensity, scale);
        draw_settings_checkbox_row(pixmap, width, theme, f, 1, start_minimized, "Start minimized", hovered_row, hover_intensity, scale);
        draw_section_header(pixmap, width, theme, f, 2, "TRACKING", scale);
        draw_settings_idle_row(pixmap, width, theme, f, 2, idle_threshold_mins, hovered_row, hover_intensity, scale);
        draw_settings_checkbox_row(pixmap, width, theme, f, 3, show_seconds, "Show seconds", hovered_row, hover_intensity, scale);
        draw_section_header(pixmap, width, theme, f, 4, "DATA", scale);
        draw_settings_action_row(pixmap, width, theme, f, 4, "Clear history", hovered_row, hover_intensity, confirm_clear, scale);
        draw_settings_action_row(pixmap, width, theme, f, 5, "Open data folder", hovered_row, hover_intensity, false, scale);
        draw_section_separator(pixmap, width, theme, f, 6, scale);
        draw_settings_back_row(pixmap, width, theme, f, 6, hovered_row, hover_intensity, scale);
    }
}

const SETTINGS_TOP: f32 = 48.0;
const HEADER_OFFSET: f32 = 32.0;
const SRH: f32 = 56.0;
const SECTION_GAP: f32 = 28.0;

fn settings_gap_count(row: usize) -> usize {
    match row { 0 | 1 => 0, 2 | 3 => 1, 4 | 5 => 2, _ => 3 }
}

pub fn settings_row_y(row: usize, scale: f32) -> f32 {
    (SETTINGS_TOP + HEADER_OFFSET + row as f32 * SRH + settings_gap_count(row) as f32 * SECTION_GAP) * scale
}

pub fn settings_row_at(cy: f32, scale: f32) -> Option<usize> {
    for row in 0..7 {
        let y0 = settings_row_y(row, scale);
        if cy >= y0 && cy < y0 + SRH * scale { return Some(row); }
    }
    None
}

pub fn settings_idle_button_positions(width: u32, scale: f32) -> (f32, f32) {
    let right_x = width as f32 - PADDING_X * scale;
    let plus_cx = right_x - 14.0 * scale;
    let minus_cx = plus_cx - 48.0 * scale;
    (minus_cx, plus_cx)
}

pub fn settings_confirm_areas(width: u32, scale: f32) -> ((f32, f32), (f32, f32)) {
    let mid = width as f32 / 2.0;
    ((mid - 20.0 * scale, mid + 10.0 * scale), (mid + 15.0 * scale, mid + 50.0 * scale))
}

fn draw_row_hover(pixmap: &mut Pixmap, theme: &Theme, y: f32, intensity: f32, row_h: f32) {
    let a = (theme.hover_bg.alpha() * intensity * 255.0) as u8;
    if a == 0 { return; }
    let mut hp = Paint::default();
    hp.set_color(Color::from_rgba8(
        (theme.hover_bg.red() * 255.0) as u8,
        (theme.hover_bg.green() * 255.0) as u8,
        (theme.hover_bg.blue() * 255.0) as u8, a,
    ));
    hp.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(0.0, y, pixmap.width() as f32, row_h).unwrap(),
        &hp, Transform::identity(), None,
    );
}

fn draw_section_header(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, label: &str, scale: f32) {
    let gap_y = settings_row_y(row, scale) - SECTION_GAP * scale;
    let sep_y = gap_y + SECTION_GAP * scale / 2.0;
    let mut paint = Paint::default();
    paint.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(PADDING_X * scale, sep_y, (width as f32) - PADDING_X * scale * 2.0, 1.0).unwrap(),
        &paint, Transform::identity(), None,
    );
    let fsd = FONT_SIZE_DUR * scale;
    let base = text_baseline(gap_y, SECTION_GAP * scale, font, fsd).unwrap_or(gap_y + SECTION_GAP * scale / 2.0 + fsd * 0.35);
    draw_text(pixmap, label, PADDING_X * scale, base, font, fsd, theme.text_dim);
}

fn draw_section_separator(pixmap: &mut Pixmap, width: u32, theme: &Theme, _font: &fontdue::Font, row: usize, scale: f32) {
    let gap_y = settings_row_y(row, scale) - SECTION_GAP * scale;
    let sep_y = gap_y + SECTION_GAP * scale / 2.0;
    let mut paint = Paint::default();
    paint.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(PADDING_X * scale, sep_y, (width as f32) - PADDING_X * scale * 2.0, 1.0).unwrap(),
        &paint, Transform::identity(), None,
    );
}

fn draw_settings_checkbox_row(pixmap: &mut Pixmap, _width: u32, theme: &Theme, font: &fontdue::Font, row: usize, checked: bool, label: &str, hovered_row: Option<usize>, hover_intensity: f32, scale: f32) {
    let srh = SRH * scale;
    let y = settings_row_y(row, scale);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity, srh);
    }

    let px = PADDING_X * scale;
    let cb_x = px;
    let cb_y = y + (srh - 14.0 * scale) / 2.0;
    let cb_size = 14.0 * scale;

    let mut paint = Paint::default();
    paint.set_color(theme.text_dim);
    paint.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(cb_x, cb_y, cb_size, cb_size).unwrap(),
        &paint, Transform::identity(), None,
    );

    if checked {
        let inner = 2.0 * scale;
        let mut fill = Paint::default();
        fill.set_color(theme.accent);
        fill.anti_alias = true;
        pixmap.fill_rect(
            Rect::from_xywh(cb_x + inner, cb_y + inner, cb_size - inner * 2.0, cb_size - inner * 2.0).unwrap(),
            &fill, Transform::identity(), None,
        );
        let mut stroke = tiny_skia::Stroke::default();
        stroke.width = 2.0 * scale;
        stroke.line_cap = tiny_skia::LineCap::Round;
        let cx = cb_x + 2.0 * scale;
        let cy = cb_y + 2.0 * scale;
        let mut path = tiny_skia::PathBuilder::new();
        path.move_to(cx + 2.0 * scale, cy + 5.0 * scale);
        path.line_to(cx + 5.0 * scale, cy + 8.0 * scale);
        path.line_to(cx + 9.0 * scale, cy + 1.0 * scale);
        if let Some(p) = path.finish() {
            let mut gp = Paint::default();
            gp.set_color(Color::from_rgba8(255, 255, 255, 255));
            pixmap.stroke_path(&p, &gp, &stroke, Transform::identity(), None);
        }
    }

    let fs = FONT_SIZE * scale;
    let text_base = text_baseline(y, srh, font, fs).unwrap_or(y + srh / 2.0 + fs * 0.35);
    draw_text(pixmap, label, cb_x + cb_size + 10.0 * scale, text_base, font, fs, theme.text);
}

fn draw_settings_idle_row(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, value: u32, hovered_row: Option<usize>, hover_intensity: f32, scale: f32) {
    let srh = SRH * scale;
    let y = settings_row_y(row, scale);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity, srh);
    }

    let fs = FONT_SIZE * scale;
    let px = PADDING_X * scale;
    let text_base = text_baseline(y, srh, font, fs).unwrap_or(y + srh / 2.0 + fs * 0.35);
    draw_text(pixmap, "Idle threshold", px, text_base, font, fs, theme.text);

    let right_x = width as f32 - px;
    let plus_str = "+";
    let minus_str = "−";
    let btn_w = measure_text(plus_str, font, fs).max(measure_text(minus_str, font, fs));
    let val_str = format!("{} min", value);
    let val_w = measure_text(&val_str, font, fs);
    let plus_x = right_x - btn_w;
    let val_x = plus_x - 8.0 * scale - val_w;
    let minus_x = val_x - 8.0 * scale - btn_w;

    draw_text(pixmap, "+", plus_x, text_base, font, fs, theme.text);
    draw_text(pixmap, &val_str, val_x, text_base, font, fs, theme.text);
    draw_text(pixmap, "−", minus_x, text_base, font, fs, theme.text);
}

fn draw_settings_action_row(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, label: &str, hovered_row: Option<usize>, hover_intensity: f32, confirm_clear: bool, scale: f32) {
    let srh = SRH * scale;
    let px = PADDING_X * scale;
    let y = settings_row_y(row, scale);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity, srh);
    }

    let btn_x = px;
    let btn_y = y + 8.0 * scale;
    let btn_w = (width as f32) - px * 2.0;
    let btn_h = srh - 16.0 * scale;
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(255, 255, 255, 8));
    paint.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(btn_x, btn_y, btn_w, btn_h).unwrap(),
        &paint, Transform::identity(), None,
    );

    let fs = FONT_SIZE * scale;
    let text_base = text_baseline(y, srh, font, fs).unwrap_or(y + srh / 2.0 + fs * 0.35);

    if confirm_clear && label == "Clear history" {
        draw_text(pixmap, "Are you sure?", px, text_base, font, fs, theme.text);
        let mid = width as f32 / 2.0;
        draw_text(pixmap, "[Yes]", mid - 20.0 * scale, text_base, font, fs, theme.accent);
        draw_text(pixmap, "[Cancel]", mid + 15.0 * scale, text_base, font, fs, theme.text_dim);
    } else {
        draw_text(pixmap, label, px + 8.0 * scale, text_base, font, fs, theme.text);
    }
}

fn draw_settings_back_row(pixmap: &mut Pixmap, _width: u32, theme: &Theme, font: &fontdue::Font, row: usize, hovered_row: Option<usize>, hover_intensity: f32, scale: f32) {
    let srh = SRH * scale;
    let y = settings_row_y(row, scale);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity, srh);
    }
    let fs = FONT_SIZE * scale;
    let text_base = text_baseline(y, srh, font, fs).unwrap_or(y + srh / 2.0 + fs * 0.35);
    draw_text(pixmap, "←  Back", PADDING_X * scale, text_base, font, fs, theme.text_dim);
}

pub fn chart_back_y(height: u32, scale: f32) -> f32 {
    height as f32 - 24.0 * scale
}

fn draw_chart(
    pixmap: &mut Pixmap, width: u32, height: u32, theme: &Theme,
    font_reg: Option<&fontdue::Font>, font_bld: Option<&fontdue::Font>,
    data: &[(String, u64)], hovered_row: Option<usize>, hover_intensity: f32, scale: f32,
) {
    let px = PADDING_X * scale;
    let th = (TITLEBAR_HEIGHT * scale).round();
    let fs = FONT_SIZE * scale;
    let fsd = FONT_SIZE_DUR * scale;

    let header_y = th + 20.0 * scale;
    if let Some(f) = font_bld {
        draw_text(pixmap, "Daily Usage (7 days)", px, header_y, f, fs, theme.text);
    }

    let chart_top = header_y + 28.0 * scale;
    let chart_bottom = chart_back_y(height, scale) - 36.0 * scale;
    let chart_h = chart_bottom - chart_top;
    if chart_h <= 0.0 { return; }

    let max_val = data.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1);
    let n = data.len().max(1);
    let gap = 4.0 * scale;
    let chart_left = px + 4.0 * scale;
    let chart_right = (width as f32) - px - 4.0 * scale;
    let chart_w = chart_right - chart_left;
    let bar_w = (chart_w - gap * (n as f32 + 1.0)) / n as f32;

    for (i, (date_str, val)) in data.iter().enumerate() {
        let bar_h = (*val as f32 / max_val as f32) * (chart_h - 36.0 * scale);
        let bar_x = chart_left + gap + i as f32 * (bar_w + gap);
        let bar_y = chart_bottom - bar_h;

        let mut paint = Paint::default();
        paint.set_color(theme.accent);
        pixmap.fill_rect(
            Rect::from_xywh(bar_x, bar_y, bar_w, bar_h).unwrap(),
            &paint, Transform::identity(), None,
        );

        if *val > 0 {
            if let Some(f) = font_reg {
                let dur_str = fmt_duration(*val, false);
                let text_w = measure_text(&dur_str, f, fsd);
                let tx = bar_x + bar_w / 2.0 - text_w / 2.0;
                let ty = bar_y - 6.0 * scale;
                draw_text(pixmap, &dur_str, tx, ty, f, fsd, theme.text);
            }
        }

        if let Some(f) = font_reg {
            let label = if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                d.format("%a %m/%d").to_string()
            } else {
                date_str[5..].to_string()
            };
            let text_w = measure_text(&label, f, fsd);
            let tx = bar_x + bar_w / 2.0 - text_w / 2.0;
            let ty = chart_bottom + 2.0 * scale + fsd;
            draw_text(pixmap, &label, tx, ty, f, fsd, theme.text_dim);
        }
    }

    if data.is_empty() {
        if let Some(f) = font_reg {
            let msg = "No data yet";
            let text_w = measure_text(msg, f, fs);
            let tx = (width as f32 - text_w) / 2.0;
            let ty = (chart_top + chart_bottom) / 2.0;
            draw_text(pixmap, msg, tx, ty, f, fs, theme.text_dim);
        }
    }

    let back_y = chart_back_y(height, scale);
    if Some(0) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, back_y - 12.0 * scale, hover_intensity, SRH * scale);
    }
    if let Some(f) = font_bld {
        draw_text(pixmap, "←  Back", px, back_y, f, fs, theme.text_dim);
    }
}

fn donut_colors() -> [Color; 6] {
    [
        Color::from_rgba8( 74, 144, 217, 255),
        Color::from_rgba8(232, 145,  58, 255),
        Color::from_rgba8( 58, 180, 123, 255),
        Color::from_rgba8(155,  89, 182, 255),
        Color::from_rgba8(231,  76,  60, 255),
        Color::from_rgba8( 72,  72,  76, 255),
    ]
}

fn draw_scrollbar(pixmap: &mut Pixmap, width: u32, viewport_top: f32, viewport_bottom: f32, scroll_offset: f32, content_len: usize, scale: f32) {
    let rh = (ROW_HEIGHT * scale).round();
    let content_h = content_len as f32 * rh;
    let viewport_h = viewport_bottom - viewport_top;
    if content_h <= viewport_h { return; }
    let max_scroll = content_h - viewport_h;
    let sb_x = scrollbar_x(width, scale);
    let sb_w = SCROLLBAR_W * scale;
    let track_h = viewport_h;
    let thumb_h = (viewport_h / content_h * track_h).max(12.0 * scale);
    let thumb_y = viewport_top + (scroll_offset / max_scroll) * (track_h - thumb_h);

    let mut tp = Paint::default();
    tp.set_color(Color::from_rgba8(128, 128, 128, 30));
    pixmap.fill_rect(
        Rect::from_xywh(sb_x, viewport_top, sb_w, track_h).unwrap(),
        &tp, Transform::identity(), None,
    );

    let mut pp = Paint::default();
    pp.set_color(Color::from_rgba8(128, 128, 128, 120));
    pixmap.fill_rect(
        Rect::from_xywh(sb_x, thumb_y, sb_w, thumb_h).unwrap(),
        &pp, Transform::identity(), None,
    );
}

fn draw_fade_gradient(pixmap: &mut Pixmap, width: u32, viewport_bottom: f32, theme: &Theme, viewport_top: f32, scale: f32) {
    let fade_h = 36.0 * scale;
    let fade_top = viewport_bottom - fade_h;
    if fade_top < viewport_top { return; }
    let br = (theme.background.red() * 255.0) as u8;
    let bg = (theme.background.green() * 255.0) as u8;
    let bb = (theme.background.blue() * 255.0) as u8;
    let steps = fade_h.round().max(1.0) as i32;
    for i in 0..steps {
        let a = (i as f32 / fade_h * 255.0) as u8;
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(br, bg, bb, a));
        pixmap.fill_rect(
            Rect::from_xywh(0.0, fade_top + i as f32, width as f32, 1.0).unwrap(),
            &paint, Transform::identity(), None,
        );
    }
}

fn angle_pts(cx: f32, cy: f32, r: f32, start: f32, end: f32, steps: usize) -> Vec<(f32, f32)> {
    let mut pts = Vec::with_capacity(steps + 1);
    let da = (end - start) / steps as f32;
    for i in 0..=steps {
        let a = start + da * i as f32;
        pts.push((cx + a.cos() * r, cy + a.sin() * r));
    }
    pts
}

fn draw_donut_segment(pixmap: &mut Pixmap, cx: f32, cy: f32, ir: f32, or_: f32, start: f32, end: f32, color: Color) {
    let steps = 30;
    let outer = angle_pts(cx, cy, or_, start, end, steps);
    let inner = angle_pts(cx, cy, ir, start, end, steps);
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(outer[0].0, outer[0].1);
    for &(x, y) in &outer[1..] { path.line_to(x, y); }
    for &(x, y) in inner.iter().rev() { path.line_to(x, y); }
    path.close();
    if let Some(p) = path.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(&p, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_donut(
    pixmap: &mut Pixmap, width: u32, height: u32, theme: &Theme,
    _font_reg: Option<&fontdue::Font>, font_bld: Option<&fontdue::Font>,
    data: &[(String, u64)], scale: f32,
) {
    let Some(bld) = font_bld else { return };
    let donut_h = DONUT_HEIGHT * scale;
    let fs = FONT_SIZE * scale;
    let fsd = FONT_SIZE_DUR * scale;
    let px = PADDING_X * scale;

    let donut_top = (height as f32) - donut_h;
    let cx = width as f32 / 2.0;
    let cy = donut_top + donut_h * 0.45;
    let or_ = (donut_h * 0.5 - 18.0 * scale).max(20.0 * scale).min(80.0 * scale);
    let ir = or_ * 0.42;

    let total: u64 = data.iter().map(|(_, v)| v).sum();
    let total_str = format!("Today — {}", fmt_duration(total, false));
    let tw = measure_text(&total_str, bld, fs);
    draw_text(pixmap, &total_str, (width as f32 - tw) / 2.0, donut_top - 2.0 * scale, bld, fs, theme.text);

    if data.is_empty() || total == 0 {
        let msg = "No activity yet";
        let mw = measure_text(msg, bld, fs);
        draw_text(pixmap, msg, (width as f32 - mw) / 2.0, cy, bld, fs, theme.text_dim);
        let mut path = tiny_skia::PathBuilder::new();
        path.push_circle(cx, cy, or_);
        if let Some(p) = path.finish() {
            let mut paint = Paint::default();
            paint.set_color(theme.separator);
            let mut stroke = tiny_skia::Stroke::default();
            stroke.width = 2.0 * scale;
            pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
        return;
    }

    let mut sorted: Vec<&(String, u64)> = data.iter().filter(|(_, v)| *v > 0).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    let n = sorted.len().min(5);
    let other_sum: u64 = if sorted.len() > 5 { sorted[5..].iter().map(|(_, v)| v).sum() } else { 0 };

    let colors = donut_colors();
    let mut start_a = -90.0_f32.to_radians();
    for i in 0..n {
        let angle = (sorted[i].1 as f32 / total as f32) * 360.0_f32.to_radians();
        let end_a = start_a + angle;
        draw_donut_segment(pixmap, cx, cy, ir, or_, start_a, end_a, colors[i]);
        start_a = end_a;
    }
    if other_sum > 0 {
        let angle = (other_sum as f32 / total as f32) * 360.0_f32.to_radians();
        let end_a = start_a + angle;
        draw_donut_segment(pixmap, cx, cy, ir, or_, start_a, end_a, colors[5]);
    }

    let mut hole = tiny_skia::PathBuilder::new();
    hole.push_circle(cx, cy, ir);
    if let Some(p) = hole.finish() {
        let mut paint = Paint::default();
        paint.set_color(theme.background);
        paint.anti_alias = true;
        pixmap.fill_path(&p, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }

    let dot_r = 3.0 * scale;
    let lgap = 8.0 * scale;
    let legend_y1 = donut_top + donut_h * 0.78 + 4.0 * scale;
    let mut all_items: Vec<(&str, &Color)> = sorted[..n].iter().enumerate()
        .map(|(i, (name, _))| (name.strip_suffix(".exe").unwrap_or(name), &colors[i]))
        .collect();
    if other_sum > 0 { all_items.push(("Other", &colors[5])); }

    let mut draw_row = |y: f32, slice: &[(&str, &Color)]| {
        let mut lx = px;
        for &(label, color) in slice {
            let dot_cx = lx + dot_r;
            let dot_cy = y - dot_r;
            let mut dp = Paint::default();
            dp.set_color(*color);
            dp.anti_alias = true;
            let mut circle = tiny_skia::PathBuilder::new();
            circle.push_circle(dot_cx, dot_cy, dot_r);
            if let Some(c) = circle.finish() {
                pixmap.fill_path(&c, &dp, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }
            let tx = lx + dot_r * 2.0 + 4.0 * scale;
            draw_text(pixmap, label, tx, y, bld, fsd, theme.text);
            lx = tx + measure_text(label, bld, fsd) + lgap;
        }
    };

    let first = all_items.len().min(3);
    if first > 0 { draw_row(legend_y1, &all_items[..first]); }
    if all_items.len() > first { draw_row(legend_y1 + 18.0 * scale, &all_items[first..]); }
}

#[cfg(target_os = "macos")]
fn draw_titlebar_buttons_macos(pixmap: &mut Pixmap, width: u32, scale: f32, theme: &Theme, hover: HoveredTitleButton) {
    let btn_size = 32.0 * scale;
    let x1 = width as f32 - btn_size;       // settings
    let x0 = x1 - btn_size;                 // chart

    let mut bg_paint = Paint::default();
    bg_paint.anti_alias = true;
    match hover {
        HoveredTitleButton::Settings => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(Rect::from_xywh(x1, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        HoveredTitleButton::Chart => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(Rect::from_xywh(x0, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        _ => {}
    }

    let mut paint = Paint::default();
    paint.set_color(theme.text_dim);
    paint.anti_alias = true;
    // x0 (left) — chart (статистика), x1 (right) — gear (настройки)
    draw_chart_icon(pixmap, x0, btn_size, &paint);
    draw_gear_icon(pixmap, x1, btn_size, &paint, scale);
}

#[cfg(not(target_os = "macos"))]
fn draw_titlebar_buttons(pixmap: &mut Pixmap, width: u32, scale: f32, theme: &Theme, hover: HoveredTitleButton) {
    let btn_size = 32.0 * scale;
    let x3 = width as f32 - btn_size;       // close
    let x2 = x3 - btn_size;                 // minimize
    let x1 = x2 - btn_size;                 // settings
    let x0 = x1 - btn_size;                 // chart

    let mut bg_paint = Paint::default();
    bg_paint.anti_alias = true;
    match hover {
        HoveredTitleButton::Close => {
            bg_paint.set_color(Color::from_rgba8(196, 43, 43, 255));
            pixmap.fill_rect(Rect::from_xywh(x3, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        HoveredTitleButton::Minimize => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(Rect::from_xywh(x2, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        HoveredTitleButton::Settings => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(Rect::from_xywh(x1, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        HoveredTitleButton::Chart => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(Rect::from_xywh(x0, 0.0, btn_size, btn_size).unwrap(), &bg_paint, Transform::identity(), None);
        }
        HoveredTitleButton::None => {}
    }

    let mut paint = Paint::default();
    paint.set_color(theme.text_dim);
    paint.anti_alias = true;

    let mut stroke_solid = tiny_skia::Stroke::default();
    stroke_solid.width = 1.0 * scale;
    stroke_solid.line_cap = tiny_skia::LineCap::Round;

    // close
    let cx = x3 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let arm = 3.5 * scale;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - arm, cy - arm);
    path.line_to(cx + arm, cy + arm);
    path.move_to(cx + arm, cy - arm);
    path.line_to(cx - arm, cy + arm);
    if let Some(p) = path.finish() { pixmap.stroke_path(&p, &paint, &stroke_solid, Transform::identity(), None); }

    // minimize
    let cx = x2 + btn_size / 2.0;
    let cy = btn_size * 0.65;
    let half_w = 4.5 * scale;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - half_w, cy);
    path.line_to(cx + half_w, cy);
    if let Some(p) = path.finish() { pixmap.stroke_path(&p, &paint, &stroke_solid, Transform::identity(), None); }

    draw_gear_icon(pixmap, x1, btn_size, &paint, scale);
    draw_chart_icon(pixmap, x0, btn_size, &paint);
}

fn draw_gear_icon(pixmap: &mut Pixmap, x0: f32, btn_size: f32, paint: &Paint, scale: f32) {
    let cx = x0 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let r = 5.0 * scale;
    let spoke_len = 3.0 * scale;

    let mut circ = tiny_skia::PathBuilder::new();
    circ.push_circle(cx, cy, r);
    if let Some(p) = circ.finish() {
        let mut stroke = tiny_skia::Stroke::default();
        stroke.width = 1.5 * scale;
        pixmap.stroke_path(&p, paint, &stroke, Transform::identity(), None);
    }
    let mut path = tiny_skia::PathBuilder::new();
    for angle_deg in [0.0, 90.0, 180.0, 270.0] {
        let rad = angle_deg * std::f32::consts::PI / 180.0;
        let (dx, dy) = (rad.cos(), rad.sin());
        path.move_to(cx + r * dx, cy + r * dy);
        path.line_to(cx + (r + spoke_len) * dx, cy + (r + spoke_len) * dy);
    }
    if let Some(p) = path.finish() {
        let mut stroke = tiny_skia::Stroke::default();
        stroke.width = 1.5 * scale;
        stroke.line_cap = tiny_skia::LineCap::Round;
        pixmap.stroke_path(&p, paint, &stroke, Transform::identity(), None);
    }
}

fn draw_chart_icon(pixmap: &mut Pixmap, x0: f32, btn_size: f32, paint: &Paint) {
    let cx = x0 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let bar_w = 4.0;
    let bar_gap = 4.0;
    let heights = [12.0, 22.0, 16.0];
    let starts_x = cx - (bar_w * 3.0 + bar_gap * 2.0) / 2.0;
    for (i, &h) in heights.iter().enumerate() {
        let bx = starts_x + i as f32 * (bar_w + bar_gap);
        let by = cy + 8.0 - h;
        pixmap.fill_rect(Rect::from_xywh(bx, by, bar_w, h).unwrap(), paint, Transform::identity(), None);
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

fn fmt_duration(secs: u64, show_seconds: bool) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        if show_seconds { format!("{}h {:02}m {:02}s", h, m, s) } else { format!("{}h {:02}m", h, m) }
    } else if m > 0 {
        if show_seconds { format!("{}m {:02}s", m, s) } else { format!("{}m", m) }
    } else if show_seconds { format!("{}s", s) } else { "0m".to_string() }
}

fn font() -> Option<&'static fontdue::Font> {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        #[cfg(target_os = "windows")]
        let data = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")
            .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\arial.ttf")).ok()?;
        #[cfg(target_os = "macos")]
        let data = std::fs::read("/System/Library/Fonts/SFNS.ttf")
            .or_else(|_| std::fs::read("/System/Library/Fonts/Helvetica.ttc"))
            .or_else(|_| std::fs::read("/Library/Fonts/Arial.ttf")).ok()?;
        #[cfg(target_os = "linux")]
        let data = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"))
            .or_else(|_| std::fs::read("/usr/share/fonts/TTF/DejaVuSans.ttf")).ok()?;
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let data = None?;
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    }).as_ref()
}

fn font_bold() -> Option<&'static fontdue::Font> {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        #[cfg(target_os = "windows")]
        let data = std::fs::read("C:\\Windows\\Fonts\\segoeuib.ttf")
            .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")).ok()?;
        #[cfg(target_os = "macos")]
        let data = std::fs::read("/System/Library/Fonts/SFNS.ttf")
            .or_else(|_| std::fs::read("/System/Library/Fonts/HelveticaNeue.ttc"))
            .or_else(|_| std::fs::read("/Library/Fonts/Arial Bold.ttf")).ok()?;
        #[cfg(target_os = "linux")]
        let data = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf")
            .or_else(|_| std::fs::read("/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf")).ok()?;
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let data = None?;
        fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
    }).as_ref()
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
                if alpha == 0 { continue; }
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
    let painted = if src_w == dst_w && src_h == dst_h { rgba.to_vec() } else { resize_rgba_bilinear(rgba, src_w, src_h, dst_w, dst_h) };
    let pw = pixmap.width();
    let ph = pixmap.height();
    for row in 0..dst_h {
        for col in 0..dst_w {
            let src_idx = ((row * dst_w + col) * 4) as usize;
            let a = painted[src_idx + 3] as u32;
            if a == 0 { continue; }
            let px = dst_x + col;
            let py = dst_y + row;
            if px >= pw || py >= ph { continue; }
            let dst_idx = (py * pw + px) as usize;
            let dst_pixel = &mut pixmap.pixels_mut()[dst_idx];
            let sr = painted[src_idx] as u32;
            let sg = painted[src_idx + 1] as u32;
            let sb = painted[src_idx + 2] as u32;
            if a >= 254 {
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(sr as u8, sg as u8, sb as u8, 255) { *dst_pixel = p; }
            } else {
                let inv_a = 255u32.wrapping_sub(a);
                let dr = dst_pixel.red() as u32;
                let dg = dst_pixel.green() as u32;
                let db = dst_pixel.blue() as u32;
                let r = ((a * sr + inv_a * dr) / 255) as u8;
                let g = ((a * sg + inv_a * dg) / 255) as u8;
                let b = ((a * sb + inv_a * db) / 255) as u8;
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255) { *dst_pixel = p; }
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
                let c00 = si(x0, y0, ch);
                let c10 = si(x1, y0, ch);
                let c01 = si(x0, y1, ch);
                let c11 = si(x1, y1, ch);
                let top = c00 + (c10 - c00) * fx;
                let bot = c01 + (c11 - c01) * fx;
                let val = (top + (bot - top) * fy).round().max(0.0).min(255.0) as u8;
                dst[((dy * dst_w + dx) * 4 + ch) as usize] = val;
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
            eprintln!("ch='{ch}' xmin={} ymin={} w={} h={} advance={} bounds={{{:?}}}",
                      m.xmin, m.ymin, m.width, m.height, m.advance_width, m.bounds);
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
        assert_eq!(fmt_duration(0, false), "0m");
        assert_eq!(fmt_duration(0, true), "0s");
        assert_eq!(fmt_duration(59, true), "59s");
        assert_eq!(fmt_duration(60, true), "1m 00s");
        assert_eq!(fmt_duration(61, true), "1m 01s");
        assert_eq!(fmt_duration(61, false), "1m");
        assert_eq!(fmt_duration(3600, true), "1h 00m 00s");
        assert_eq!(fmt_duration(3600, false), "1h 00m");
        assert_eq!(fmt_duration(3661, true), "1h 01m 01s");
        assert_eq!(fmt_duration(3661, false), "1h 01m");
    }
}