#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Titlebar,
    CloseButton,
    MinimizeButton,
    SettingsButton,
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

pub fn hit_test(x: f64, y: f64, window_width: f64, window_height: f64, scale: f64) -> HitZone {
    let th = TITLEBAR_HEIGHT * scale;
    let bs = BUTTON_SIZE * scale;
    let rb = RESIZE_BORDER * scale;
    let sh = SEARCH_HEIGHT * scale;
    let px = PADDING_X * scale;
    let scw = SEARCH_CLEAR_W * scale;

    // 1. Кнопки на titlebar (имеют приоритет над resize-углами сверху). Три кнопки по 32px.
    if y <= th {
        if x >= window_width - bs {
            return HitZone::CloseButton;
        }
        if x >= window_width - bs * 2.0 {
            return HitZone::MinimizeButton;
        }
        if x >= window_width - bs * 3.0 {
            return HitZone::SettingsButton;
        }
    }

    // 2. Resize-границы (6px от краёв)
    let on_left = x <= rb;
    let on_right = x >= window_width - rb;
    let on_top = y <= rb;
    let on_bottom = y >= window_height - rb;

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
    if y > th && y <= th + sh {
        if x >= window_width - px - scw {
            return HitZone::SearchClear;
        }
        return HitZone::SearchField;
    }

    // 4. Drag titlebar (между кнопками и левым краем, не в resize-зоне)
    if y <= th {
        return HitZone::Titlebar;
    }

    HitZone::Client
}
