// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from platform-macos/src/input/ax_actions.rs and ax/bindings.rs; see NOTICE.
use super::super::policy::Action;
use super::action_point;
use objc2::{msg_send, runtime::AnyObject};
use objc2_foundation::NSString;
use std::ffi::c_void;

type Ref = *const c_void;
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueGetValue(value: Ref, kind: u32, out: *mut c_void) -> bool;
    fn AXValueGetType(value: Ref) -> u32;
    fn AXValueGetTypeID() -> usize;
    fn AXUIElementCreateApplication(pid: i32) -> Ref;
    fn AXUIElementSetMessagingTimeout(element: Ref, seconds: f32) -> i32;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementCopyElementAtPosition(element: Ref, x: f32, y: f32, value: *mut Ref) -> i32;
    fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    fn AXUIElementCopyActionNames(element: Ref, value: *mut Ref) -> i32;
    fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
    fn AXUIElementIsAttributeSettable(element: Ref, attribute: Ref, value: *mut u8) -> i32;
    fn AXUIElementSetAttributeValue(element: Ref, attribute: Ref, value: Ref) -> i32;
    fn _AXUIElementGetWindow(element: Ref, window: *mut u32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRetain(value: Ref) -> Ref;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFRelease(value: Ref);
    fn CFArrayGetCount(value: Ref) -> isize;
    fn CFArrayGetValueAtIndex(value: Ref, index: isize) -> Ref;
}
pub struct Element(Ref);
// Retained AX proxies are thread-safe CF objects; access is serialized by the session mutex.
unsafe impl Send for Element {}
impl Clone for Element {
    fn clone(&self) -> Self {
        Self(unsafe { CFRetain(self.0) })
    }
}
impl Drop for Element {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
fn string(s: &NSString) -> Ref {
    (s as *const NSString).cast()
}
impl Element {
    fn owned(ptr: Ref) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            unsafe {
                AXUIElementSetMessagingTimeout(ptr, 0.005);
            }
            Some(Self(ptr))
        }
    }
    pub(super) fn reading<T>(&self, read: impl FnOnce() -> T) -> T {
        self.with_timeout(0.05, read)
    }
    fn with_timeout<T>(&self, seconds: f32, call: impl FnOnce() -> T) -> T {
        struct Reset<'a>(&'a Element);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                unsafe {
                    AXUIElementSetMessagingTimeout(self.0 .0, 0.005);
                }
            }
        }
        unsafe {
            AXUIElementSetMessagingTimeout(self.0, seconds);
        }
        let _reset = Reset(self);
        call()
    }
    pub(super) fn app(pid: i32) -> Option<Self> {
        Self::owned(unsafe { AXUIElementCreateApplication(pid) })
    }
    pub(super) fn attr(&self, name: &str) -> Option<Self> {
        self.attr_result(name).ok()
    }
    fn attr_result(&self, name: &str) -> Result<Self, String> {
        let mut result = std::ptr::null();
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.0, string(&NSString::from_str(name)), &mut result)
        };
        if status == 0 {
            Self::owned_value(result).ok_or_else(|| format!("{name}: null attribute"))
        } else {
            Err(format!("{name}: ax_error({status})"))
        }
    }
    // Attribute results may be strings/arrays/AXValues, not AXUIElements.
    fn owned_value(ptr: Ref) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }
    fn equals(&self, value: &str) -> bool {
        unsafe { msg_send![self.0 as *const AnyObject, isEqual: &*NSString::from_str(value)] }
    }
    pub(super) fn settable(&self, name: &str) -> bool {
        let mut value = 0;
        unsafe {
            AXUIElementIsAttributeSettable(self.0, string(&NSString::from_str(name)), &mut value)
                == 0
                && value != 0
        }
    }
    pub(super) fn window_id(&self) -> Option<u32> {
        self.window_id_result().ok()
    }
    fn window_id_result(&self) -> Result<u32, String> {
        let mut id = 0;
        let status = unsafe { _AXUIElementGetWindow(self.0, &mut id) };
        if status == 0 && id != 0 {
            Ok(id)
        } else {
            Err(format!(
                "_AXUIElementGetWindow: ax_error({status}), id={id}"
            ))
        }
    }
    pub(super) fn text(&self, name: &str) -> Option<String> {
        let value = self.attr(name)?;
        unsafe {
            let kind = CFGetTypeID(value.0);
            if kind != CFStringGetTypeID()
                && kind != CFNumberGetTypeID()
                && kind != CFBooleanGetTypeID()
            {
                return None;
            }
            let text: *const NSString = msg_send![value.0 as *const AnyObject, description];
            text.as_ref().map(|s| s.to_string())
        }
    }
    pub(super) fn children(&self, name: &str, limit: usize) -> Vec<Self> {
        self.children_result(name, limit).unwrap_or_default()
    }
    fn children_result(&self, name: &str, limit: usize) -> Result<Vec<Self>, String> {
        if limit == 0 {
            return Ok(vec![]);
        }
        let array = self.attr_result(name)?;
        unsafe {
            if CFGetTypeID(array.0) != CFArrayGetTypeID() {
                return Err(format!("{name}: not an AX array"));
            }
            Ok((0..CFArrayGetCount(array.0).min(limit as isize))
                .filter_map(|i| Self::owned(CFRetain(CFArrayGetValueAtIndex(array.0, i))))
                .collect())
        }
    }
    pub(super) fn actions(&self) -> Vec<String> {
        let mut ptr = std::ptr::null();
        if unsafe { AXUIElementCopyActionNames(self.0, &mut ptr) } != 0 {
            return vec![];
        }
        let Some(array) = Self::owned_value(ptr) else {
            return vec![];
        };
        unsafe {
            if CFGetTypeID(array.0) != CFArrayGetTypeID() {
                return vec![];
            }
            (0..CFArrayGetCount(array.0).min(32))
                .filter_map(|i| {
                    let ptr = CFArrayGetValueAtIndex(array.0, i);
                    (CFGetTypeID(ptr) == CFStringGetTypeID())
                        .then(|| (&*(ptr as *const NSString)).to_string())
                })
                .collect()
        }
    }
    pub(super) fn frame(&self) -> Option<[f64; 4]> {
        let position = self.attr("AXPosition")?;
        let size = self.attr("AXSize")?;
        let mut p = [0f64; 2];
        let mut s = [0f64; 2];
        unsafe {
            if CFGetTypeID(position.0) != AXValueGetTypeID()
                || CFGetTypeID(size.0) != AXValueGetTypeID()
                || AXValueGetType(position.0) != 1
                || AXValueGetType(size.0) != 2
                || !AXValueGetValue(position.0, 1, p.as_mut_ptr().cast())
                || !AXValueGetValue(size.0, 2, s.as_mut_ptr().cast())
            {
                return None;
            }
        }
        let frame = [p[0], p[1], s[0], s[1]];
        frame.iter().all(|n| n.is_finite()).then_some(frame)
    }
    pub(super) fn enabled(&self) -> bool {
        self.text("AXEnabled").as_deref() != Some("0")
    }
    pub(super) fn belongs_to(&self, pid: i32, window: u32) -> bool {
        let mut owner = 0;
        unsafe {
            AXUIElementSetMessagingTimeout(self.0, 0.005);
        }
        (unsafe { AXUIElementGetPid(self.0, &mut owner) == 0 })
            && owner == pid
            && self
                .window_id()
                .or_else(|| self.attr("AXWindow").and_then(|w| w.window_id()))
                == Some(window)
    }
    pub(super) fn set_text(
        &self,
        attribute: &str,
        text: &str,
        reason: &mut Option<String>,
    ) -> Result<bool, String> {
        action_result(attribute, self.apply(attribute, Some(text)), reason)
    }
}
fn target(pid: i32, point: Option<[f64; 2]>) -> Option<Element> {
    let app = Element::app(pid)?;
    let element = if let Some(p) = point {
        let mut element = std::ptr::null();
        if unsafe {
            AXUIElementCopyElementAtPosition(app.0, p[0] as f32, p[1] as f32, &mut element)
        } != 0
        {
            return None;
        }
        Element::owned(element)?
    } else {
        app.attr("AXFocusedUIElement")?
    };
    unsafe {
        AXUIElementSetMessagingTimeout(element.0, 0.005);
    }
    let mut owner = 0;
    if unsafe { AXUIElementGetPid(element.0, &mut owner) } != 0 || owner != pid {
        return None;
    }
    Some(element)
}
pub fn secure(pid: i32) -> bool {
    target(pid, None).is_none_or(|e| {
        let role = e.text("AXRole").unwrap_or_default();
        role.is_empty()
            || super::super::policy::secure_field(&role, &e.text("AXSubrole").unwrap_or_default())
    })
}
fn field_info(mut read: impl FnMut(&str) -> Option<String>) -> super::super::policy::Target {
    let role = read("AXRole").unwrap_or_default();
    let static_label = if role == "AXStaticText" {
        read("AXValue").unwrap_or_default()
    } else {
        String::new()
    };
    let mut info = super::super::policy::Target {
        secure: role.is_empty()
            || super::super::policy::secure_field(&role, &read("AXSubrole").unwrap_or_default()),
        role,
        label: [
            "AXTitle",
            "AXDescription",
            "AXHelp",
            "AXIdentifier",
            "AXPlaceholderValue",
        ]
        .iter()
        .filter_map(|a| read(a))
        .collect::<Vec<_>>()
        .join(" "),
        ..Default::default()
    };
    info.label.push_str(&format!(" {static_label}"));
    info.label = info.label.trim().to_owned();
    info
}
// Inspect only AX metadata, never field values.
fn row_label(row: &Element) -> String {
    let mut labels = vec![field_info(|a| row.text(a)).label];
    let mut pending = row.children("AXChildren", 32);
    for _ in 0..64 {
        let Some(e) = pending.pop() else {
            break;
        };
        labels.push(field_info(|a| e.text(a)).label);
        if pending.len() < 64 {
            pending.extend(e.children("AXChildren", 16));
        }
    }
    labels.join(" ")
}
pub fn pane_info(pid: i32, window: Option<u32>) -> Result<super::super::policy::Target, String> {
    let root = if let Some(id) = window {
        window_element(pid, id).0
    } else {
        Element::app(pid).and_then(|a| a.attr("AXFocusedWindow"))
    }
    .ok_or("System Settings read-only access: cannot inspect current pane")?;
    let mut info = super::super::policy::Target {
        window_title: root.text("AXTitle").unwrap_or_default(),
        ..Default::default()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut pending = vec![(root, 0)];
    for _ in 0..512 {
        let Some((e, depth)) = pending.pop() else {
            break;
        };
        if std::time::Instant::now() >= deadline || depth > 25 {
            return Err("System Settings read-only access: incomplete pane inspection".into());
        }
        let role = e.text("AXRole").unwrap_or_default();
        if role == "AXSheet" {
            info.window_title
                .push_str(&format!(" {}", field_info(|a| e.text(a)).label));
        }
        if matches!(role.as_str(), "AXRow" | "AXOutlineRow" | "AXCell")
            && e.text("AXSelected").as_deref() == Some("1")
        {
            info.selected_rows.push_str(&format!(" {}", row_label(&e)));
        }
        for row in e.children("AXSelectedRows", 32) {
            info.selected_rows
                .push_str(&format!(" {}", row_label(&row)));
        }
        pending.extend(
            e.children("AXChildren", 512)
                .into_iter()
                .map(|child| (child, depth + 1)),
        );
    }
    if !pending.is_empty() {
        return Err("System Settings read-only access: pane inspection limit reached".into());
    }
    if matches!(info.window_title.trim(), "" | "System Settings")
        && info.selected_rows.trim().is_empty()
    {
        return Err("System Settings read-only access: cannot identify current pane".into());
    }
    Ok(info)
}
pub fn check_pane(pid: i32, bundle: &str, window: Option<u32>) -> Result<(), String> {
    if bundle.eq_ignore_ascii_case(super::super::policy::SYSTEM_SETTINGS) {
        super::super::policy::check_pane(bundle, &pane_info(pid, window)?)?;
    }
    Ok(())
}
pub fn policy_target(
    pid: i32,
    window: Option<u32>,
    action: &Action,
    selected: Option<&Element>,
) -> Result<super::super::policy::Target, String> {
    let hit = target(pid, action_point(action));
    let element = selected.or(hit.as_ref());
    let insertion = element.and_then(|e| window.and_then(|w| focused_insertion(pid, w, e, action)));
    let effective = insertion.as_ref().or(element);
    if let (Some(e), Some(id)) = (effective, window) {
        if !e.belongs_to(pid, id) {
            return Err("AX policy target left the captured window".into());
        }
    }
    let mut info = effective
        .map(|e| field_info(|a| e.text(a)))
        .unwrap_or_default();
    // A static-text child of a setting control must inherit the control's restriction.
    if matches!(action, Action::Click { .. }) {
        let mut parent = effective.and_then(|e| e.attr("AXParent"));
        for _ in 0..8 {
            let Some(e) = parent else {
                break;
            };
            let role = e.text("AXRole").unwrap_or_default();
            if matches!(
                role.as_str(),
                "AXCheckBox"
                    | "AXSwitch"
                    | "AXRadioButton"
                    | "AXSlider"
                    | "AXIncrementor"
                    | "AXPopUpButton"
                    | "AXComboBox"
                    | "AXTextField"
                    | "AXTextArea"
            ) {
                info.role = role.clone();
            }
            if matches!(role.as_str(), "AXRow" | "AXOutlineRow" | "AXCell") {
                info.label.push_str(&format!(" {}", row_label(&e)));
                break;
            }
            if role == "AXWindow" {
                break;
            }
            info.label
                .push_str(&format!(" {}", field_info(|a| e.text(a)).label));
            parent = e.attr("AXParent");
        }
        if let Some(e) =
            effective.filter(|_| matches!(info.role.as_str(), "AXRow" | "AXOutlineRow" | "AXCell"))
        {
            info.label.push_str(&format!(" {}", row_label(e)));
        }
    }
    let app = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|a| a.bundleIdentifier())
        .ok_or("Target application unavailable")?
        .to_string();
    if app.eq_ignore_ascii_case(super::super::policy::SYSTEM_SETTINGS) {
        let mut pane = pane_info(pid, window)?;
        if window.is_none() {
            if let Some(id) = element.and_then(|e| {
                e.window_id()
                    .or_else(|| e.attr("AXWindow").and_then(|w| w.window_id()))
            }) {
                let pointed = pane_info(pid, Some(id))?;
                pane.window_title
                    .push_str(&format!(" {}", pointed.window_title));
                pane.selected_rows
                    .push_str(&format!(" {}", pointed.selected_rows));
            }
        }
        info.window_title = pane.window_title;
        info.selected_rows = pane.selected_rows;
    }
    Ok(info)
}
pub fn focused_window(pid: i32) -> Option<u32> {
    Element::app(pid)?.attr("AXFocusedWindow")?.window_id()
}

pub fn restore_window(pid: i32, id: u32) {
    let Some(app) = Element::app(pid) else {
        return;
    };
    let Some(windows) = app.attr("AXWindows") else {
        return;
    };
    unsafe {
        for i in 0..CFArrayGetCount(windows.0) {
            let ptr = CFArrayGetValueAtIndex(windows.0, i);
            let window = std::mem::ManuallyDrop::new(Element(ptr));
            if window.window_id() == Some(id) {
                AXUIElementSetAttributeValue(
                    app.0,
                    string(&NSString::from_str("AXFocusedWindow")),
                    ptr,
                );
                break;
            }
        }
    }
}

pub fn try_action(pid: i32, window: u32, action: &Action) -> Result<bool, String> {
    if super::choose_path(action, true, false, false) != super::Path::Ax {
        return Ok(false);
    }
    let Some(element) = target(pid, action_point(action)) else {
        return Ok(false);
    };
    if element
        .window_id()
        .or_else(|| element.attr("AXWindow").and_then(|w| w.window_id()))
        != Some(window)
    {
        return Ok(false);
    }
    if element.attr("AXEnabled").is_some_and(|v| unsafe {
        let enabled: bool = msg_send![v.0 as *const AnyObject, boolValue];
        !enabled
    }) {
        return Ok(false);
    }
    let status = match action {
        Action::Click { button, .. } if button == "left" => {
            let mut actions = std::ptr::null();
            if unsafe { AXUIElementCopyActionNames(element.0, &mut actions) } != 0 {
                return Ok(false);
            }
            let Some(actions) = Element::owned_value(actions) else {
                return Ok(false);
            };
            let press = NSString::from_str("AXPress");
            let supported = unsafe {
                (0..CFArrayGetCount(actions.0)).any(|i| {
                    let name = CFArrayGetValueAtIndex(actions.0, i);
                    msg_send![name as *const AnyObject, isEqual: &*press]
                })
            };
            if !supported {
                return Ok(false);
            }
            unsafe { AXUIElementPerformAction(element.0, string(&press)) }
        }
        Action::Type {
            text, overwrite, ..
        } => {
            let text_field = element.attr("AXRole").is_some_and(|r| {
                r.equals("AXTextField") || r.equals("AXTextArea") || r.equals("AXComboBox")
            });
            let attribute = if *overwrite && text_field && element.settable("AXValue") {
                "AXValue"
            } else if element.settable("AXSelectedText") {
                "AXSelectedText"
            } else {
                return Ok(false);
            };
            unsafe {
                AXUIElementSetAttributeValue(
                    element.0,
                    string(&NSString::from_str(attribute)),
                    string(&NSString::from_str(text)),
                )
            }
        }
        _ => return Ok(false),
    };
    action_status(status)
}

pub fn action_status(status: i32) -> Result<bool, String> {
    match status {
        0 => Ok(true),
        // CannotComplete, AttributeUnsupported, ActionUnsupported: one event fallback.
        -25204 | -25205 | -25206 => Ok(false),
        _ => Err(format!("AX action failed ({status})")),
    }
}

// Keep exact errors for both the durable step log and sidecar.log. Preserve the
// M3b fallback allowlist; invalid handles/API-disabled remain terminal errors.
fn action_result(name: &str, status: i32, reason: &mut Option<String>) -> Result<bool, String> {
    if status == 0 {
        return Ok(true);
    }
    let operation = match name {
        "AXPress" => "ax_press",
        "AXPick" => "ax_pick",
        "AXShowMenu" => "ax_show_menu",
        "AXValue" => "ax_set_value",
        "AXSelectedText" => "ax_set_selected_text",
        _ => "ax_action",
    };
    let error = format!("{operation}_failed({status})");
    *reason = Some(error.clone());
    action_status(status).map_err(|_| error)
}

pub(super) fn window_element(pid: i32, id: u32) -> (Option<Element>, serde_json::Value) {
    let Some(app) = Element::app(pid) else {
        return (
            None,
            serde_json::json!({"pid":pid,"window_id":id,"error":"AXUIElementCreateApplication returned null"}),
        );
    };
    find_window(
        pid,
        id,
        |attribute| app.with_timeout(0.25, || app.children_result(attribute, 128)),
        |window| window.with_timeout(0.25, || window.window_id_result()),
    )
}

// AXWindows works for background apps; AXChildren supplements it, as in Cua's
// copy_top_level. Never substitute another window or activate the target app.
fn find_window<H>(
    pid: i32,
    id: u32,
    mut list: impl FnMut(&str) -> Result<Vec<H>, String>,
    window_id: impl Fn(&H) -> Result<u32, String>,
) -> (Option<H>, serde_json::Value) {
    let deadline = std::time::Instant::now() + super::ax_tree::SNAPSHOT_BUDGET;
    let mut attempts = vec![];
    for attempt in 1..=2 {
        for attribute in ["AXWindows", "AXChildren"] {
            if std::time::Instant::now() >= deadline {
                return (
                    None,
                    serde_json::json!({"pid":pid,"window_id":id,"error":"window_lookup_time_budget","attempts":attempts}),
                );
            }
            let mut row = serde_json::json!({"attempt":attempt,"attribute":attribute});
            match list(attribute) {
                Ok(windows) => {
                    row["count"] = serde_json::json!(windows.len());
                    let mut candidates = vec![];
                    for window in windows {
                        if std::time::Instant::now() >= deadline {
                            break;
                        }
                        match window_id(&window) {
                            Ok(found) if found == id => {
                                row["matched_id"] = serde_json::json!(found);
                                attempts.push(row);
                                return (
                                    Some(window),
                                    serde_json::json!({"pid":pid,"window_id":id,"attempts":attempts}),
                                );
                            }
                            Ok(found) => candidates.push(serde_json::json!({"id":found})),
                            Err(error) => candidates.push(serde_json::json!({"error":error})),
                        }
                    }
                    row["candidates"] = serde_json::json!(candidates);
                }
                Err(error) => row["error"] = serde_json::json!(error),
            }
            attempts.push(row);
        }
    }
    (
        None,
        serde_json::json!({"pid":pid,"window_id":id,"error":"target_window_not_found","attempts":attempts}),
    )
}

trait ActionElement {
    fn enabled(&self) -> bool;
    fn actions(&self) -> Vec<String>;
    fn settable(&self, name: &str) -> bool;
    fn apply(&self, name: &str, value: Option<&str>) -> i32;
}
impl ActionElement for Element {
    fn enabled(&self) -> bool {
        Element::enabled(self)
    }
    fn actions(&self) -> Vec<String> {
        Element::actions(self)
    }
    fn settable(&self, name: &str) -> bool {
        Element::settable(self, name)
    }
    fn apply(&self, name: &str, value: Option<&str>) -> i32 {
        // Cua uses the retained proxy and a CFString action, with a 2 s timeout.
        // Allow a press 50 ms; text writes and preflight reads retain 5 ms.
        // Click dispatch skips text probes to stay within M3's stop budget.
        self.with_timeout(if value.is_none() { 0.05 } else { 0.005 }, || unsafe {
            let name = NSString::from_str(name);
            if let Some(value) = value {
                AXUIElementSetAttributeValue(
                    self.0,
                    string(&name),
                    string(&NSString::from_str(value)),
                )
            } else {
                AXUIElementPerformAction(self.0, string(&name))
            }
        })
    }
}

// Adapted from tools/click.rs, set_value.rs and type_text.rs. No focus/raise writes.
pub(super) fn element_action(
    element: &Element,
    action: &Action,
    reason: &mut Option<String>,
) -> Result<bool, String> {
    dispatch_element(element, action, reason)
}
fn dispatch_element(
    element: &impl ActionElement,
    action: &Action,
    reason: &mut Option<String>,
) -> Result<bool, String> {
    if !element.enabled() {
        *reason = Some("ax_element_disabled".into());
        return Ok(false);
    }
    // Do not spend mutation latency on irrelevant settable-attribute queries.
    let (actions, value, selected) = match action {
        Action::Click { .. } => (element.actions(), false, false),
        Action::Type {
            overwrite,
            enter: false,
            ..
        } => {
            let value = *overwrite && element.settable("AXValue");
            (vec![], value, !value && element.settable("AXSelectedText"))
        }
        _ => (vec![], false, false),
    };
    let Some(name) = element_route(action, &actions, value, selected) else {
        *reason = Some("ax_element_action_unavailable".into());
        return Ok(false);
    };
    let text = match action {
        Action::Type { text, .. } => Some(text.as_str()),
        _ => None,
    };
    action_result(name, element.apply(name, text), reason)
}
pub(super) fn focused_insertion(
    pid: i32,
    window: u32,
    element: &Element,
    action: &Action,
) -> Option<Element> {
    if let Action::Type {
        overwrite,
        enter: false,
        ..
    } = action
    {
        if !(*overwrite && element.settable("AXValue")) && !element.settable("AXSelectedText") {
            return target(pid, None)
                .filter(|e| e.belongs_to(pid, window) && e.settable("AXSelectedText"));
        }
    }
    None
}
fn element_route<'a>(
    action: &Action,
    actions: &'a [String],
    value: bool,
    selected: bool,
) -> Option<&'a str> {
    match action {
        Action::Click {
            button,
            clicks: 1,
            hold_keys,
            ..
        } if hold_keys.is_empty() => {
            let names: &[&str] = match button.as_str() {
                "left" => &["AXPress", "AXPick", "AXShowMenu"],
                "right" => &["AXShowMenu"],
                _ => &[],
            };
            names
                .iter()
                .find_map(|name| actions.iter().find(|a| a == name).map(String::as_str))
                // Cua attempts the requested action even when it is not advertised.
                .or_else(|| names.first().copied())
        }
        Action::Type {
            overwrite: true,
            enter: false,
            ..
        } if value => Some("AXValue"),
        Action::Type { enter: false, .. } if selected => Some("AXSelectedText"),
        _ => None,
    }
}

#[cfg(test)]
mod element_tests {
    use super::super::ax_tree::{Node, Provider, Snapshot, SNAPSHOT_BUDGET};
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn policy_metadata_includes_placeholder_and_identifier_even_with_a_benign_title() {
        for sensitive_attribute in [
            "AXDescription",
            "AXHelp",
            "AXIdentifier",
            "AXPlaceholderValue",
        ] {
            let target = field_info(|attribute| match attribute {
                "AXRole" => Some("AXTextField".into()),
                "AXTitle" => Some("Document".into()),
                a if a == sensitive_attribute => Some("PIN".into()),
                "AXValue" => panic!("must never read editable field values"),
                _ => None,
            });
            assert!(target.label.contains("Document"));
            assert!(target.label.contains("PIN"));
            assert!(!target.secure);
        }
        assert!(
            field_info(|a| match a {
                "AXRole" => Some("AXTextField".into()),
                "AXSubrole" => Some("AXSecureEntry".into()),
                _ => None,
            })
            .secure
        );
        let row = field_info(|a| match a {
            "AXRole" => Some("AXStaticText".into()),
            "AXValue" => Some("Privacy & Security".into()),
            _ => None,
        });
        assert_eq!(row.label, "Privacy & Security");
    }
    struct FakeElement {
        token: usize,
        advertised: Vec<String>,
        status: i32,
        calls: Rc<RefCell<Vec<(usize, String)>>>,
    }
    impl ActionElement for FakeElement {
        fn enabled(&self) -> bool {
            true
        }
        fn actions(&self) -> Vec<String> {
            self.advertised.clone()
        }
        fn settable(&self, _: &str) -> bool {
            panic!("click must not query text attributes")
        }
        fn apply(&self, name: &str, value: Option<&str>) -> i32 {
            assert!(value.is_none());
            self.calls.borrow_mut().push((self.token, name.into()));
            self.status
        }
    }
    struct FakeProvider;
    impl Provider for FakeProvider {
        type Handle = FakeElement;
        fn read(&self, e: &FakeElement) -> Node {
            Node {
                role: "AXButton".into(),
                label: "1".into(),
                actions: e.actions(),
                enabled: true,
                ..Node::default()
            }
        }
        fn children(&self, _: &FakeElement, _: usize) -> Vec<FakeElement> {
            vec![]
        }
    }
    fn click() -> Action {
        Action::Click {
            x: 0.,
            y: 0.,
            button: "left".into(),
            clicks: 1,
            hold_keys: vec![],
        }
    }
    #[test]
    fn presses_retained_snapshot_handle_even_without_advertised_actions() {
        for advertised in [vec!["AXPress".into()], vec![]] {
            let calls = Rc::new(RefCell::new(vec![]));
            let handle = FakeElement {
                token: 37,
                advertised,
                status: 0,
                calls: calls.clone(),
            };
            let mut snapshot = Snapshot::default();
            snapshot.replace(&FakeProvider, Some(handle), 10, SNAPSHOT_BUDGET);
            let id = snapshot.id;
            // The snapshot owns the original handle, not a reconstructed path/index.
            assert_eq!(Rc::strong_count(&calls), 2);
            let (_, retained) = snapshot.resolve(id, 0).unwrap();
            let mut reason = None;
            assert!(dispatch_element(retained, &click(), &mut reason).unwrap());
            assert_eq!(*calls.borrow(), vec![(37, "AXPress".into())]);
            assert!(reason.is_none());
            snapshot.replace(&FakeProvider, None, 10, SNAPSHOT_BUDGET);
            assert!(snapshot.resolve(id, 0).is_err());
            assert_eq!(Rc::strong_count(&calls), 1);
        }
    }
    #[test]
    fn press_errors_keep_exact_code_and_existing_fallback_policy() {
        for status in [-25204, -25205, -25206, -25201, -25202, -25211] {
            let element = FakeElement {
                token: 37,
                advertised: vec![],
                status,
                calls: Rc::default(),
            };
            let mut reason = None;
            let result = dispatch_element(&element, &click(), &mut reason);
            let expected = format!("ax_press_failed({status})");
            assert_eq!(reason, Some(expected.clone()));
            if [-25204, -25205, -25206].contains(&status) {
                assert_eq!(result, Ok(false));
            } else {
                assert_eq!(result, Err(expected));
            }
            assert_eq!(element.calls.borrow().len(), 1); // No AX mutation retry.
        }
    }
    #[test]
    fn background_window_lookup_uses_pid_window_list_and_exact_id() {
        let (window, log) = find_window(
            42,
            73,
            |attribute| {
                assert_eq!(attribute, "AXWindows"); // No frontmost/focused-window query.
                Ok(vec![8, 73, 99])
            },
            |id| Ok(*id),
        );
        assert_eq!(window, Some(73));
        assert_eq!(log["pid"], 42);
        assert_eq!(log["window_id"], 73);
        assert_eq!(log["attempts"][0]["matched_id"], 73);
    }
    #[test]
    fn window_lookup_retries_read_failure_and_records_it() {
        let mut reads = 0;
        let (window, log) = find_window(
            42,
            73,
            |attribute| {
                reads += 1;
                match (reads, attribute) {
                    (1, "AXWindows") => Err("AXWindows: ax_error(-25204)".into()),
                    (2, "AXChildren") => Ok(vec![]),
                    (3, "AXWindows") => Ok(vec![73]),
                    _ => panic!("unexpected lookup"),
                }
            },
            |id| Ok(*id),
        );
        assert_eq!(window, Some(73));
        assert_eq!(reads, 3);
        assert_eq!(log["attempts"][0]["error"], "AXWindows: ax_error(-25204)");
    }
    #[test]
    fn window_lookup_uses_children_union_but_never_substitutes_wrong_id() {
        let (window, _) = find_window(
            42,
            73,
            |attribute| {
                Ok(if attribute == "AXChildren" {
                    vec![73]
                } else {
                    vec![8]
                })
            },
            |id| Ok(*id),
        );
        assert_eq!(window, Some(73));
        let (window, log) = find_window(
            42,
            73,
            |_| Ok(vec![8, 0]),
            |id| {
                if *id == 0 {
                    Err("_AXUIElementGetWindow: ax_error(-25202), id=0".into())
                } else {
                    Ok(*id)
                }
            },
        );
        assert_eq!(window, None);
        assert_eq!(log["error"], "target_window_not_found");
        assert_eq!(log["attempts"].as_array().unwrap().len(), 4);
        assert_eq!(log["attempts"][0]["candidates"][0]["id"], 8);
        assert_eq!(
            log["attempts"][0]["candidates"][1]["error"],
            "_AXUIElementGetWindow: ax_error(-25202), id=0"
        );
    }
    #[test]
    fn routes_press_value_selected_text_and_fallback() {
        let mut action = Action::Click {
            x: 0.,
            y: 0.,
            button: "left".into(),
            clicks: 1,
            hold_keys: vec![],
        };
        assert_eq!(
            element_route(&action, &["AXPress".into()], false, false),
            Some("AXPress")
        );
        assert_eq!(
            element_route(&action, &["AXPick".into()], false, false),
            Some("AXPick")
        );
        assert_eq!(element_route(&action, &[], false, false), Some("AXPress"));
        action = Action::Type {
            text: "hi".into(),
            x: None,
            y: None,
            overwrite: true,
            enter: false,
        };
        assert_eq!(element_route(&action, &[], true, true), Some("AXValue"));
        assert_eq!(
            element_route(&action, &[], false, true),
            Some("AXSelectedText")
        );
        assert_eq!(element_route(&action, &[], false, false), None);
        if let Action::Type { overwrite, .. } = &mut action {
            *overwrite = false;
        }
        assert_eq!(
            element_route(&action, &[], true, true),
            Some("AXSelectedText")
        );
        if let Action::Type { enter, .. } = &mut action {
            *enter = true;
        }
        assert_eq!(element_route(&action, &[], true, true), None); // Never replay text to add Return.
    }
}
