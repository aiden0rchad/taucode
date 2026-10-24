// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Background routing adapted from input/mouse.rs and input/keyboard.rs; see NOTICE.
pub(super) mod ax_actions;
mod ax_tree;
use ax_tree::Provider;
mod bezier;
mod capture;
mod delivery;
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

pub use focus_guard::UserFocus;
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
    let ax_eligible = matches!(action, Action::Scroll { .. })
        || matches!(action, Action::Click { button, clicks: 1, hold_keys, .. } if matches!(button.as_str(), "left" | "right") && hold_keys.is_empty())
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
    let (frame, window) = capture::capture(pid)?;
    ax_actions::check_pane(pid, bundle, Some(window.id))?;
    Ok(frame)
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
    pub fallback_reason: Mutex<Option<String>>,
    ax_fallback: AtomicBool,
    snapshot: Mutex<ax_tree::Snapshot<ax_actions::Element>>,
    selected: Mutex<Option<(u64, usize)>>,
    delivery: Mutex<Option<delivery::Scope>>,
    observed: Mutex<Option<[u8; 32]>>,
    attempted: Mutex<Option<(Action, [u8; 32])>>,
}
impl Session {
    pub async fn bind(
        bundle: &str,
        app: tauri::AppHandle,
        cursor: bool,
        previous: UserFocus,
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
            guard: focus_guard::Guard::new(pid, previous),
            overlay: overlay::Overlay::new(app, cursor),
            path: Mutex::new(None),
            fallback_reason: Mutex::new(None),
            ax_fallback: AtomicBool::new(false),
            snapshot: Mutex::new(ax_tree::Snapshot::default()),
            selected: Mutex::new(None),
            delivery: Mutex::new(None),
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
        self.check().map_err(|e| {
            format!(
                "Process identity check: pid={}, bundle={}, {e}",
                self.pid, self.bundle
            )
        })?;
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
        ax_actions::check_pane(self.pid, &self.bundle, Some(window.id))?;
        self.overlay.set_visible(window.on_screen);
        self.ax_fallback.store(false, Ordering::SeqCst);
        *self.fallback_reason.lock().unwrap() = None;
        *self.observed.lock().unwrap() = Some(Sha256::digest(frame.png.as_bytes()).into());
        *self.selected.lock().unwrap() = None;
        let (root, lookup) = ax_actions::window_element(self.pid, window.id);
        let mut snapshot = self.snapshot.lock().unwrap();
        snapshot.replace(
            &ax_tree::Native(&window),
            root,
            ax_tree::MAX_ELEMENTS,
            ax_tree::SNAPSHOT_BUDGET,
        );
        snapshot.lookup = lookup;
        *self.window.lock().unwrap() = Some(window);
        Ok(frame)
    }
    pub fn window(&self) -> Result<Window, String> {
        self.check()?;
        let mut expected = self
            .window
            .lock()
            .unwrap()
            .clone()
            .ok_or("Capture a target window first")?;
        expected.on_screen = windows::validate(&expected, self.pid)?;
        self.overlay.set_visible(expected.on_screen);
        Ok(expected)
    }
    pub fn snapshot_json(&self) -> serde_json::Value {
        let snapshot = self.snapshot.lock().unwrap();
        let size = self.window.lock().unwrap().as_ref().map(|w| w.size);
        serde_json::json!({"snapshot_id":snapshot.id,"elements":snapshot.elements,"size":size,"status":snapshot.status,"window_lookup":snapshot.lookup})
    }
    pub fn select_element(
        &self,
        target: Option<(u64, usize)>,
        action: &mut Action,
    ) -> Result<(String, bool), String> {
        *self.selected.lock().unwrap() = None;
        let Some((id, index)) = target else {
            return Ok((String::new(), false));
        };
        let window = self.window()?;
        let snapshot = self.snapshot.lock().unwrap();
        let (node, element) = snapshot.resolve(id, index)?;
        if !element.belongs_to(self.pid, window.id) {
            return Err(format!("AX element binding check: pid={}, window={}, snapshot={id}, index={index}, belongs_to=false", self.pid, window.id));
        }
        let [x, y, w, h] = node.frame.ok_or("AX element has no frame")?;
        let point = super::policy::map_point(
            [x + w / 2., y + h / 2.],
            window.size,
            super::policy::Display {
                origin: window.origin,
                width: window.size[0],
                height: window.size[1],
                pixel_width: 1,
                pixel_height: 1,
            },
        )?;
        match action {
            Action::Click { x, y, .. } | Action::Scroll { x, y, .. } => {
                *x = point[0];
                *y = point[1];
            }
            Action::Type { x, y, .. } => {
                *x = Some(point[0]);
                *y = Some(point[1]);
            }
            _ => return Err("Unsupported element action".into()),
        }
        *self.selected.lock().unwrap() = target;
        Ok((format!("{} {}", node.role, node.label), node.secure))
    }
    pub fn policy_target(&self, action: &Action) -> Result<super::policy::Target, String> {
        let window = self.window()?;
        let selected = if matches!(
            action,
            Action::Click { .. } | Action::Type { .. } | Action::Scroll { .. }
        ) {
            *self.selected.lock().unwrap()
        } else {
            None
        };
        let snapshot = self.snapshot.lock().unwrap();
        let element = selected
            .map(|(id, index)| snapshot.resolve(id, index).map(|(_, e)| e))
            .transpose()?;
        ax_actions::policy_target(self.pid, Some(window.id), action, element)
    }
    pub fn verify_element(&self) -> Result<(), String> {
        let Some((id, index)) = *self.selected.lock().unwrap() else {
            return Ok(());
        };
        let window = self.window()?;
        let snapshot = self.snapshot.lock().unwrap();
        let (prior, element) = snapshot.resolve(id, index)?;
        if !element.belongs_to(self.pid, window.id) {
            return Err("AX element left target window".into());
        }
        let current = ax_tree::Native(&window).read(element);
        if prior.role != current.role
            || prior.label != current.label
            || prior.secure != current.secure
            || prior.frame != current.frame
            || !current.enabled
        {
            return Err(format!("AX target changed after planning: snapshot={id}, index={index}, role before={} after={}, label_changed={}, secure before={} after={}, frame before={:?} after={:?}, enabled={}; capture again",
                prior.role, current.role, prior.label != current.label, prior.secure, current.secure,
                prior.frame, current.frame, current.enabled));
        }
        Ok(())
    }
    pub fn try_ax(&self, action: &Action) -> Result<bool, String> {
        let window = self.window()?;
        self.guard.arm();
        let selected = *self.selected.lock().unwrap();
        let mut reason = self.fallback_reason.lock().unwrap();
        *reason = None;
        let handled = if let Some((id, index)) = selected {
            let snapshot = self.snapshot.lock().unwrap();
            let (_, element) = snapshot.resolve(id, index)?;
            if !element.belongs_to(self.pid, window.id) {
                return Err("AX target changed".into());
            }
            if let Action::Scroll {
                clicks,
                shift: false,
                ..
            } = action
            {
                if *clicks == 0 {
                    true
                } else if let Some(button) = snapshot.scroll_button(index, *clicks > 0) {
                    let (_, button) = snapshot.resolve(id, button)?;
                    ax_actions::element_action(
                        button,
                        &Action::Click {
                            x: 0.,
                            y: 0.,
                            button: "left".into(),
                            clicks: 1,
                            hold_keys: vec![],
                        },
                        &mut reason,
                    )?
                } else {
                    false
                }
            } else {
                if let Some(focused) =
                    ax_actions::focused_insertion(self.pid, window.id, element, action)
                {
                    if let Action::Type { text, .. } = action {
                        focused.set_text("AXSelectedText", text, &mut reason)?
                    } else {
                        false
                    }
                } else {
                    ax_actions::element_action(element, action, &mut reason)?
                }
            }
        } else {
            ax_actions::try_action(self.pid, window.id, action)?
        };
        self.ax_fallback.store(!handled, Ordering::SeqCst);
        if !handled && reason.is_none() {
            *reason = Some(
                if selected.is_some() {
                    "ax_element_action_unavailable"
                } else {
                    "ax_hit_test_unavailable_or_rejected"
                }
                .into(),
            );
        }
        if handled {
            *self.path.lock().unwrap() = Some(ax_path(selected.is_some()));
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
        self.fallback_reason
            .lock()
            .unwrap()
            .get_or_insert_with(|| "action_requires_pixel_events".into());
        // A fresh AX failure still gets its one requested event attempt, even
        // when the previous step left the screenshot unchanged.
        if self.repeated_without_change(action) && !self.ax_fallback.load(Ordering::SeqCst) {
            return false;
        }
        let Some(window) = self.window.lock().unwrap().clone() else {
            return false;
        };
        self.guard.delivering.store(true, Ordering::SeqCst);
        let keyboard = match action {
            Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. } => true,
            Action::Click { hold_keys, .. } | Action::Drag { hold_keys, .. } => {
                !hold_keys.is_empty()
            }
            _ => false,
        };
        *self.delivery.lock().unwrap() =
            Some(delivery::Scope::enter(self, self.pid, window.id, keyboard));
        if self.events_available(action) {
            true
        } else {
            self.end_delivery(false);
            false
        }
    }
    pub fn finish_delivery(self: &Arc<Self>, cancelled: bool) -> Option<Box<dyn FnOnce() + Send>> {
        let mut scope = self.delivery.lock().unwrap().take()?;
        scope.release_key(&**self);
        let session = self.clone();
        Some(Box::new(move || {
            scope.finish(&*session, !cancelled);
            session.guard.delivering.store(false, Ordering::SeqCst);
        }))
    }
    fn end_delivery(&self, settle: bool) {
        let scope = self.delivery.lock().unwrap().take();
        if let Some(scope) = scope {
            scope.finish(self, settle);
        }
        self.guard.delivering.store(false, Ordering::SeqCst);
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
        self.end_delivery(false);
        self.overlay.hide();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn ax_path(selected: bool) -> &'static str {
    if selected {
        "ax-element"
    } else {
        "ax-hit-test"
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
    *session.path.lock().unwrap() =
        Some(event_path(session.ax_fallback.load(Ordering::SeqCst), path));
}
fn event_path(_ax_fallback: bool, _path: &'static str) -> &'static str {
    "pixel-skylight"
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
