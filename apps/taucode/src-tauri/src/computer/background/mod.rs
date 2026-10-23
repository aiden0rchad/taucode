// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Background routing adapted from input/mouse.rs and input/keyboard.rs; see NOTICE.
mod ax_actions;
mod bezier;
mod capture;
mod focus_guard;
mod overlay;
pub(super) mod skylight;
mod windows;

use super::policy::{Action, Context};
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSArray, NSString};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub use windows::Window;

#[derive(Debug, PartialEq)]
pub enum Path {
    Ax,
    Events,
    ApprovalRequired,
    Unavailable,
}

// The caller must not retry a partially dispatched action: CGEvent has no delivery acknowledgment.
pub fn choose_path(
    action: &Action,
    ax_available: bool,
    events_available: bool,
    fallback: bool,
) -> Path {
    let ax_eligible = matches!(action, Action::Click { button, clicks: 1, hold_keys, .. } if button == "left" && hold_keys.is_empty())
        || matches!(action, Action::Type { enter: false, .. });
    if ax_eligible && ax_available {
        Path::Ax
    } else if events_available {
        Path::Events
    } else if fallback {
        Path::ApprovalRequired
    } else {
        Path::Unavailable
    }
}
pub fn prepare_action(
    action: &Action,
    ax: impl FnOnce() -> Result<bool, String>,
    events: impl FnOnce() -> bool,
    fallback: bool,
) -> Result<Path, String> {
    let handled = choose_path(action, true, false, false) == Path::Ax && ax()?;
    Ok(choose_path(action, handled, !handled && events(), fallback))
}

pub fn screenshot(bundle: &str) -> Result<super::native::Frame, String> {
    let pid =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle))
            .iter()
            .find(|a| !a.isTerminated())
            .map(|a| a.processIdentifier())
            .ok_or("Target app is not running")?;
    capture::capture(pid).map(|(frame, _)| frame)
}

pub struct Session {
    pub pid: i32,
    pub bundle: String,
    pub window: Mutex<Option<Window>>,
    pub foreground: AtomicBool,
    pub dragging: AtomicBool,
    pub active: AtomicBool,
    pub guard: focus_guard::Guard,
    pub overlay: overlay::Overlay,
    pub path: Mutex<Option<&'static str>>,
    observed: Mutex<Option<[u8; 32]>>,
    attempted: Mutex<Option<(Action, [u8; 32])>>,
}
impl Session {
    pub async fn bind(
        bundle: &str,
        app: tauri::AppHandle,
        cursor: bool,
    ) -> Result<Arc<Self>, String> {
        let running = || {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
                bundle,
            ))
            .iter()
            .find(|a| !a.isTerminated())
            .map(|a| a.processIdentifier())
        };
        let pid = if let Some(pid) = running() {
            pid
        } else {
            {
                let url = NSWorkspace::sharedWorkspace()
                    .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle))
                    .ok_or("Target application is not installed")?;
                let config = NSWorkspaceOpenConfiguration::configuration();
                config.setActivates(false);
                NSWorkspace::sharedWorkspace()
                    .openApplicationAtURL_configuration_completionHandler(&url, &config, None);
            }
            let mut pid = None;
            for _ in 0..100 {
                if let Some(value) = running() {
                    pid = Some(value);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            pid.ok_or("Target application did not launch in background")?
        };
        let session = Arc::new(Self {
            pid,
            bundle: bundle.into(),
            window: Mutex::new(None),
            foreground: AtomicBool::new(false),
            dragging: AtomicBool::new(false),
            active: AtomicBool::new(true),
            guard: focus_guard::Guard::new(pid),
            overlay: overlay::Overlay::new(app, cursor),
            path: Mutex::new(None),
            observed: Mutex::new(None),
            attempted: Mutex::new(None),
        });
        session.guard.start(Arc::downgrade(&session));
        Ok(session)
    }
    pub fn check(&self) -> Result<(), String> {
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid)
            .ok_or("Target process exited")?;
        if app.isTerminated()
            || app
                .bundleIdentifier()
                .is_none_or(|id| id.to_string() != self.bundle)
        {
            return Err("Target process binding changed".into());
        }
        Ok(())
    }
    pub fn context(&self) -> Result<Context, String> {
        self.check()?;
        Ok(Context {
            app: self.bundle.clone(),
            secure: ax_actions::secure(self.pid),
        })
    }
    pub fn open(&self, app_or_file: &str) -> Result<(), String> {
        self.check()?;
        if app_or_file == self.bundle {
            return Ok(());
        }
        let (bundle, url) = super::native::target(app_or_file)?;
        if bundle != self.bundle {
            return Err("File association changed after approval".into());
        }
        let id = NSString::from_str(&bundle);
        if NSRunningApplication::runningApplicationsWithBundleIdentifier(&id).len() != 1 {
            return Err("Cannot route file open to one target process".into());
        }
        let workspace = NSWorkspace::sharedWorkspace();
        let application = workspace
            .URLForApplicationWithBundleIdentifier(&id)
            .ok_or("Target application unavailable")?;
        let config = NSWorkspaceOpenConfiguration::configuration();
        config.setActivates(false);
        self.guard.arm();
        workspace.openURLs_withApplicationAtURL_configuration_completionHandler(
            &NSArray::from_retained_slice(&[url]),
            &application,
            &config,
            None,
        );
        Ok(())
    }
    pub fn capture(&self) -> Result<super::native::Frame, String> {
        self.check()?;
        let (frame, window) = capture::capture(self.pid)?;
        *self.observed.lock().unwrap() = Some(Sha256::digest(frame.png.as_bytes()).into());
        *self.window.lock().unwrap() = Some(window);
        Ok(frame)
    }
    pub fn window(&self) -> Result<Window, String> {
        self.check()?;
        let expected = self
            .window
            .lock()
            .unwrap()
            .clone()
            .ok_or("Capture a target window first")?;
        windows::validate(&expected, self.pid)?;
        Ok(expected)
    }
    pub fn try_ax(&self, action: &Action) -> Result<bool, String> {
        let window = self.window()?;
        self.guard.arm();
        let handled = ax_actions::try_action(self.pid, window.id, action)?;
        if handled {
            *self.path.lock().unwrap() = Some("ax");
            if let Some(point) = action_point(action) {
                self.overlay
                    .move_to(point, matches!(action, Action::Click { .. }));
            }
        }
        Ok(handled)
    }
    pub fn events_available(&self, action: &Action) -> bool {
        !matches!(
            action,
            Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. }
        ) || self
            .window
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|w| ax_actions::focused_window(self.pid) == Some(w.id))
    }
    pub fn prepare_events(&self, action: &Action) -> bool {
        if self.repeated_without_change(action) {
            return false;
        }
        let window = self.window.lock().unwrap().clone();
        if let Some(w) = window {
            if ax_actions::focused_window(self.pid) != Some(w.id)
                && skylight::prime_window(self.pid, w.id)
            {
                // Upstream target-only AppKit key-window settle interval.
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        self.events_available(action)
    }
    pub fn repeated_without_change(&self, action: &Action) -> bool {
        let observed = *self.observed.lock().unwrap();
        self.attempted
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(prior, image)| prior == action && observed == Some(*image))
    }
    pub fn record_attempt(&self, action: &Action) {
        if let Some(image) = *self.observed.lock().unwrap() {
            *self.attempted.lock().unwrap() = Some((action.clone(), image));
        }
    }
    pub fn begin_foreground(&self) -> Result<(), String> {
        let window = self.window()?;
        self.foreground.store(true, Ordering::SeqCst);
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid)
            .ok_or("Target process exited")?;
        #[allow(deprecated)]
        if !app.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions(0)) {
            return Err("Target activation failed".into());
        }
        ax_actions::restore_window(self.pid, window.id);
        *self.path.lock().unwrap() = Some("foreground");
        Ok(())
    }
    pub fn end_foreground(&self) {
        if self.foreground.swap(false, Ordering::SeqCst) {
            self.guard.restore();
        }
    }
    pub fn stop(&self) {
        if !self.active.swap(false, Ordering::SeqCst) {
            return;
        }
        self.end_foreground();
        self.overlay.hide();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn action_point(a: &Action) -> Option<[f64; 2]> {
    match a {
        Action::Click { x, y, .. } | Action::Scroll { x, y, .. } => Some([*x, *y]),
        Action::Type {
            x: Some(x),
            y: Some(y),
            ..
        } => Some([*x, *y]),
        Action::Drag { to, .. } => Some(*to),
        _ => None,
    }
}

// Copyright (c) 2025 Cua AI, Inc. MIT: input/mouse.rs post_mouse_event_with_mode
// and input/keyboard.rs post_keyboard_event. See NOTICE.
pub fn post_event(
    session: &Session,
    event: &core_graphics::event::CGEvent,
    keyboard: bool,
    point: [f64; 2],
    window: u32,
    group: i64,
) {
    let ptr = (&**event as *const core_graphics::event::CGEventRef)
        .cast_mut()
        .cast();
    if !keyboard {
        event.set_location(core_graphics::geometry::CGPoint::new(point[0], point[1]));
        skylight::set_window_location(ptr, point[0], point[1]);
        for (field, value) in [
            (51, window as i64),
            (58, group),
            (91, window as i64),
            (92, window as i64),
        ] {
            event.set_integer_value_field(field, value);
            skylight::set_integer_field(ptr, field, value);
        }
    }
    event.set_integer_value_field(40, session.pid as i64);
    skylight::set_integer_field(ptr, 40, session.pid as i64);
    // core-graphics 0.25 gates this public API behind elcapitan; use the already-linked symbol.
    let path = post_route(
        skylight::post_to_pid(session.pid, ptr, keyboard),
        || unsafe { CGEventPostToPid(session.pid, ptr) },
    );
    *session.path.lock().unwrap() = Some(path);
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventPostToPid(pid: i32, event: *mut std::ffi::c_void);
}
pub fn post_route(skylight_posted: bool, public_post: impl FnOnce()) -> &'static str {
    if skylight_posted {
        "skylight"
    } else {
        public_post();
        "postToPid"
    }
}

#[cfg(test)]
mod tests;
