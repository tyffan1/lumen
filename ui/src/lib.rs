mod chrome;
mod render;

pub use chrome::HitZone;
pub use render::AppUsage;
pub use render::AppView;
pub use render::HoveredTitleButton;

use lumen_core::Config;
use lumen_storage::Storage;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::keyboard::{Key, NamedKey};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowAttributes, WindowId};

/// Пользовательское событие для winit EventLoopProxy — пробуждает event loop
/// без создания окна, когда агрегатор обновил данные.
#[derive(Debug, Clone)]
pub enum UserEvent {
    DataUpdated,
    /// Показать окно (из трей-меню).
    ShowWindow,
    /// Завершить приложение (из трей-меню).
    ExitRequested,
}

const TICK_INTERVAL: Duration = Duration::from_secs(1);

pub struct LumenApp {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context: Option<Context<Rc<Window>>>,
    dirty: bool,
    dragging: bool,
    last_cursor_pos: (f64, f64),
    drag_start_cursor: (f64, f64),
    drag_start_window: (i32, i32),
    shared_usage: Arc<Mutex<Vec<AppUsage>>>,
    /// true, пока окно Lumen имеет фокус ввода. Только в этом состоянии
    /// поддерживается посекундный WaitUntil для обновления счётчиков.
    window_focused: bool,
    /// true, пока окно видимо (не скрыто в трей).
    window_visible: bool,
    /// Какая titlebar-кнопка под курсором (для hover-эффекта).
    hovered_button: Option<HitZone>,
    /// Индекс строки списка/настроек под курсором (None = не над строками).
    hovered_row: Option<usize>,
    /// Прогресс hover-анимации 0.0…1.0.
    hover_progress: f32,
    /// Время последнего кадра для расчёта delta анимации.
    last_frame: Option<Instant>,
    /// Текст поискового запроса.
    search_query: String,
    /// true, если поле поиска в фокусе.
    search_focused: bool,
    /// true, когда курсор в поиске видим (мигание каждые 500ms).
    search_cursor_visible: bool,
    /// Время последнего переключения видимости курсора.
    last_cursor_toggle: Instant,
    /// Текущий экран: список приложений или настройки.
    current_view: AppView,
    /// Включён ли автозапуск.
    autostart_enabled: bool,
    /// Разделяемая конфигурация (idle threshold, show_seconds, start_minimized).
    config: Arc<Mutex<Config>>,
    /// Путь к config.json для сохранения.
    config_path: PathBuf,
    /// true, когда показано подтверждение очистки истории.
    confirm_clear: bool,
    /// Флаг для агрегатора: очистить историю.
    clear_history_flag: Arc<AtomicBool>,
    /// Соединение с БД для запроса данных графика (только чтение).
    storage: Option<Storage>,
    /// Кэш по-дневной статистики: Vec<(дата "YYYY-MM-DD", секунды)>.
    chart_cache: Vec<(String, u64)>,
    /// Смещение скролла списка приложений (в пикселях).
    scroll_offset: f32,
    /// Целевое смещение для плавной анимации скролла.
    scroll_target: f32,
    /// Кэш данных для круговой диаграммы: Vec<(имя, секунды_за_сегодня)>.
    donut_cache: Vec<(String, u64)>,
    /// true, когда пользователь тащит ползунок скроллбара.
    scroll_dragging: bool,
    /// Y мыши (в координатах окна) в момент начала перетаскивания ползунка.
    scroll_drag_start_y: f64,
    /// scroll_target в момент начала перетаскивания.
    scroll_drag_start_offset: f32,
}

impl LumenApp {
    pub fn new(shared_usage: Arc<Mutex<Vec<AppUsage>>>, config: Arc<Mutex<Config>>, config_path: PathBuf, clear_history_flag: Arc<AtomicBool>) -> Self {
        let autostart_enabled = Self::read_autostart();
        Self {
            window: None,
            surface: None,
            context: None,
            dirty: true,
            dragging: false,
            last_cursor_pos: (0.0, 0.0),
            drag_start_cursor: (0.0, 0.0),
            drag_start_window: (0, 0),
            shared_usage,
            window_focused: false,
            window_visible: false,
            hovered_button: None,
            hovered_row: None,
            hover_progress: 0.0,
            last_frame: None,
            search_query: String::new(),
            search_focused: false,
            search_cursor_visible: true,
            last_cursor_toggle: Instant::now(),
            current_view: AppView::List,
            autostart_enabled,
            config,
            config_path,
            confirm_clear: false,
            clear_history_flag,
            storage: None,
            chart_cache: Vec::new(),
            scroll_offset: 0.0,
            scroll_target: 0.0,
            donut_cache: Vec::new(),
            scroll_dragging: false,
            scroll_drag_start_y: 0.0,
            scroll_drag_start_offset: 0.0,
        }
    }

    fn read_autostart() -> bool {
        use winreg::enums::*;
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_READ,
            )
            .ok()
            .and_then(|k| k.get_value::<String, _>("Lumen").ok())
            .is_some()
    }

    fn write_autostart(enabled: bool) {
        use winreg::enums::*;
        if let Ok(run) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_SET_VALUE,
            )
        {
            if enabled {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = run.set_value("Lumen", &exe.to_string_lossy().as_ref());
                }
            } else {
                let _ = run.delete_value("Lumen");
            }
        }
    }

    fn request_redraw_if_dirty(&mut self) {
        if self.dirty {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// После каждого redraw, если окно в фокусе, планируем следующий тик
    /// через TICK_INTERVAL. Вызывается из обработчика RedrawRequested,
    /// где event_loop доступен.
    fn schedule_next_tick(&self, event_loop: &ActiveEventLoop) {
        let animating = (self.hover_progress > 0.0 && self.hover_progress < 1.0)
            || (self.scroll_offset != self.scroll_target);
        if animating {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(16),
            ));
        } else if self.search_focused {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(500),
            ));
        } else if self.window_visible && self.window_focused {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + TICK_INTERVAL,
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

impl ApplicationHandler<UserEvent> for LumenApp {
    /// Вызывается при старте и при каждом пробуждении event loop.
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + TICK_INTERVAL,
        ));

        let attrs = WindowAttributes::default()
            .with_title("Lumen")
            .with_decorations(false)
            .with_inner_size(winit::dpi::LogicalSize::new(500.0, 600.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(280.0, 200.0));

        let window = Rc::new(event_loop.create_window(attrs).expect("create window"));

        // Windows 11+: скруглённые углы окна. На Win10 тихо ignored.
        #[cfg(windows)]
        {
            use std::ffi::c_void;
            use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
            if let Ok(handle) = winit::raw_window_handle::HasWindowHandle::window_handle(&window) {
                use windows::Win32::Foundation::HWND;
                if let winit::raw_window_handle::RawWindowHandle::Win32(w32) = handle.as_ref() {
                    let hwnd = HWND(w32.hwnd.get() as *mut c_void);
                    let pref: u32 = 2;
                    unsafe {
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(33),
                            &pref as *const _ as *const c_void,
                            std::mem::size_of::<u32>() as u32,
                        );
                    }
                }
            }
        }

        let context = Context::new(window.clone()).expect("softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");

        self.window = Some(window.clone());
        self.context = Some(context);
        self.surface = Some(surface);

        let start_minimized = self.config.lock().unwrap().start_minimized;
        if start_minimized {
            self.window_visible = false;
            window.set_visible(false);
        } else {
            self.window_visible = true;
        }

        if self.storage.is_none() {
            let db_path = self.config_path.parent().unwrap().join("lumen.db");
            self.storage = Storage::open(&db_path).ok();
        }

        self.mark_dirty();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.window_visible = false;
                if let Some(window) = &self.window {
                    window.set_visible(false);
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }

            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                if focused {
                    self.mark_dirty();
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + TICK_INTERVAL,
                    ));
                } else {
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    if let (Some(w), Some(h)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    {
                        surface.resize(w, h).ok();
                    }
                }
                self.mark_dirty();
            }

            WindowEvent::RedrawRequested => {
                self.redraw();
                self.dirty = false;
                self.schedule_next_tick(event_loop);
            }

            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let Some(window) = &self.window else { return };
                let (cx, cy) = self.last_cursor_pos;
                let size = window.inner_size();
                match state {
                    ElementState::Pressed => {
                        match chrome::hit_test(cx, cy, size.width as f64, size.height as f64) {
                            chrome::HitZone::CloseButton => {
                                self.window_visible = false;
                                window.set_visible(false);
                                event_loop.set_control_flow(ControlFlow::Wait);
                            }
                            chrome::HitZone::MinimizeButton => {
                                window.set_minimized(true);
                            }
                            chrome::HitZone::ChartButton => {
                                self.current_view = match self.current_view {
                                    AppView::Chart => AppView::List,
                                    _ => AppView::Chart,
                                };
                                self.search_focused = false;
                                self.hovered_row = None;
                                self.confirm_clear = false;
                                if self.current_view == AppView::Chart {
                                    self.refresh_chart_cache();
                                }
                                self.dirty = true;
                            }
                            chrome::HitZone::SettingsButton => {
                                self.current_view = match self.current_view {
                                    AppView::Settings => AppView::List,
                                    _ => AppView::Settings,
                                };
                                self.search_focused = false;
                                self.hovered_row = None;
                                self.confirm_clear = false;
                                self.dirty = true;
                            }
                            chrome::HitZone::SearchField => {
                                self.search_focused = true;
                                self.search_cursor_visible = true;
                                self.last_cursor_toggle = Instant::now();
                                self.dirty = true;
                            }
                            chrome::HitZone::SearchClear => {
                                self.search_query.clear();
                                self.search_focused = true;
                                self.search_cursor_visible = true;
                                self.last_cursor_toggle = Instant::now();
                                self.dirty = true;
                            }
                            chrome::HitZone::ResizeLeft => { let _ = window.drag_resize_window(ResizeDirection::West); }
                            chrome::HitZone::ResizeRight => { let _ = window.drag_resize_window(ResizeDirection::East); }
                            chrome::HitZone::ResizeTop => { let _ = window.drag_resize_window(ResizeDirection::North); }
                            chrome::HitZone::ResizeBottom => { let _ = window.drag_resize_window(ResizeDirection::South); }
                            chrome::HitZone::ResizeTopLeft => { let _ = window.drag_resize_window(ResizeDirection::NorthWest); }
                            chrome::HitZone::ResizeTopRight => { let _ = window.drag_resize_window(ResizeDirection::NorthEast); }
                            chrome::HitZone::ResizeBottomLeft => { let _ = window.drag_resize_window(ResizeDirection::SouthWest); }
                            chrome::HitZone::ResizeBottomRight => { let _ = window.drag_resize_window(ResizeDirection::SouthEast); }
                            chrome::HitZone::Client if self.current_view == AppView::Chart => {
                                if self.hovered_row == Some(0) {
                                    self.current_view = AppView::List;
                                    self.hovered_row = None;
                                    self.dirty = true;
                                }
                            }
                            chrome::HitZone::Client if self.current_view == AppView::Settings => {
                                if let Some(row) = render::settings_row_at(cy as f32) {
                                    match row {
                                        0 => {
                                            self.autostart_enabled = !self.autostart_enabled;
                                            Self::write_autostart(self.autostart_enabled);
                                            self.dirty = true;
                                        }
                                        1 => {
                                            let mut cfg = self.config.lock().unwrap();
                                            cfg.start_minimized = !cfg.start_minimized;
                                            cfg.save(&self.config_path);
                                            self.dirty = true;
                                        }
                                        2 => {
                                            let (minus_cx, plus_cx) = render::settings_idle_button_positions(size.width);
                                            let hit_radius = 15.0;
                                            let mut cfg = self.config.lock().unwrap();
                                            if (cx - minus_cx as f64).abs() < hit_radius {
                                                if cfg.idle_threshold_mins > 1 {
                                                    cfg.idle_threshold_mins -= 1;
                                                }
                                            } else if (cx - plus_cx as f64).abs() < hit_radius {
                                                if cfg.idle_threshold_mins < 30 {
                                                    cfg.idle_threshold_mins += 1;
                                                }
                                            }
                                            cfg.save(&self.config_path);
                                            self.dirty = true;
                                        }
                                        3 => {
                                            let mut cfg = self.config.lock().unwrap();
                                            cfg.show_seconds = !cfg.show_seconds;
                                            cfg.save(&self.config_path);
                                            self.dirty = true;
                                        }
                                        4 => {
                                            if self.confirm_clear {
                                                let (yes_zone, no_zone) = render::settings_confirm_areas(size.width);
                                                if cx >= yes_zone.0 as f64 && cx < yes_zone.1 as f64 {
                                                    self.clear_history_flag.store(true, Ordering::Relaxed);
                                                    self.confirm_clear = false;
                                                    self.dirty = true;
                                                } else if cx >= no_zone.0 as f64 && cx < no_zone.1 as f64 {
                                                    self.confirm_clear = false;
                                                    self.dirty = true;
                                                }
                                            } else {
                                                self.confirm_clear = true;
                                                self.dirty = true;
                                            }
                                        }
                                        5 => {
                                            let path = std::env::var("LOCALAPPDATA")
                                                .map(|s| std::path::PathBuf::from(s).join("Lumen"))
                                                .unwrap_or_else(|_| std::path::PathBuf::from("."));
                                            let _ = std::process::Command::new("explorer").arg(&path).spawn();
                                        }
                                        6 => {
                                            self.current_view = AppView::List;
                                            self.hovered_row = None;
                                            self.dirty = true;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ if self.current_view == AppView::List
                                && (cx as f32) >= render::scrollbar_x(size.width)
                                && (cx as f32) <= render::scrollbar_x(size.width) + render::SCROLLBAR_W
                                && (cy as f32) >= render::LIST_TOP
                                && (cy as f32) <= (size.height as f32) - render::DONUT_HEIGHT =>
                            {
                                let vp_bottom = (size.height as f32) - render::DONUT_HEIGHT;
                                let vp_h = vp_bottom - render::LIST_TOP;
                                let usage_len = self.shared_usage.lock().unwrap().len();
                                let content_h = usage_len as f32 * render::ROW_HEIGHT;
                                if content_h > vp_h {
                                    let max_scroll = content_h - vp_h;
                                    let track_h = vp_h;
                                    let thumb_h = (vp_h / content_h * track_h).max(12.0);
                                    let click_ratio = ((cy as f32 - render::LIST_TOP) - thumb_h / 2.0) / (track_h - thumb_h);
                                    self.scroll_target = (click_ratio * max_scroll).clamp(0.0, max_scroll);
                                }
                                self.scroll_dragging = true;
                                self.scroll_drag_start_y = cy;
                                self.scroll_drag_start_offset = self.scroll_target;
                                self.dirty = true;
                            }
                            _ => {
                                if self.search_focused {
                                    self.search_focused = false;
                                    self.dirty = true;
                                }
                                self.start_drag_if_on_titlebar(size)
                            }
                        }
                    }
                    ElementState::Released => {
                        self.dragging = false;
                        self.scroll_dragging = false;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_pos = (position.x, position.y);
                if self.dragging {
                    self.drag_window(position.x, position.y);
                }
                if self.scroll_dragging {
                    if let Some(window) = &self.window {
                        let size = window.inner_size();
                        let vp_bottom = (size.height as f32) - render::DONUT_HEIGHT;
                        let vp_h = vp_bottom - render::LIST_TOP;
                        let usage_len = self.shared_usage.lock().unwrap().len();
                        let content_h = usage_len as f32 * render::ROW_HEIGHT;
                        if content_h > vp_h {
                            let max_scroll = content_h - vp_h;
                            let track_h = vp_h;
                            let thumb_h = (vp_h / content_h * track_h).max(12.0);
                            let dy = position.y - self.scroll_drag_start_y;
                            let ratio = dy as f32 / (track_h - thumb_h);
                            let new_offset = (self.scroll_drag_start_offset + ratio * max_scroll).clamp(0.0, max_scroll);
                            self.scroll_target = new_offset;
                        }
                        self.dirty = true;
                        window.request_redraw();
                    }
                }
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    let zone = chrome::hit_test(position.x, position.y, size.width as f64, size.height as f64);
                    let new_hover = match zone {
                        chrome::HitZone::CloseButton | chrome::HitZone::MinimizeButton | chrome::HitZone::SettingsButton | chrome::HitZone::ChartButton => Some(zone),
                        _ => None,
                    };
                    if new_hover != self.hovered_button {
                        self.hovered_button = new_hover;
                        self.dirty = true;
                        window.request_redraw();
                    }
                    if self.current_view == AppView::List {
                        let vp_bottom = (size.height as f32 - render::DONUT_HEIGHT) as f64;
                        let new_row = if position.y > render::LIST_TOP as f64 && position.y < vp_bottom {
                            Some(((position.y - render::LIST_TOP as f64 + self.scroll_offset as f64) / render::ROW_HEIGHT as f64) as usize)
                        } else {
                            None
                        };
                        if new_row != self.hovered_row {
                            self.hovered_row = new_row;
                            self.dirty = true;
                            window.request_redraw();
                        }
                    } else if self.current_view == AppView::Settings {
                        let new_row = render::settings_row_at(position.y as f32);
                        if new_row != self.hovered_row {
                            self.hovered_row = new_row;
                            self.dirty = true;
                            window.request_redraw();
                        }
                    } else if self.current_view == AppView::Chart {
                        let back_y = render::chart_back_y(size.height) as f64;
                        let new_row = if position.y >= back_y - 12.0 && position.y < back_y + 8.0 {
                            Some(0)
                        } else {
                            None
                        };
                        if new_row != self.hovered_row {
                            self.hovered_row = new_row;
                            self.dirty = true;
                            window.request_redraw();
                        }
                    }

                    let cursor = match zone {
                        chrome::HitZone::ResizeLeft | chrome::HitZone::ResizeRight => CursorIcon::EwResize,
                        chrome::HitZone::ResizeTop | chrome::HitZone::ResizeBottom => CursorIcon::NsResize,
                        chrome::HitZone::ResizeTopLeft | chrome::HitZone::ResizeBottomRight => CursorIcon::NwseResize,
                        chrome::HitZone::ResizeTopRight | chrome::HitZone::ResizeBottomLeft => CursorIcon::NeswResize,
                        _ => CursorIcon::Default,
                    };
                    window.set_cursor(cursor);
                }
            }

            WindowEvent::KeyboardInput { event, .. } if self.search_focused => {
                if event.state == ElementState::Pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Backspace) => {
                            self.search_query.pop();
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Enter) => {
                            self.search_focused = false;
                            self.dirty = true;
                        }
                        _ => {
                            if let Some(text) = &event.text {
                                if !text.is_empty() {
                                    self.search_query.push_str(text);
                                    self.search_cursor_visible = true;
                                    self.last_cursor_toggle = Instant::now();
                                    self.dirty = true;
                                }
                            }
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } if self.current_view == AppView::List => {
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    let viewport_h = (size.height as f32) - render::DONUT_HEIGHT - render::LIST_TOP;
                    let content_h = self.shared_usage.lock().unwrap().len() as f32 * render::ROW_HEIGHT;
                    let max_scroll = (content_h - viewport_h).max(0.0);
                    let delta_y = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * render::ROW_HEIGHT,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    self.scroll_target = (self.scroll_target - delta_y).clamp(0.0, max_scroll);
                    self.dirty = true;
                    window.request_redraw();
                }
            }

            WindowEvent::CursorLeft { .. } => {
                if self.hovered_button.is_some() {
                    self.hovered_button = None;
                    self.dirty = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                if self.hovered_row.is_some() {
                    self.hovered_row = None;
                    self.dirty = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }

        self.request_redraw_if_dirty();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::DataUpdated => {
                self.mark_dirty();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            UserEvent::ShowWindow => {
                let Some(window) = &self.window else { return };
                self.window_visible = true;
                let _ = window.set_minimized(false);
                window.set_visible(true);
                window.focus_window();
                self.mark_dirty();
            }
            UserEvent::ExitRequested => {
                event_loop.exit();
            }
        }
    }
}

impl LumenApp {
    fn start_drag_if_on_titlebar(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };

        let (cx, cy) = self.last_cursor_pos;
        let zone = chrome::hit_test(cx, cy, size.width as f64, size.height as f64);

        if zone != chrome::HitZone::Titlebar {
            return;
        }

        if window.drag_window().is_ok() {
            return;
        }

        self.dragging = true;
        let pos = window.outer_position().unwrap_or(PhysicalPosition::new(0, 0));
        self.drag_start_cursor = (cx, cy);
        self.drag_start_window = (pos.x, pos.y);
    }

    fn drag_window(&mut self, cursor_x: f64, cursor_y: f64) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };

        let dx = cursor_x - self.drag_start_cursor.0;
        let dy = cursor_y - self.drag_start_cursor.1;

        let new_x = (self.drag_start_window.0 as f64 + dx) as i32;
        let new_y = (self.drag_start_window.1 as f64 + dy) as i32;

        let _ = window.set_outer_position(PhysicalPosition::new(new_x, new_y));
    }

    fn refresh_chart_cache(&mut self) {
        let rows = match self.storage {
            Some(ref storage) => storage.usage_by_day(7).unwrap_or_default(),
            None => return,
        };
        let today = chrono::Local::now().date_naive();
        let mut data: Vec<(String, u64)> = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let date = today - chrono::Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();
            let val = rows
                .iter()
                .find(|(d, _)| d == &date_str)
                .map(|(_, v)| (*v).max(0) as u64)
                .unwrap_or(0);
            data.push((date_str, val));
        }
        self.chart_cache = data;
    }

    fn refresh_donut_cache(&mut self) {
        let usage = self.shared_usage.lock().unwrap().clone();
        let mut result: Vec<(String, u64)> = usage.into_iter()
            .map(|app| (app.name.clone(), app.duration_secs))
            .collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        self.donut_cache = result;
    }

    fn advance_hover(&mut self) {
        let now = Instant::now();
        let dt = self.last_frame.map_or(0.0, |t| (now - t).as_secs_f32());
        self.last_frame = Some(now);

        let target = if self.hovered_row.is_some() { 1.0 } else { 0.0 };
        let speed = 1.0 / 0.150; // полный переход за 150ms

        if self.hover_progress < target {
            self.hover_progress = (self.hover_progress + dt * speed).min(1.0);
        } else if self.hover_progress > target {
            self.hover_progress = (self.hover_progress - dt * speed).max(0.0);
        }
    }

    fn redraw(&mut self) {
        let search_query = self.search_query.clone();
        let usage = self.shared_usage.lock().unwrap().clone();
        let usage: Vec<AppUsage> = if search_query.is_empty() {
            usage
        } else {
            let q = search_query.to_lowercase();
            usage.into_iter().filter(|a| {
                a.name.to_lowercase().contains(&q)
            }).collect()
        };

        // плавный скролл
        let diff = self.scroll_target - self.scroll_offset;
        if diff.abs() > 0.5 {
            self.scroll_offset += diff * 0.15;
            self.dirty = true;
        } else {
            self.scroll_offset = self.scroll_target;
        }

        // мигание курсора поиска
        if self.search_focused && self.last_cursor_toggle.elapsed() >= Duration::from_millis(500) {
            self.search_cursor_visible = !self.search_cursor_visible;
            self.last_cursor_toggle = Instant::now();
        }

        // защита от выхода за границы отфильтрованного списка
        if self.hovered_row.map_or(false, |r| r >= usage.len()) {
            self.hovered_row = None;
        }

        // обновляем кэш пончика на каждый редро (реагирует на DataUpdated/тик)
        self.refresh_donut_cache();

        let window = match &self.window {
            Some(w) => w,
            None => return,
        };
        let surface = match &mut self.surface {
            Some(s) => s,
            None => return,
        };

        let size = window.inner_size();
        let width = size.width;
        let height = size.height;
        if width == 0 || height == 0 {
            return;
        }

        let mut buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };

        let hover = match self.hovered_button {
            Some(chrome::HitZone::CloseButton) => render::HoveredTitleButton::Close,
            Some(chrome::HitZone::MinimizeButton) => render::HoveredTitleButton::Minimize,
            Some(chrome::HitZone::SettingsButton) => render::HoveredTitleButton::Settings,
            Some(chrome::HitZone::ChartButton) => render::HoveredTitleButton::Chart,
            _ => render::HoveredTitleButton::None,
        };
        let show_seconds = self.config.lock().unwrap().show_seconds;
        let idle_threshold_mins = self.config.lock().unwrap().idle_threshold_mins;
        let start_minimized = self.config.lock().unwrap().start_minimized;

        let pixmap = render::draw_frame(width, height, &render::Theme::default(), &usage, hover, self.hovered_row, self.hover_progress, &search_query, self.search_focused, self.search_cursor_visible, self.current_view, self.autostart_enabled, show_seconds, start_minimized, idle_threshold_mins, self.confirm_clear, &self.chart_cache, self.scroll_offset, &self.donut_cache);
        render::blit_to_softbuffer(&pixmap, &mut *buffer);
        let _ = buffer.present();
    }
}
