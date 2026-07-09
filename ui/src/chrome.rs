//! Раз системного chrome нет, нужен ручной hit-test:
//! какая часть окна за что отвечает (drag, close, minimize, resize border).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZone {
    Titlebar,
    CloseButton,
    MinimizeButton,
    ResizeBorder,
    Client,
}

const TITLEBAR_HEIGHT: f64 = 32.0;
const BUTTON_SIZE: f64 = 32.0;

/// TODO(DeepSeek): расширить под реальную ширину окна (передавать size),
/// сейчас предполагается фиксированная ширина 360 для разметки кнопок.
pub fn hit_test(x: f64, y: f64, window_width: f64) -> HitZone {
    if y > TITLEBAR_HEIGHT {
        return HitZone::Client;
    }

    if x > window_width - BUTTON_SIZE {
        return HitZone::CloseButton;
    }
    if x > window_width - BUTTON_SIZE * 2.0 {
        return HitZone::MinimizeButton;
    }

    HitZone::Titlebar
}
