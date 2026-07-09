use std::sync::OnceLock;

use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppView {
    List,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoveredTitleButton {
    None,
    Close,
    Minimize,
    Settings,
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

const TITLEBAR_HEIGHT: f32 = 32.0;
const SEARCH_HEIGHT: f32 = 28.0;
const LIST_TOP: f32 = TITLEBAR_HEIGHT + SEARCH_HEIGHT + 8.0;
const ROW_HEIGHT: f32 = 56.0;
const FONT_SIZE: f32 = 14.0;
const FONT_SIZE_DUR: f32 = 12.0;
const BAR_HEIGHT: f32 = 2.0;
const PADDING_X: f32 = 16.0;
const ICON_SIZE: f32 = 20.0;
const ICON_GAP: f32 = 8.0;
const INDICATOR_W: f32 = 2.0;

pub fn draw_frame(width: u32, height: u32, theme: &Theme, usage: &[AppUsage], button_hover: HoveredTitleButton, hovered_row: Option<usize>, hover_intensity: f32, search_query: &str, search_focused: bool, cursor_visible: bool, view: AppView, autostart: bool, show_seconds: bool, start_minimized: bool, idle_threshold_mins: u32, confirm_clear: bool) -> Pixmap {
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

    let font_reg = font();
    let font_bld = font_bold();

    // суммарное время слева в titlebar
    if let Some(f) = font_reg {
        let total_secs: u64 = usage.iter().map(|a| a.duration_secs).sum();
        let base = text_baseline(0.0, TITLEBAR_HEIGHT, f, FONT_SIZE_DUR)
            .unwrap_or(TITLEBAR_HEIGHT / 2.0 + FONT_SIZE_DUR * 0.35);
        draw_text(&mut pixmap, &fmt_duration(total_secs, false), PADDING_X, base, f, FONT_SIZE_DUR, theme.text_dim);
    }

    let mut paint_sep = Paint::default();
    paint_sep.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(0.0, TITLEBAR_HEIGHT, width as f32, 1.0).unwrap(),
        &paint_sep,
        Transform::identity(),
        None,
    );

    match view {
        AppView::List => draw_list_content(&mut pixmap, width, height, theme, usage, hovered_row, hover_intensity, font_reg, font_bld, search_query, search_focused, cursor_visible, show_seconds),
        AppView::Settings => draw_settings(&mut pixmap, width, height, theme, font_reg, autostart, show_seconds, start_minimized, idle_threshold_mins, hovered_row, hover_intensity, confirm_clear),
    }

    pixmap
}

fn draw_list_content(pixmap: &mut Pixmap, width: u32, height: u32, theme: &Theme, usage: &[AppUsage], hovered_row: Option<usize>, hover_intensity: f32, font_reg: Option<&fontdue::Font>, font_bld: Option<&fontdue::Font>, search_query: &str, search_focused: bool, cursor_visible: bool, show_seconds: bool) {
    // поле поиска
    let search_y = TITLEBAR_HEIGHT;
    if let Some(f) = font_reg {
        let base = text_baseline(search_y, SEARCH_HEIGHT, f, FONT_SIZE_DUR)
            .unwrap_or(search_y + SEARCH_HEIGHT / 2.0 + FONT_SIZE_DUR * 0.35);
        let display = if search_query.is_empty() && !search_focused {
            "Search..."
        } else {
            search_query
        };
        let color = if search_query.is_empty() && !search_focused {
            theme.text_dim
        } else {
            theme.text
        };
        draw_text(pixmap, display, PADDING_X, base, f, FONT_SIZE_DUR, color);

        // крестик очистки ×
        if !search_query.is_empty() {
            let clear_cx = (width as f32) - PADDING_X - 10.0;
            let clear_cy = search_y + SEARCH_HEIGHT / 2.0;
            let arm = 3.0;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(clear_cx - arm, clear_cy - arm);
            path.line_to(clear_cx + arm, clear_cy + arm);
            path.move_to(clear_cx + arm, clear_cy - arm);
            path.line_to(clear_cx - arm, clear_cy + arm);
            if let Some(p) = path.finish() {
                let mut stroke = tiny_skia::Stroke::default();
                stroke.width = 1.0;
                stroke.line_cap = tiny_skia::LineCap::Round;
                let mut paint = Paint::default();
                paint.set_color(theme.text_dim);
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }

        // мигающий курсор
        if search_focused && cursor_visible {
            let text_w = measure_text(search_query, f, FONT_SIZE_DUR);
            let cursor_x = PADDING_X + text_w + 1.0;
            let cursor_y0 = base - 8.0;
            let cursor_y1 = base + 2.0;
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(cursor_x, cursor_y0);
            path.line_to(cursor_x, cursor_y1);
            if let Some(p) = path.finish() {
                let mut stroke = tiny_skia::Stroke::default();
                stroke.width = 1.0;
                let mut paint = Paint::default();
                paint.set_color(theme.text);
                pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
            }
        }
    }

    // нижняя линия поля поиска
    let underline_y = TITLEBAR_HEIGHT + SEARCH_HEIGHT - 1.0;
    let mut pu = Paint::default();
    pu.set_color(if search_focused { theme.accent } else { theme.separator });
    pixmap.fill_rect(
        Rect::from_xywh(PADDING_X, underline_y, (width as f32) - PADDING_X * 2.0, 1.0).unwrap(),
        &pu,
        Transform::identity(),
        None,
    );

    let mut y = LIST_TOP;
    let max_dur = usage.iter().map(|a| a.duration_secs).max().unwrap_or(0);

    for (i, app) in usage.iter().enumerate() {
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

        // hover-подсветка строки
        if Some(i) == hovered_row && hover_intensity > 0.0 {
            let a = (theme.hover_bg.alpha() * hover_intensity * 255.0) as u8;
            let mut hp = Paint::default();
            hp.set_color(Color::from_rgba8(
                (theme.hover_bg.red() * 255.0) as u8,
                (theme.hover_bg.green() * 255.0) as u8,
                (theme.hover_bg.blue() * 255.0) as u8,
                a,
            ));
            hp.anti_alias = true;
            pixmap.fill_rect(
                Rect::from_xywh(0.0, y, width as f32, ROW_HEIGHT).unwrap(),
                &hp,
                Transform::identity(),
                None,
            );
        }

        let icon_x = PADDING_X;
        let icon_y = y + (ROW_HEIGHT - ICON_SIZE) / 2.0;
        let icon_w = if let Some(ref rgba) = app.icon_rgba {
            blit_rgba(
                pixmap,
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
            draw_placeholder_icon(pixmap, icon_x, icon_y, ICON_SIZE, theme.placeholder_icon);
            ICON_SIZE
        };

        let name_x = icon_x + icon_w + ICON_GAP;
        let display_name = app.name.strip_suffix(".exe").unwrap_or(&app.name);
        if let Some(f) = font_bld {
            let base = text_baseline(y, ROW_HEIGHT, f, FONT_SIZE)
                .unwrap_or(y + ROW_HEIGHT / 2.0 + FONT_SIZE * 0.35);
            draw_text(pixmap, display_name, name_x, base, f, FONT_SIZE, theme.text);
        }

        let dur_str = fmt_duration(app.duration_secs, show_seconds);
        if let Some(f) = font_reg {
            let text_w = measure_text(&dur_str, f, FONT_SIZE_DUR);
            let base = text_baseline(y, ROW_HEIGHT, f, FONT_SIZE_DUR)
                .unwrap_or(y + ROW_HEIGHT / 2.0 + FONT_SIZE_DUR * 0.35);
            draw_text(
                pixmap,
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
}

fn draw_settings(pixmap: &mut Pixmap, width: u32, _height: u32, theme: &Theme, font: Option<&fontdue::Font>, autostart: bool, show_seconds: bool, start_minimized: bool, idle_threshold_mins: u32, hovered_row: Option<usize>, hover_intensity: f32, confirm_clear: bool) {
    if let Some(f) = font {
        let hdr_y = SETTINGS_TOP;
        draw_text(pixmap, "Settings", PADDING_X, hdr_y, f, FONT_SIZE, theme.text);

        draw_settings_checkbox_row(pixmap, width, theme, f, 0, autostart, "Launch at startup", hovered_row, hover_intensity);
        draw_settings_checkbox_row(pixmap, width, theme, f, 1, start_minimized, "Start minimized", hovered_row, hover_intensity);

        draw_section_header(pixmap, width, theme, f, 2, "TRACKING");

        draw_settings_idle_row(pixmap, width, theme, f, 2, idle_threshold_mins, hovered_row, hover_intensity);
        draw_settings_checkbox_row(pixmap, width, theme, f, 3, show_seconds, "Show seconds", hovered_row, hover_intensity);

        draw_section_header(pixmap, width, theme, f, 4, "DATA");

        draw_settings_action_row(pixmap, width, theme, f, 4, "Clear history", hovered_row, hover_intensity, confirm_clear);
        draw_settings_action_row(pixmap, width, theme, f, 5, "Open data folder", hovered_row, hover_intensity, false);

        draw_section_separator(pixmap, width, theme, f, 6);
        draw_settings_back_row(pixmap, width, theme, f, 6, hovered_row, hover_intensity);
    }
}

const SETTINGS_TOP: f32 = 48.0;
const HEADER_OFFSET: f32 = 32.0;
const SRH: f32 = 56.0;
const SECTION_GAP: f32 = 28.0;

fn settings_gap_count(row: usize) -> usize {
    match row {
        0 | 1 => 0,
        2 | 3 => 1,
        4 | 5 => 2,
        _ => 3,
    }
}

pub fn settings_row_y(row: usize) -> f32 {
    SETTINGS_TOP + HEADER_OFFSET + row as f32 * SRH + settings_gap_count(row) as f32 * SECTION_GAP
}

pub fn settings_row_at(cy: f32) -> Option<usize> {
    for row in 0..7 {
        let y0 = settings_row_y(row);
        if cy >= y0 && cy < y0 + SRH {
            return Some(row);
        }
    }
    None
}

pub fn settings_idle_button_positions(width: u32) -> (f32, f32) {
    let right_x = width as f32 - PADDING_X;
    let plus_cx = right_x - 14.0;
    let minus_cx = plus_cx - 48.0;
    (minus_cx, plus_cx)
}

pub fn settings_confirm_areas(width: u32) -> ((f32, f32), (f32, f32)) {
    let mid = width as f32 / 2.0;
    ( (mid - 20.0, mid + 10.0), (mid + 15.0, mid + 50.0) )
}

fn draw_row_hover(pixmap: &mut Pixmap, theme: &Theme, y: f32, intensity: f32) {
    let a = (theme.hover_bg.alpha() * intensity * 255.0) as u8;
    if a == 0 { return; }
    let mut hp = Paint::default();
    hp.set_color(Color::from_rgba8(
        (theme.hover_bg.red() * 255.0) as u8,
        (theme.hover_bg.green() * 255.0) as u8,
        (theme.hover_bg.blue() * 255.0) as u8,
        a,
    ));
    hp.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(0.0, y, pixmap.width() as f32, SRH).unwrap(),
        &hp,
        Transform::identity(),
        None,
    );
}

fn draw_section_header(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, label: &str) {
    let gap_y = settings_row_y(row) - SECTION_GAP;
    let sep_y = gap_y + SECTION_GAP / 2.0;
    let mut paint = Paint::default();
    paint.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(PADDING_X, sep_y, (width as f32) - PADDING_X * 2.0, 1.0).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );
    let base = text_baseline(gap_y, SECTION_GAP, font, FONT_SIZE_DUR)
        .unwrap_or(gap_y + SECTION_GAP / 2.0 + FONT_SIZE_DUR * 0.35);
    draw_text(pixmap, label, PADDING_X, base, font, FONT_SIZE_DUR, theme.text_dim);
}

fn draw_section_separator(pixmap: &mut Pixmap, width: u32, theme: &Theme, _font: &fontdue::Font, row: usize) {
    let gap_y = settings_row_y(row) - SECTION_GAP;
    let sep_y = gap_y + SECTION_GAP / 2.0;
    let mut paint = Paint::default();
    paint.set_color(theme.separator);
    pixmap.fill_rect(
        Rect::from_xywh(PADDING_X, sep_y, (width as f32) - PADDING_X * 2.0, 1.0).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );
}

fn draw_settings_checkbox_row(pixmap: &mut Pixmap, _width: u32, theme: &Theme, font: &fontdue::Font, row: usize, checked: bool, label: &str, hovered_row: Option<usize>, hover_intensity: f32) {
    let y = settings_row_y(row);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity);
    }

    let cb_x = PADDING_X;
    let cb_y = y + (SRH - 14.0) / 2.0;
    let cb_size = 14.0;

    let mut paint = Paint::default();
    paint.set_color(theme.text_dim);
    paint.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(cb_x, cb_y, cb_size, cb_size).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );

    if checked {
        let inner = 2.0;
        let mut fill = Paint::default();
        fill.set_color(theme.accent);
        fill.anti_alias = true;
        pixmap.fill_rect(
            Rect::from_xywh(cb_x + inner, cb_y + inner, cb_size - inner * 2.0, cb_size - inner * 2.0).unwrap(),
            &fill,
            Transform::identity(),
            None,
        );
        let mut stroke = tiny_skia::Stroke::default();
        stroke.width = 2.0;
        stroke.line_cap = tiny_skia::LineCap::Round;
        let cx = cb_x + 2.0;
        let cy = cb_y + 2.0;
        let mut path = tiny_skia::PathBuilder::new();
        path.move_to(cx + 2.0, cy + 5.0);
        path.line_to(cx + 5.0, cy + 8.0);
        path.line_to(cx + 9.0, cy + 1.0);
        if let Some(p) = path.finish() {
            let mut gp = Paint::default();
            gp.set_color(Color::from_rgba8(255, 255, 255, 255));
            pixmap.stroke_path(&p, &gp, &stroke, Transform::identity(), None);
        }
    }

    let text_base = text_baseline(y, SRH, font, FONT_SIZE)
        .unwrap_or(y + SRH / 2.0 + FONT_SIZE * 0.35);
    draw_text(pixmap, label, cb_x + cb_size + 10.0, text_base, font, FONT_SIZE, theme.text);
}

fn draw_settings_idle_row(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, value: u32, hovered_row: Option<usize>, hover_intensity: f32) {
    let y = settings_row_y(row);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity);
    }

    let text_base = text_baseline(y, SRH, font, FONT_SIZE)
        .unwrap_or(y + SRH / 2.0 + FONT_SIZE * 0.35);
    draw_text(pixmap, "Idle threshold", PADDING_X, text_base, font, FONT_SIZE, theme.text);

    let w = width as f32;
    let right_x = w - PADDING_X;
    let plus_str = "+";
    let minus_str = "−";
    let btn_w = measure_text(plus_str, font, FONT_SIZE).max(measure_text(minus_str, font, FONT_SIZE));
    let val_str = format!("{} min", value);
    let val_w = measure_text(&val_str, font, FONT_SIZE);

    let plus_x = right_x - btn_w;
    let val_x = plus_x - 8.0 - val_w;
    let minus_x = val_x - 8.0 - btn_w;

    draw_text(pixmap, "+", plus_x, text_base, font, FONT_SIZE, theme.text);
    draw_text(pixmap, &val_str, val_x, text_base, font, FONT_SIZE, theme.text);
    draw_text(pixmap, "−", minus_x, text_base, font, FONT_SIZE, theme.text);
}

fn draw_settings_action_row(pixmap: &mut Pixmap, width: u32, theme: &Theme, font: &fontdue::Font, row: usize, label: &str, hovered_row: Option<usize>, hover_intensity: f32, confirm_clear: bool) {
    let y = settings_row_y(row);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity);
    }

    let btn_x = PADDING_X;
    let btn_y = y + 8.0;
    let btn_w = (width as f32) - PADDING_X * 2.0;
    let btn_h = SRH - 16.0;
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(255, 255, 255, 8));
    paint.anti_alias = true;
    pixmap.fill_rect(
        Rect::from_xywh(btn_x, btn_y, btn_w, btn_h).unwrap(),
        &paint,
        Transform::identity(),
        None,
    );

    let text_base = text_baseline(y, SRH, font, FONT_SIZE)
        .unwrap_or(y + SRH / 2.0 + FONT_SIZE * 0.35);

    if confirm_clear && label == "Clear history" {
        draw_text(pixmap, "Are you sure?", PADDING_X, text_base, font, FONT_SIZE, theme.text);
        let mid = width as f32 / 2.0;
        draw_text(pixmap, "[Yes]", mid - 20.0, text_base, font, FONT_SIZE, theme.accent);
        draw_text(pixmap, "[Cancel]", mid + 15.0, text_base, font, FONT_SIZE, theme.text_dim);
    } else {
        draw_text(pixmap, label, PADDING_X + 8.0, text_base, font, FONT_SIZE, theme.text);
    }
}

fn draw_settings_back_row(pixmap: &mut Pixmap, _width: u32, theme: &Theme, font: &fontdue::Font, row: usize, hovered_row: Option<usize>, hover_intensity: f32) {
    let y = settings_row_y(row);
    if Some(row) == hovered_row && hover_intensity > 0.0 {
        draw_row_hover(pixmap, theme, y, hover_intensity);
    }

    let text_base = text_baseline(y, SRH, font, FONT_SIZE)
        .unwrap_or(y + SRH / 2.0 + FONT_SIZE * 0.35);
    draw_text(pixmap, "←  Back", PADDING_X, text_base, font, FONT_SIZE, theme.text_dim);
}

fn draw_titlebar_buttons(pixmap: &mut Pixmap, width: u32, theme: &Theme, hover: HoveredTitleButton) {
    let btn_size = 32.0;
    let x2 = width as f32 - btn_size;       // close
    let x1 = x2 - btn_size;                 // minimize
    let x0 = x1 - btn_size;                 // settings

    // фон при наведении
    let mut bg_paint = Paint::default();
    bg_paint.anti_alias = true;

    match hover {
        HoveredTitleButton::Close => {
            bg_paint.set_color(Color::from_rgba8(196, 43, 43, 255));
            pixmap.fill_rect(
                Rect::from_xywh(x2, 0.0, btn_size, btn_size).unwrap(),
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
        HoveredTitleButton::Settings => {
            bg_paint.set_color(Color::from_rgba8(255, 255, 255, 20));
            pixmap.fill_rect(
                Rect::from_xywh(x0, 0.0, btn_size, btn_size).unwrap(),
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

    let mut stroke_solid = tiny_skia::Stroke::default();
    stroke_solid.width = 1.0;
    stroke_solid.line_cap = tiny_skia::LineCap::Round;

    // close — тонкий крестик
    let cx = x2 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let arm = 3.5;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - arm, cy - arm);
    path.line_to(cx + arm, cy + arm);
    path.move_to(cx + arm, cy - arm);
    path.line_to(cx - arm, cy + arm);
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke_solid, Transform::identity(), None);
    }

    // minimize — тонкая линия подчёркивания
    let cx = x1 + btn_size / 2.0;
    let cy = btn_size * 0.65;
    let half_w = 4.5;
    let mut path = tiny_skia::PathBuilder::new();
    path.move_to(cx - half_w, cy);
    path.line_to(cx + half_w, cy);
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke_solid, Transform::identity(), None);
    }

    // settings — шестерёнка (окружность с четырьмя спицами)
    let cx = x0 + btn_size / 2.0;
    let cy = btn_size / 2.0;
    let r = 4.0;
    let spoke_len = 2.5;
    // окружность
    let mut circ = tiny_skia::PathBuilder::new();
    circ.push_circle(cx, cy, r);
    if let Some(p) = circ.finish() {
        let mut stroke_circ = tiny_skia::Stroke::default();
        stroke_circ.width = 1.0;
        pixmap.stroke_path(&p, &paint, &stroke_circ, Transform::identity(), None);
    }
    // спицы
    let mut path = tiny_skia::PathBuilder::new();
    for angle_deg in [0.0, 90.0, 180.0, 270.0] {
        let rad = angle_deg * std::f32::consts::PI / 180.0;
        let (dx, dy) = (rad.cos(), rad.sin());
        path.move_to(cx + r * dx, cy + r * dy);
        path.line_to(cx + (r + spoke_len) * dx, cy + (r + spoke_len) * dy);
    }
    if let Some(p) = path.finish() {
        pixmap.stroke_path(&p, &paint, &stroke_solid, Transform::identity(), None);
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
        if show_seconds {
            format!("{}h {:02}m {:02}s", h, m, s)
        } else {
            format!("{}h {:02}m", h, m)
        }
    } else if m > 0 {
        if show_seconds {
            format!("{}m {:02}s", m, s)
        } else {
            format!("{}m", m)
        }
    } else {
        if show_seconds {
            format!("{}s", s)
        } else {
            "0m".to_string()
        }
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

fn font_bold() -> Option<&'static fontdue::Font> {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        let data = std::fs::read("C:\\Windows\\Fonts\\segoeuib.ttf")
            .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf"))
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
