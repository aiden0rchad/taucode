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
        struct Reset<'a>(&'a Element);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                unsafe {
                    AXUIElementSetMessagingTimeout(self.0 .0, 0.005);
                }
            }
        }
        unsafe {
            AXUIElementSetMessagingTimeout(self.0, 0.05);
        }
        let _reset = Reset(self);
        read()
    }
    pub(super) fn app(pid: i32) -> Option<Self> {
        Self::owned(unsafe { AXUIElementCreateApplication(pid) })
    }
    pub(super) fn attr(&self, name: &str) -> Option<Self> {
        let mut result = std::ptr::null();
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.0, string(&NSString::from_str(name)), &mut result)
        };
        if status == 0 {
            Self::owned_value(result)
        } else {
            None
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
        let mut id = 0;
        (unsafe { _AXUIElementGetWindow(self.0, &mut id) } == 0 && id != 0).then_some(id)
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
        if limit == 0 {
            return vec![];
        }
        let Some(array) = self.attr(name) else {
            return vec![];
        };
        unsafe {
            if CFGetTypeID(array.0) != CFArrayGetTypeID() {
                return vec![];
            }
            (0..CFArrayGetCount(array.0).min(limit as isize))
                .filter_map(|i| Self::owned(CFRetain(CFArrayGetValueAtIndex(array.0, i))))
                .collect()
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
    pub(super) fn perform(&self, action: &str) -> Result<bool, String> {
        action_status(unsafe {
            AXUIElementPerformAction(self.0, string(&NSString::from_str(action)))
        })
    }
    pub(super) fn set_text(&self, attribute: &str, text: &str) -> Result<bool, String> {
        action_status(unsafe {
            AXUIElementSetAttributeValue(
                self.0,
                string(&NSString::from_str(attribute)),
                string(&NSString::from_str(text)),
            )
        })
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
    target(pid, None)
        .and_then(|e| e.attr("AXSubrole"))
        .is_none_or(|s| s.equals("AXSecureTextField"))
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

pub(super) fn window_element(pid: i32, id: u32) -> Option<Element> {
    let deadline = std::time::Instant::now() + super::ax_tree::SNAPSHOT_BUDGET;
    let app = Element::app(pid)?;
    app.reading(|| app.children("AXWindows", 128))
        .into_iter()
        .take_while(|_| std::time::Instant::now() < deadline)
        .find(|w| w.reading(|| w.window_id()) == Some(id))
}

// Adapted from tools/click.rs, set_value.rs and type_text.rs. No focus/raise writes.
pub(super) fn element_action(element: &Element, action: &Action) -> Result<bool, String> {
    if !element.enabled() {
        return Ok(false);
    }
    match element_route(
        action,
        &element.actions(),
        element.settable("AXValue"),
        element.settable("AXSelectedText"),
    ) {
        Some("AXValue") => {
            if let Action::Type { text, .. } = action {
                element.set_text("AXValue", text)
            } else {
                Ok(false)
            }
        }
        Some("AXSelectedText") => {
            if let Action::Type { text, .. } = action {
                element.set_text("AXSelectedText", text)
            } else {
                Ok(false)
            }
        }
        Some(name) => element.perform(name),
        None => Ok(false),
    }
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
    use super::*;
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
        assert_eq!(element_route(&action, &[], false, false), None);
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
