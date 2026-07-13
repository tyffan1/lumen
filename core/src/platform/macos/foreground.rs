use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use objc2::{define_class, msg_send, sel, ClassType, MainThreadOnly};
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2_foundation::{NSNotification, NSObjectProtocol};
use objc2_app_kit::{NSRunningApplication, NSWorkspace};

use crate::{ProcessInfo, TrackerEvent, WindowHandle};

static TX: Mutex<Option<Sender<TrackerEvent>>> = Mutex::new(None);

/// true, когда последним активным приложением был loginwindow (экран блокировки).
static LOGINWINDOW_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Проверяет, является ли имя процесса loginwindow (регистронезависимо).
fn is_loginwindow(name: &str) -> bool {
    name.eq_ignore_ascii_case("loginwindow")
}

/// Обрабатывает событие активации loginwindow.
/// Возвращает true, если событие поглощено (loginwindow) и не требует WindowChanged.
fn handle_loginwindow(tx: &Sender<TrackerEvent>, exe_name: &str) -> bool {
    if is_loginwindow(exe_name) {
        if !LOGINWINDOW_ACTIVE.swap(true, Ordering::Relaxed) {
            let _ = tx.send(TrackerEvent::IdleStarted);
        }
        return true;
    }

    if LOGINWINDOW_ACTIVE.swap(false, Ordering::Relaxed) {
        let _ = tx.send(TrackerEvent::IdleEnded);
    }

    false
}

#[derive(Default)]
struct ForegroundObserverIvars {
    // No ivars needed
}

define_class!(
    // SAFETY: NSObject doesn't have subclassing requirements; ForegroundObserver doesn't implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ForegroundObserverIvars]
    struct ForegroundObserver;

    // SAFETY: NSObjectProtocol has no safety requirements
    unsafe impl NSObjectProtocol for ForegroundObserver {}

    impl ForegroundObserver {
        #[unsafe(method(applicationActivated:))]
        fn application_activated(&self, notification: &NSNotification) {
            if let Some(tx) = TX.lock().unwrap().as_ref() {
                emit_from_notification(tx, notification);
            }
        }
    }
);

fn nsstring_to_string(s: &NSObject) -> String {
    unsafe {
        let cstr: *const i8 = msg_send![s, UTF8String];
        if cstr.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(cstr)
            .to_string_lossy()
            .into_owned()
    }
}

fn emit_from_notification(tx: &Sender<TrackerEvent>, notification: &NSNotification) {
    unsafe {
        let user_info: *mut NSObject = msg_send![notification, userInfo];
        if user_info.is_null() {
            return;
        }

        extern "C" {
            static NSWorkspaceApplicationKey: *mut NSObject;
        }
        if NSWorkspaceApplicationKey.is_null() {
            return;
        }

        let app: *mut NSObject =
            msg_send![user_info, objectForKey: &*NSWorkspaceApplicationKey];
        if app.is_null() {
            return;
        }

        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            return;
        }

        let name_obj: *mut NSObject = msg_send![app, localizedName];
        let exe_name = if !name_obj.is_null() {
            nsstring_to_string(&*name_obj)
        } else {
            format!("pid:{}", pid)
        };

        if handle_loginwindow(tx, &exe_name) {
            return;
        }

        let url: *mut NSObject = msg_send![app, bundleURL];
        let exe_path = if !url.is_null() {
            let path_obj: *mut NSObject = msg_send![url, path];
            if !path_obj.is_null() {
                nsstring_to_string(&*path_obj)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let window_title = super::process_info::window_title_for_pid(pid);

        let info = ProcessInfo {
            pid: pid as u32,
            exe_name: exe_name.clone(),
            exe_path,
            window_title,
            window_handle: Some(WindowHandle(pid as usize)),
        };

        if super::fullscreen::is_fullscreen_for_pid(pid) {
            let _ = tx.send(TrackerEvent::FullscreenEntered(info.clone()));
        }

        let _ = tx.send(TrackerEvent::WindowChanged(info));
    }
}

fn emit_current_foreground(tx: &Sender<TrackerEvent>) {
    unsafe {
        let workspace: Retained<NSWorkspace> = NSWorkspace::sharedWorkspace();

        let app: Retained<NSRunningApplication> = msg_send![&*workspace, frontmostApplication];
        let app = Retained::as_ptr(&app) as *mut NSObject;
        if app.is_null() {
            return;
        }

        let pid: i32 = msg_send![app, processIdentifier];
        if pid <= 0 {
            return;
        }

        let name_obj: *mut NSObject = msg_send![app, localizedName];
        let exe_name = if !name_obj.is_null() {
            nsstring_to_string(&*name_obj)
        } else {
            format!("pid:{}", pid)
        };

        if handle_loginwindow(tx, &exe_name) {
            return;
        }

        let url: *mut NSObject = msg_send![app, bundleURL];
        let exe_path = if !url.is_null() {
            let path_obj: *mut NSObject = msg_send![url, path];
            if !path_obj.is_null() {
                nsstring_to_string(&*path_obj)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let window_title = super::process_info::window_title_for_pid(pid);

        let info = ProcessInfo {
            pid: pid as u32,
            exe_name,
            exe_path,
            window_title,
            window_handle: Some(WindowHandle(pid as usize)),
        };

        if super::fullscreen::is_fullscreen_for_pid(pid) {
            let _ = tx.send(TrackerEvent::FullscreenEntered(info.clone()));
        }

        let _ = tx.send(TrackerEvent::WindowChanged(info));
    }
}

pub struct MacOsForegroundTracker;

impl crate::ForegroundTracker for MacOsForegroundTracker {
    fn run(self, tx: Sender<TrackerEvent>) {
        *TX.lock().unwrap() = Some(tx.clone());

        emit_current_foreground(&tx);

        unsafe {
            let observer: Retained<ForegroundObserver> =
                msg_send![ForegroundObserver::class(), new];

            let workspace: Retained<NSWorkspace> = NSWorkspace::sharedWorkspace();

            let notification_center: *mut NSObject =
                msg_send![&*workspace, notificationCenter];

            extern "C" {
                static NSWorkspaceDidActivateApplicationNotification: *mut NSObject;
            }

            let _: () = msg_send![
                &*notification_center,
                addObserver: &*observer,
                selector: sel!(applicationActivated:),
                name: NSWorkspaceDidActivateApplicationNotification,
                object: std::ptr::null::<NSObject>() as *const NSObject
            ];

            std::mem::forget(observer);

            extern "C" {
                fn CFRunLoopRun();
            }
            CFRunLoopRun();
        }
    }
}