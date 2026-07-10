use std::cell::RefCell;
use std::sync::mpsc::Sender;

use objc2::class;
use objc2::declare::ClassType;
use objc2::{declare_class, msg_send, msg_send_id, sel};
use objc2::rc::Retained;
use objc2::runtime::NSObject;

use crate::{ProcessInfo, TrackerEvent, WindowHandle};

thread_local! {
    static TX: RefCell<Option<Sender<TrackerEvent>>> = RefCell::new(None);
}

declare_class!(
    struct ForegroundObserver;

    unsafe impl ClassType for ForegroundObserver {
        type Super = NSObject;
        type Mutability = objc2::mutability::IsRetained;
    }

    extern "C" {
        #[sel(applicationActivated:)]
        fn application_activated(&self, notification: &NSObject);
    }
);

extern "C" fn ForegroundObserver_application_activated(
    _this: &ForegroundObserver,
    notification: &NSObject,
) {
    TX.with(|cell| {
        if let Some(tx) = cell.borrow().as_ref() {
            emit_from_notification(tx, notification);
        }
    });
}

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

fn emit_from_notification(tx: &Sender<TrackerEvent>, notification: &NSObject) {
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
            msg_send![user_info, objectForKey: *NSWorkspaceApplicationKey];
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

fn emit_current_foreground(tx: &Sender<TrackerEvent>) {
    unsafe {
        let workspace: Retained<NSObject> =
            msg_send_id![class!(NSWorkspace), sharedWorkspace];
        let workspace = &*workspace as *const NSObject as *mut NSObject;

        let app: *mut NSObject = msg_send![&*workspace, frontmostApplication];
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
        TX.with(|cell| *cell.borrow_mut() = Some(tx.clone()));

        emit_current_foreground(&tx);

        unsafe {
            let observer: Retained<ForegroundObserver> =
                msg_send_id![ForegroundObserver::class(), new];

            let workspace: Retained<NSObject> =
                msg_send_id![class!(NSWorkspace), sharedWorkspace];
            let workspace = &*workspace as *const NSObject as *mut NSObject;

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
