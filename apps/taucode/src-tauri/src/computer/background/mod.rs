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
mod menus;
mod overlay;
mod process;
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

#[cfg(test)]
pub use focus_guard::action_restoration;
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
    let ax_eligible = matches!(action, Action::Scroll { .. } | Action::Hotkey { .. })
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

fn refresh_textedit_document(bundle: &str, role: &str, action: &Action, find_disabled: bool) -> bool {
    bundle == "com.apple.TextEdit" && role == "AXTextArea" && find_disabled
        && matches!(action, Action::Click { button, clicks: 1, hold_keys, .. } if button == "left" && hold_keys.is_empty())
}

pub fn screenshot(bundle: &str) -> Result<super::native::Frame, String> {
    let pid =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle))
            .iter()
            .find(|a| !a.isTerminated())
            .map(|a| a.processIdentifier())
            .ok_or("Target app is not running")?;
    let process = process::Binding::bind(pid, bundle)?;
    let (frame, window) = capture::capture(&process)?;
    ax_actions::check_pane(pid, bundle, Some(window.id))?;
    Ok(frame)
}

pub struct Session {
    pub pid: i32,
    pub bundle: String,
    process: process::Binding,
    pub window: Mutex<Option<Window>>,
    pub foreground: AtomicBool,
    pub dragging: AtomicBool,
    pub active: AtomicBool,
    pub guard: focus_guard::Guard,
    pub overlay: overlay::Overlay,
    pub path: Mutex<Option<&'static str>>,
    pub fallback_reason: Mutex<Option<String>>,
    pub menu_title: Mutex<Option<String>>,
    pub menu_resolution: Mutex<Option<menus::Diagnostic>>,
    menus: Mutex<menus::Inventory<ax_actions::Element>>,
    resolved_menu: Mutex<Option<menus::Item<ax_actions::Element>>>,
    ax_fallback: AtomicBool,
    snapshot: Mutex<ax_tree::Snapshot<ax_actions::Element>>,
    selected: Mutex<Option<(u64, usize)>>,
    pressed: Mutex<Option<ax_actions::Element>>,
    keyboard_target: Mutex<Option<ax_actions::Element>>,
    // Role of an elementless typing target whose app may rebuild it in place (see ax_actions::focus_matches).
    keyboard_role: Mutex<Option<String>>,
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
        let process = process::Binding::bind(pid, bundle)?;
        let session = Arc::new(Self {
            pid,
            bundle: bundle.into(),
            process,
            window: Mutex::new(None),
            foreground: AtomicBool::new(false),
            dragging: AtomicBool::new(false),
            active: AtomicBool::new(true),
            guard: focus_guard::Guard::new(pid, previous),
            overlay: overlay::Overlay::new(app, cursor, pid),
            path: Mutex::new(None),
            fallback_reason: Mutex::new(None),
            menu_title: Mutex::new(None),
            menu_resolution: Mutex::new(None),
            menus: Mutex::new(menus::Inventory::default()),
            resolved_menu: Mutex::new(None),
            ax_fallback: AtomicBool::new(false),
            snapshot: Mutex::new(ax_tree::Snapshot::default()),
            selected: Mutex::new(None),
            pressed: Mutex::new(None),
            keyboard_target: Mutex::new(None),
            keyboard_role: Mutex::new(None),
            delivery: Mutex::new(None),
            observed: Mutex::new(None),
            attempted: Mutex::new(None),
        });
        session.guard.start(Arc::downgrade(&session));
        Ok(session)
    }
    pub fn check(&self) -> Result<(), String> {
        self.process.check("session")
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
        let (frame, window) = capture::capture(&self.process)?;
        ax_actions::check_pane(self.pid, &self.bundle, Some(window.id))?;
        self.overlay.set_visible(window.on_screen);
        self.ax_fallback.store(false, Ordering::SeqCst);
        *self.fallback_reason.lock().unwrap() = None;
        *self.path.lock().unwrap() = None;
        *self.menu_title.lock().unwrap() = None;
        *self.menu_resolution.lock().unwrap() = None;
        *self.resolved_menu.lock().unwrap() = None;
        *self.observed.lock().unwrap() = Some(Sha256::digest(frame.png.as_bytes()).into());
        *self.selected.lock().unwrap() = None;
        let mut snapshot = self.snapshot.lock().unwrap();
        snapshot.capture(self.pid, &window)?;
        for (node, handle) in menus::open_elements(self.pid, self.pressed.lock().unwrap().as_ref())
        {
            snapshot.append_menu(node, handle);
        }
        let menus = menus::read(self.pid);
        for item in menus.elements() {
            snapshot.append_menu(item.node(), item.handle.clone());
        }
        *self.menus.lock().unwrap() = menus;
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
        let status = if snapshot.partial > 0 {
            format!("{}; partial: {}", snapshot.status, snapshot.partial)
        } else {
            snapshot.status.into()
        };
        // Keep the original geometry for preflight validation; only grounding
        // uses the visible viewport of long, scrolled document elements.
        let elements: Vec<_> = snapshot.elements.iter().map(|node| {
            let mut node = node.clone();
            node.frame = size.and_then(|size| snapshot.visible_frame(node.index, size));
            node
        }).collect();
        serde_json::json!({"snapshot_id":snapshot.id,"elements":elements,"size":size,"status":status,"partial":snapshot.partial,"walk_attempts":snapshot.attempts,"window_lookup":snapshot.lookup,"menus":self.menus.lock().unwrap().observation()})
    }
    pub fn target_info(&self) -> serde_json::Value {
        let name = NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid)
            .and_then(|app| app.localizedName())
            .map(|name| name.to_string())
            .unwrap_or_else(|| self.bundle.clone());
        serde_json::json!({"bundleId":self.bundle,"pid":self.pid,"name":name,"window":*self.window.lock().unwrap()})
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
        let (node, _) =
            snapshot.resolve_bound(id, index, self.pid, window.id, ax_actions::Element::alive)?;
        if node.role == "AXMenuItem"
            && !matches!(action, Action::Click { button, clicks: 1, hold_keys, .. } if button == "left" && hold_keys.is_empty())
        {
            return Err("Menu elements support only a single AXPress click".into());
        }
        let [x, y, w, h] = snapshot.visible_frame(index, window.size)
            .ok_or("AX element is outside the visible viewport")?;
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
        if let Action::Hotkey { keys } = action {
            let inventory = if keys.is_empty() {
                menus::Inventory::default()
            } else {
                menus::read(self.pid)
            };
            let result = inventory.resolve(keys);
            let item = result.as_ref().ok().copied();
            if !keys.is_empty() {
                let diagnostic = result.err().map(|mut d| {
                    if d.reason == "disabled" { d.window_focus = Some(ax_actions::window_focus(self.pid, window.id)); }
                    d
                });
                *self.menu_resolution.lock().unwrap() = diagnostic;
                if let Some(error) = self
                    .menu_resolution
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|d| d.background_error(self.events_available(action)))
                {
                    return Err(error);
                }
            }
            let target = item.map(menus::Item::target);
            *self.resolved_menu.lock().unwrap() = item.map(|i| menus::Item {
                handle: i.handle.clone(),
                path: i.path.clone(),
                metadata: i.metadata.clone(),
            });
            if let Some(target) = target {
                return Ok(target);
            }
        }
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
            .map(|(id, index)| {
                snapshot
                    .resolve_bound(id, index, self.pid, window.id, ax_actions::Element::alive)
                    .map(|(_, e)| e)
            })
            .transpose()?;
        let mut target = ax_actions::policy_target(self.pid, Some(window.id), action, element)?;
        if let Some((id, index)) = selected {
            let (node, element) = snapshot.resolve(id, index)?;
            if node.role == "AXMenuItem" {
                target.role = "AXMenuItem".into();
                target.label = menus::path(element).ok_or("AX menu path unavailable")?;
            }
        }
        Ok(target)
    }
    pub fn verify_element(&self) -> Result<(), String> {
        let window = self.window()?;
        let Some((id, index)) = *self.selected.lock().unwrap() else {
            return Ok(());
        };
        let snapshot = self.snapshot.lock().unwrap();
        let (prior, element) =
            snapshot.resolve_bound(id, index, self.pid, window.id, ax_actions::Element::alive)?;
        let mut current = ax_tree::Native(&window).read(element);
        if prior.role == "AXMenuItem" {
            current.label = menus::path(element).ok_or_else(|| {
                format!(
                    "{}AX menu path unavailable; capture again",
                    super::runner::STALE_OBSERVATION
                )
            })?;
        }
        if prior.role != current.role
            || prior.label != current.label
            || prior.secure != current.secure
            || (prior.role != "AXMenuItem" && prior.frame != current.frame)
            || !current.enabled
        {
            return Err(format!("{}AX target changed after planning: snapshot={id}, index={index}, role before={} after={}, label_changed={}, secure before={} after={}, frame before={:?} after={:?}, enabled={}; capture again",
                super::runner::STALE_OBSERVATION, prior.role, current.role, prior.label != current.label, prior.secure, current.secure,
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
        if matches!(action, Action::Hotkey { .. }) {
            if let Some(item) = self.resolved_menu.lock().unwrap().as_ref() {
                *self.path.lock().unwrap() = Some("ax-menu");
                *self.menu_title.lock().unwrap() = Some(item.path.clone());
                item.handle.press()?;
                return Ok(true);
            }
            return Ok(false);
        }
        let handled = if let Some((id, index)) = selected {
            let snapshot = self.snapshot.lock().unwrap();
            let (node, element) = snapshot.resolve_bound(
                id,
                index,
                self.pid,
                window.id,
                ax_actions::Element::alive,
            )?;
            if matches!(action, Action::Click { .. }) {
                *self.pressed.lock().unwrap() = Some(element.clone());
            }
            if node.role == "AXMenuItem" {
                *self.path.lock().unwrap() = Some("ax-menu");
                *self.menu_title.lock().unwrap() = Some(node.label.clone());
                element.press()?;
                if let Some(point) = action_point(action) {
                    self.overlay.move_to(point, true);
                }
                return Ok(true);
            }
            let selected_row = ax_actions::select_row(element, action).map_err(|error| {
                *self.path.lock().unwrap() = Some("ax-select");
                error
            })?;
            if selected_row {
                *self.path.lock().unwrap() = Some("ax-select");
                if let Some(point) = action_point(action) {
                    self.overlay.move_to(point, true);
                }
                return Ok(true);
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
                    let (_, button) = snapshot.resolve_bound(
                        id,
                        button,
                        self.pid,
                        window.id,
                        ax_actions::Element::alive,
                    )?;
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
                    let handled = ax_actions::element_action(element, action, &mut reason)?;
                    let disabled = self.menus.lock().unwrap().resolve(&["command".into(), "f".into()])
                        .is_err_and(|diagnostic| diagnostic.reason == "disabled");
                    if !handled && refresh_textedit_document(&self.bundle, &node.role, action, disabled) {
                        let before = (skylight::front_process_pid(), skylight::key_focus_pid());
                        if before.0.is_some() && before.1.is_some()
                            && skylight::refresh_document_window(self.pid, window.id)
                        {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            let after = (skylight::front_process_pid(), skylight::key_focus_pid());
                            *reason = Some(format!("textedit_document_window_refreshed; front_key_before={before:?}; front_key_after={after:?}"));
                            if before != after {
                                return Err("Document-window refresh changed user focus; click cancelled".into());
                            }
                        }
                    }
                    handled
                }
            }
        } else {
            if matches!(action, Action::Click { .. }) {
                *self.pressed.lock().unwrap() = ax_actions::target(self.pid, action_point(action));
            }
            ax_actions::try_action(self.pid, window.id, action, &mut reason)?
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
        self.keyboard_blocked(action).is_none()
    }
    /// Which keyboard-focus check fails for this action, if any.
    pub fn keyboard_blocked(&self, action: &Action) -> Option<&'static str> {
        if !self.keyboard_target_matches() {
            return Some("keyboard_target_unfocused");
        }
        if ax_actions::elementless_keyboard(action, self.selected.lock().unwrap().is_some()) {
            return ax_actions::focused_policy_window(self.pid).is_none().then_some("focused_element_unreadable");
        }
        (matches!(action, Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. })
            && !self.window.lock().unwrap().as_ref().is_some_and(|w| ax_actions::focused_window(self.pid) == Some(w.id)))
            .then_some("focused_window_changed")
    }
    /// Focus diagnostics for a failed check: window IDs and roles only, never field contents.
    pub fn focus_state(&self) -> serde_json::Value {
        let window = self.window.lock().unwrap().as_ref().map(|w| w.id);
        window.map_or(serde_json::Value::Null, |id| ax_actions::window_focus(self.pid, id))
    }
    pub fn prepare_events(&self, action: &Action) -> bool {
        *self.keyboard_target.lock().unwrap() = None;
        *self.keyboard_role.lock().unwrap() = None;
        if matches!(action, Action::Type { .. }) {
            let selected = *self.selected.lock().unwrap();
            let target = if let Some((id, index)) = selected {
                let Some(window) = self.window.lock().unwrap().clone() else { return false; };
                self.snapshot.lock().unwrap()
                    .resolve_bound(id, index, self.pid, window.id, ax_actions::Element::alive)
                    .ok().map(|(_, element)| element.clone())
            } else {
                ax_actions::target(self.pid, action_point(action))
            };
            if target.as_ref().is_none_or(|e| !e.has_keyboard_focus(self.pid)) {
                *self.fallback_reason.lock().unwrap() = Some("background_keyboard_target_not_focused".into());
                return false;
            }
            if selected.is_none() && action_point(action).is_none() {
                *self.keyboard_role.lock().unwrap() = target.as_ref().and_then(|e| e.text("AXRole"));
            }
            *self.keyboard_target.lock().unwrap() = target;
        }
        self.fallback_reason
            .lock()
            .unwrap()
            .get_or_insert_with(|| "action_requires_pixel_events".into());
        if let Some(check) = self.keyboard_blocked(action) {
            let focus = self.focus_state();
            let mut reason = self.fallback_reason.lock().unwrap();
            *reason = Some(format!(
                "background_keyboard_requires_activation: {check}; {}; {focus}",
                reason.as_deref().unwrap_or_default()
            ));
            return false;
        }
        // A fresh AX failure still gets its one requested event attempt, even
        // when the previous step left the screenshot unchanged.
        if self.repeated_without_change(action) && !self.ax_fallback.load(Ordering::SeqCst) {
            return false;
        }
        let Some(window) = self.window.lock().unwrap().clone() else {
            return false;
        };
        let keyboard = match action {
            Action::Type { .. } | Action::Hotkey { .. } | Action::HoldAndPress { .. } => true,
            Action::Click { hold_keys, .. } | Action::Drag { hold_keys, .. } => {
                !hold_keys.is_empty()
            }
            _ => false,
        };
        let id =
            if ax_actions::elementless_keyboard(action, self.selected.lock().unwrap().is_some()) {
                let Some(id) = ax_actions::focused_policy_window(self.pid) else {
                    return false;
                };
                id
            } else {
                window.id
            };
        // Key focus theft would route the user's own keystrokes to the target; wait for a pause instead.
        if keyboard && focus_guard::keyboard_age() < USER_TYPING_IDLE {
            *self.fallback_reason.lock().unwrap() = Some("background_keyboard_user_typing".into());
            return false;
        }
        self.guard.delivering.store(true, Ordering::SeqCst);
        *self.delivery.lock().unwrap() = Some(delivery::Scope::enter(self, self.pid, id, keyboard));
        if self.events_available(action) {
            true
        } else {
            let mut reason = self.fallback_reason.lock().unwrap();
            *reason = Some(format!(
                "background_keyboard_focus_changed; {}",
                reason.as_deref().unwrap_or_default()
            ));
            drop(reason);
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
    /// Stops further key input once the user has typed during this keyboard delivery; nothing is replayed.
    pub fn check_user_typing(&self) -> Result<(), String> {
        if self.delivery.lock().unwrap().as_ref().is_some_and(|scope| scope.user_typed(focus_guard::keyboard_age())) {
            return Err(format!("{}You pressed keys while taucode was typing into {}. macOS may have delivered some of your keystrokes there, so taucode stopped sending input and did not replay anything.", super::runner::UNCERTAIN, self.bundle));
        }
        Ok(())
    }
    fn keyboard_target_matches(&self) -> bool {
        let target = self.keyboard_target.lock().unwrap();
        let Some(element) = target.as_ref() else { return true; };
        let role = self.keyboard_role.lock().unwrap().clone();
        let window = self.window.lock().unwrap().as_ref().map(|w| w.id);
        ax_actions::focus_matches(element.has_keyboard_focus(self.pid), role.as_deref(), || {
            ax_actions::target(self.pid, None).map(|e| (window.is_some_and(|w| e.belongs_to(self.pid, w)), e.text("AXRole").unwrap_or_default()))
        })
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
        self.process.check("begin_foreground")?;
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid)
            .ok_or_else(|| format!("Target activation (begin_foreground): pid={}, NSRunningApplication=nil after successful process check", self.pid))?;
        self.foreground.store(true, Ordering::SeqCst);
        #[allow(deprecated)]
        if !app.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions(0)) {
            return Err("Target activation failed".into());
        }
        ax_actions::restore_window(self.pid, window.id);
        *self.path.lock().unwrap() = Some("foreground");
        Ok(())
    }
    pub fn end_foreground(&self) {
        *self.keyboard_target.lock().unwrap() = None;
        *self.keyboard_role.lock().unwrap() = None;
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

const USER_TYPING_IDLE: f64 = 0.5;

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
