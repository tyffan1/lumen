#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lumen_core::{extract_exe_icon, extract_icon_by_window, spawn_tracker, AppIcon, Config, TrackerEvent};
use lumen_storage::{Session, Storage};
use lumen_ui::{AppUsage, UserEvent};
use tray_icon::{Icon, TrayIconBuilder, menu::{Menu, MenuEvent, MenuItem}};
use winit::event_loop::EventLoop;

fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let data_dir = data_dir_path()?;
    let config_path = data_dir.join("config.json");
    let config = Arc::new(Mutex::new(Config::load(&config_path)));
    let shared_usage: Arc<Mutex<Vec<AppUsage>>> = Arc::new(Mutex::new(Vec::new()));

    let events = spawn_tracker(true, config.clone());
    let mut storage = Storage::open(data_dir.join("lumen.db"))?;
    let clear_history_flag = Arc::new(AtomicBool::new(false));

    std::thread::spawn({
        let shared_usage = shared_usage.clone();
        let proxy = proxy.clone();
        let clear_history_flag = clear_history_flag.clone();
        move || run_aggregator(events, &mut storage, shared_usage, proxy, clear_history_flag)
    });

    let img = image::load_from_memory(include_bytes!("../../lumen.png"))
        .expect("lumen.png")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let icon = Icon::from_rgba(img.into_raw(), w, h).expect("tray icon");

    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Открыть", true, None);
    let exit_item = MenuItem::new("Выход", true, None);
    tray_menu.append(&show_item).ok();
    tray_menu.append(&exit_item).ok();

    let _tray = TrayIconBuilder::new()
        .with_tooltip("Lumen")
        .with_icon(icon)
        .with_menu(Box::new(tray_menu))
        .build()
        .expect("tray icon");

    let show_id = show_item.id().clone();
    let exit_id = exit_item.id().clone();
    let proxy_for_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
        if event.id == show_id {
            let _ = proxy_for_menu.send_event(UserEvent::ShowWindow);
        } else if event.id == exit_id {
            let _ = proxy_for_menu.send_event(UserEvent::ExitRequested);
        }
    }));

    let mut app = lumen_ui::LumenApp::new(shared_usage, config, config_path, clear_history_flag);
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn run_aggregator(
    events: std::sync::mpsc::Receiver<TrackerEvent>,
    storage: &mut Storage,
    shared_usage: Arc<Mutex<Vec<AppUsage>>>,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    clear_history_flag: Arc<AtomicBool>,
) {
    use chrono::Utc;

    let mut current: Option<(lumen_core::ProcessInfo, chrono::DateTime<Utc>, bool)> = None;
    let mut is_idle = false;
    let mut last_foreground: Option<lumen_core::ProcessInfo> = None;
    let mut totals: HashMap<String, u64> = HashMap::new();
    let mut icon_cache: HashMap<String, AppIcon> = HashMap::new();
    let mut last_flush = Instant::now();
    let mut last_ui_update = Instant::now();

    const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
    const UI_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

    let mut send_update = false;

    loop {
        if clear_history_flag.load(Ordering::Relaxed) {
            totals.clear();
            icon_cache.clear();
            let _ = storage.clear();
            update_shared_usage(&shared_usage, &totals, &current, &icon_cache);
            let _ = proxy.send_event(UserEvent::DataUpdated);
            clear_history_flag.store(false, Ordering::Relaxed);
        }

        let event = match events.recv_timeout(UI_UPDATE_INTERVAL) {
            Ok(event) => Some(event),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        if let Some(event) = event {
            match event {
                TrackerEvent::WindowChanged(info) => {
                    eprintln!("[lumen] FG: {:>12} (pid={}) \"{}\" @ {}",
                        info.exe_name, info.pid, info.window_title,
                        chrono::Local::now().format("%H:%M:%S%.3f"),
                    );
                    last_foreground = Some(info.clone());
                    if !info.exe_path.is_empty() && !icon_cache.contains_key(&info.exe_name) {
                        let icon = extract_exe_icon(&info.exe_path)
                            .or_else(|| info.window_handle.as_ref().and_then(extract_icon_by_window));
                        if let Some(icon) = icon {
                            icon_cache.insert(info.exe_name.clone(), icon);
                        }
                    }
                    if !is_idle {
                        if let Some((prev_info, started_at, was_fs)) = current.take() {
                            let dur = (Utc::now() - started_at).num_seconds().max(0) as u64;
                            if dur > 0 {
                                eprintln!("[lumen] CLOSE {} +{:.1}s (fullscreen={}) → {}",
                                    prev_info.exe_name, dur as f64, was_fs, info.exe_name);
                            }
                            *totals.entry(prev_info.exe_name.clone()).or_insert(0) += dur;
                            storage.queue(Session {
                                exe_name: prev_info.exe_name,
                                window_title: prev_info.window_title,
                                started_at,
                                ended_at: Utc::now(),
                                was_fullscreen: false,
                            });
                        }
                        current = Some((info, Utc::now(), false));
                    }
                    send_update = true;
                }
                TrackerEvent::FullscreenEntered(_) => {
                    eprintln!("[lumen] FS_ENTER → current={:?}", 
                        current.as_ref().map(|c| &c.0.exe_name));
                    if let Some((_, _, was_fullscreen)) = &mut current {
                        *was_fullscreen = true;
                    }
                    send_update = true;
                }
                TrackerEvent::FullscreenExited => {
                    eprintln!("[lumen] FS_EXIT");
                    if let Some((_, _, was_fullscreen)) = &mut current {
                        *was_fullscreen = false;
                    }
                    send_update = true;
                }
                TrackerEvent::IdleStarted => {
                    eprintln!("[lumen] IDLE_STARTED");
                    if !is_idle {
                        is_idle = true;
                        if let Some((prev_info, started_at, was_fullscreen)) = current.take() {
                            let dur = (Utc::now() - started_at).num_seconds().max(0) as u64;
                            *totals.entry(prev_info.exe_name.clone()).or_insert(0) += dur;
                            storage.queue(Session {
                                exe_name: prev_info.exe_name,
                                window_title: prev_info.window_title,
                                started_at,
                                ended_at: Utc::now(),
                                was_fullscreen,
                            });
                        }
                        send_update = true;
                    }
                }
                TrackerEvent::IdleEnded => {
                    eprintln!("[lumen] IDLE_END → restore={}", 
                        last_foreground.as_ref().map(|i| &i.exe_name).unwrap_or(&"?".to_string()));
                    if is_idle {
                        is_idle = false;
                        if let Some(info) = &last_foreground {
                            current = Some((info.clone(), Utc::now(), false));
                        }
                        send_update = true;
                    }
                }
            }
        }

        if send_update {
            build_and_send(&shared_usage, &proxy, &totals, &current, &icon_cache);
            send_update = false;
        }

        if last_flush.elapsed() >= FLUSH_INTERVAL {
            let _ = storage.flush();
            last_flush = Instant::now();
        }

        if last_ui_update.elapsed() >= UI_UPDATE_INTERVAL {
            update_shared_usage(&shared_usage, &totals, &current, &icon_cache);
            last_ui_update = Instant::now();
        }
    }

    if let Some((prev_info, started_at, was_fullscreen)) = current.take() {
        storage.queue(Session {
            exe_name: prev_info.exe_name,
            window_title: prev_info.window_title,
            started_at,
            ended_at: Utc::now(),
            was_fullscreen,
        });
    }
    let _ = storage.flush();
}

/// Обновляет shared_usage без отправки события (тихий апдейт для таймера).
fn update_shared_usage(
    shared_usage: &Arc<Mutex<Vec<AppUsage>>>,
    totals: &HashMap<String, u64>,
    current: &Option<(lumen_core::ProcessInfo, chrono::DateTime<chrono::Utc>, bool)>,
    icon_cache: &HashMap<String, AppIcon>,
) {
    *shared_usage.lock().unwrap() = build_usage_list(totals, current, icon_cache);
}

/// Строит список и шлёт DataUpdated (для значимых событий).
fn build_and_send(
    shared_usage: &Arc<Mutex<Vec<AppUsage>>>,
    proxy: &winit::event_loop::EventLoopProxy<UserEvent>,
    totals: &HashMap<String, u64>,
    current: &Option<(lumen_core::ProcessInfo, chrono::DateTime<chrono::Utc>, bool)>,
    icon_cache: &HashMap<String, AppIcon>,
) {
    *shared_usage.lock().unwrap() = build_usage_list(totals, current, icon_cache);
    let _ = proxy.send_event(UserEvent::DataUpdated);
}

fn build_usage_list(
    totals: &HashMap<String, u64>,
    current: &Option<(lumen_core::ProcessInfo, chrono::DateTime<chrono::Utc>, bool)>,
    icon_cache: &HashMap<String, AppIcon>,
) -> Vec<AppUsage> {
    use chrono::Utc;

    let attach_icon = |name: &str| -> (Option<Vec<u8>>, u32, u32) {
        icon_cache
            .get(name)
            .map(|icon| (Some(icon.rgba.clone()), icon.width, icon.height))
            .unwrap_or((None, 0, 0))
    };

    let mut usage: Vec<AppUsage> = totals
        .iter()
        .map(|(name, secs)| {
            let (icon_rgba, icon_w, icon_h) = attach_icon(name);
            AppUsage {
                name: name.clone(),
                duration_secs: *secs,
                is_active: false,
                icon_rgba,
                icon_w,
                icon_h,
            }
        })
        .collect();

    if let Some((info, started_at, _)) = current {
        let extra = (Utc::now() - *started_at).num_seconds().max(0) as u64;
        let active_secs = totals.get(&info.exe_name).copied().unwrap_or(0) + extra;
        let (icon_rgba, icon_w, icon_h) = attach_icon(&info.exe_name);
        if let Some(existing) = usage.iter_mut().find(|a| a.name == info.exe_name) {
            existing.duration_secs = active_secs;
            existing.is_active = true;
        } else {
            usage.push(AppUsage {
                name: info.exe_name.clone(),
                duration_secs: active_secs,
                is_active: true,
                icon_rgba,
                icon_w,
                icon_h,
            });
        }
    }

    usage.sort_by(|a, b| b.duration_secs.cmp(&a.duration_secs));
    usage
}

fn data_dir_path() -> anyhow::Result<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(|| {
            eprintln!("lumen: warning: data_local_dir() returned None, falling back to current dir");
            std::env::current_dir().ok()
        })
        .unwrap_or_else(|| {
            eprintln!("lumen: warning: current_dir() also failed, using '.'");
            std::path::PathBuf::from(".")
        });

    let dir = base.join("Lumen");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
