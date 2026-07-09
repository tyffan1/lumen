mod chrome;
mod render;

pub use chrome::HitZone;
pub use render::AppUsage;
pub use render::HoveredTitleButton;

use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
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
}

impl LumenApp {
    pub fn new(shared_usage: Arc<Mutex<Vec<AppUsage>>>) -> Self {
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
        if self.window_visible && self.window_focused {
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
            .with_inner_size(winit::dpi::LogicalSize::new(400.0, 560.0))
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

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.window_visible = true;
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
                            chrome::HitZone::ResizeLeft => { let _ = window.drag_resize_window(ResizeDirection::West); }
                            chrome::HitZone::ResizeRight => { let _ = window.drag_resize_window(ResizeDirection::East); }
                            chrome::HitZone::ResizeTop => { let _ = window.drag_resize_window(ResizeDirection::North); }
                            chrome::HitZone::ResizeBottom => { let _ = window.drag_resize_window(ResizeDirection::South); }
                            chrome::HitZone::ResizeTopLeft => { let _ = window.drag_resize_window(ResizeDirection::NorthWest); }
                            chrome::HitZone::ResizeTopRight => { let _ = window.drag_resize_window(ResizeDirection::NorthEast); }
                            chrome::HitZone::ResizeBottomLeft => { let _ = window.drag_resize_window(ResizeDirection::SouthWest); }
                            chrome::HitZone::ResizeBottomRight => { let _ = window.drag_resize_window(ResizeDirection::SouthEast); }
                            _ => self.start_drag_if_on_titlebar(size),
                        }
                    }
                    ElementState::Released => self.dragging = false,
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_pos = (position.x, position.y);
                if self.dragging {
                    self.drag_window(position.x, position.y);
                }
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    let zone = chrome::hit_test(position.x, position.y, size.width as f64, size.height as f64);
                    let new_hover = match zone {
                        chrome::HitZone::CloseButton | chrome::HitZone::MinimizeButton => Some(zone),
                        _ => None,
                    };
                    if new_hover != self.hovered_button {
                        self.hovered_button = new_hover;
                        self.dirty = true;
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

            WindowEvent::CursorLeft { .. } => {
                if self.hovered_button.is_some() {
                    self.hovered_button = None;
                    self.dirty = true;
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

    fn redraw(&mut self) {
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

        let usage = self.shared_usage.lock().unwrap().clone();

        let hover = match self.hovered_button {
            Some(chrome::HitZone::CloseButton) => render::HoveredTitleButton::Close,
            Some(chrome::HitZone::MinimizeButton) => render::HoveredTitleButton::Minimize,
            _ => render::HoveredTitleButton::None,
        };
        let pixmap = render::draw_frame(width, height, &render::Theme::default(), &usage, hover);
        render::blit_to_softbuffer(&pixmap, &mut *buffer);
        let _ = buffer.present();
    }
}
