#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Titlebar,
    CloseButton,
    MinimizeButton,
    SettingsButton,
    ChartButton,
    SearchField,
    SearchClear,
    ResizeLeft,
    ResizeRight,
    ResizeTop,
    ResizeBottom,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
    Client,
}

const TITLEBAR_HEIGHT: f64 = 32.0;
const BUTTON_SIZE: f64 = 32.0;
const RESIZE_BORDER: f64 = 6.0;
const SEARCH_HEIGHT: f64 = 28.0;
const PADDING_X: f64 = 16.0;
const SEARCH_CLEAR_W: f64 = 24.0;

pub fn hit_test(x: f64, y: f64, window_width: f64, window_height: f64) -> HitZone {
    // Приоритет: кнопки titlebar → resize края → поиск → drag / client

    // 1. Кнопки на titlebar (имеют приоритет над resize-углами сверху). Четыре кнопки по 32px.
    if y <= TITLEBAR_HEIGHT {
        if x >= window_width - BUTTON_SIZE {
            return HitZone::CloseButton;
        }
        if x >= window_width - BUTTON_SIZE * 2.0 {
            return HitZone::MinimizeButton;
        }
        if x >= window_width - BUTTON_SIZE * 3.0 {
            return HitZone::SettingsButton;
        }
        if x >= window_width - BUTTON_SIZE * 4.0 {
            return HitZone::ChartButton;
        }
    }

    // 2. Resize-границы (6px от краёв)
    let on_left = x <= RESIZE_BORDER;
    let on_right = x >= window_width - RESIZE_BORDER;
    let on_top = y <= RESIZE_BORDER;
    let on_bottom = y >= window_height - RESIZE_BORDER;

    if on_bottom && on_left {
        return HitZone::ResizeBottomLeft;
    }
    if on_bottom && on_right {
        return HitZone::ResizeBottomRight;
    }
    if on_top && on_left {
        return HitZone::ResizeTopLeft;
    }
    if on_top && on_right {
        return HitZone::ResizeTopRight;
    }
    if on_left {
        return HitZone::ResizeLeft;
    }
    if on_right {
        return HitZone::ResizeRight;
    }
    if on_bottom {
        return HitZone::ResizeBottom;
    }
    if on_top {
        return HitZone::ResizeTop;
    }

    // 3. Поле поиска (под titlebar, над списком). Крестик очистки справа.
    if y > TITLEBAR_HEIGHT && y <= TITLEBAR_HEIGHT + SEARCH_HEIGHT {
        if x >= window_width - PADDING_X - SEARCH_CLEAR_W {
            return HitZone::SearchClear;
        }
        return HitZone::SearchField;
    }

    // 4. Drag titlebar (между кнопками и левым краем, не в resize-зоне)
    if y <= TITLEBAR_HEIGHT {
        return HitZone::Titlebar;
    }

    HitZone::Client
}
