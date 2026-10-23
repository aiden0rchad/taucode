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
    fn CFRelease(value: Ref);
    fn CFArrayGetCount(value: Ref) -> isize;
    fn CFArrayGetValueAtIndex(value: Ref, index: isize) -> Ref;
}
pub struct Element(Ref);
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
    fn app(pid: i32) -> Option<Self> {
        Self::owned(unsafe { AXUIElementCreateApplication(pid) })
    }
    fn attr(&self, name: &str) -> Option<Self> {
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
    fn settable(&self, name: &str) -> bool {
        let mut value = 0;
        unsafe {
            AXUIElementIsAttributeSettable(self.0, string(&NSString::from_str(name)), &mut value)
                == 0
                && value != 0
        }
    }
    fn window_id(&self) -> Option<u32> {
        let mut id = 0;
        (unsafe { _AXUIElementGetWindow(self.0, &mut id) } == 0 && id != 0).then_some(id)
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
        Action::Click { .. } => {
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
