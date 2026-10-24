// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from platform-macos/src/windows.rs; see NOTICE.
use objc2::{msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSString};
use screencapturekit::prelude::*;
use serde::Serialize;
use std::ffi::c_void;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Window {
    pub id: u32,
    pub title: String,
    pub origin: [f64; 2],
    pub size: [f64; 2],
    pub on_screen: bool,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> *mut c_void;
}

// CGWindowList is front to back; SCShareableContent does not promise ordering.
pub fn order(pid: i32) -> Vec<u32> {
    ordered(pid, 1 | 16)
}
fn ordered(pid: i32, options: u32) -> Vec<u32> {
    unsafe {
        let Some(list) =
            Retained::<NSArray<AnyObject>>::from_raw(CGWindowListCopyWindowInfo(options, 0).cast())
        else {
            return vec![];
        };
        list.iter()
            .filter_map(|dict| {
                let number = |key: &str| -> i64 {
                    let value: *mut AnyObject =
                        msg_send![&*dict, objectForKey: &*NSString::from_str(key)];
                    if value.is_null() {
                        0
                    } else {
                        msg_send![value, longLongValue]
                    }
                };
                (number("kCGWindowOwnerPID") == pid as i64 && number("kCGWindowLayer") == 0)
                    .then(|| number("kCGWindowNumber") as u32)
            })
            .collect()
    }
}
pub fn describe(w: &SCWindow) -> Window {
    let r = w.frame();
    Window {
        id: w.window_id(),
        title: w.title().unwrap_or_default(),
        origin: [r.origin.x, r.origin.y],
        size: [r.size.width, r.size.height],
        on_screen: w.is_on_screen(),
    }
}
pub fn select(content: &SCShareableContent, pid: i32) -> Result<SCWindow, String> {
    let mut windows: Vec<_> = content
        .windows()
        .into_iter()
        .filter(|w| {
            w.window_layer() == 0
                && w.owning_application()
                    .is_some_and(|a| a.process_id() == pid)
        })
        .collect();
    let order = ordered(pid, 16); // Include windows on other Spaces.
    windows.sort_by_key(|w| {
        order
            .iter()
            .position(|id| *id == w.window_id())
            .unwrap_or(usize::MAX)
    });
    let visible: Vec<_> = windows.iter().map(SCWindow::is_on_screen).collect();
    let index = select_index(&visible, |i| {
        super::ax_actions::minimized(pid, windows[i].window_id())
    })
    .map_err(|e| format!("Window selection: pid={pid}, {e}"))?;
    Ok(windows[index].clone())
}
// Input is front to back. On-screen windows cannot be minimized; query AX only
// for the fallback candidates, failing closed if their state cannot be read.
fn select_index(
    on_screen: &[bool],
    mut minimized: impl FnMut(usize) -> Result<bool, String>,
) -> Result<usize, String> {
    if let Some(index) = on_screen.iter().position(|visible| *visible) {
        return Ok(index);
    }
    let mut minimized_count = 0;
    let mut failures = vec![];
    for index in 0..on_screen.len() {
        match minimized(index) {
            Ok(false) => return Ok(index),
            Ok(true) => minimized_count += 1,
            Err(error) => failures.push(error),
        }
    }
    let reason = if minimized_count > 0 && minimized_count == on_screen.len() {
        "Target window is minimized"
    } else {
        "Target has no window"
    };
    Err(format!(
        "{reason}; listed={}, on-screen=0, minimized={minimized_count}, unknown={}; {}",
        on_screen.len(),
        failures.len(),
        failures.join("; ")
    ))
}
pub fn validate(expected: &Window, pid: i32) -> Result<bool, String> {
    unsafe {
        let list = Retained::<NSArray<AnyObject>>::from_raw(
            CGWindowListCopyWindowInfo(8 | 16, expected.id).cast(),
        )
        .ok_or_else(|| {
            format!(
                "Window validation: pid={pid}, window={}, CGWindowList=null",
                expected.id
            )
        })?;
        for dict in list.iter() {
            let get = |object: *const AnyObject, key: &str| -> *mut AnyObject {
                msg_send![object, objectForKey:&*NSString::from_str(key)]
            };
            let owner = get(&*dict, "kCGWindowOwnerPID");
            let bounds = get(&*dict, "kCGWindowBounds");
            let onscreen = get(&*dict, "kCGWindowIsOnscreen");
            if owner.is_null() || bounds.is_null() {
                continue;
            }
            let owner: i32 = msg_send![owner, intValue];
            let onscreen = !onscreen.is_null() && msg_send![onscreen, boolValue];
            let values: Option<Vec<f64>> = ["X", "Y", "Width", "Height"]
                .iter()
                .map(|key| {
                    let value = get(bounds, key);
                    if value.is_null() {
                        None
                    } else {
                        Some(msg_send![value, doubleValue])
                    }
                })
                .collect();
            if owner == pid
                && values.as_deref()
                    == Some(&[
                        expected.origin[0],
                        expected.origin[1],
                        expected.size[0],
                        expected.size[1],
                    ])
            {
                if !onscreen && super::ax_actions::minimized(pid, expected.id)? {
                    return Err(format!(
                        "Target window is minimized; pid={pid}, window={}",
                        expected.id
                    ));
                }
                return Ok(onscreen);
            }
            return Err(format!("Window validation: window={}, owner before={pid} after={owner}, bounds before={:?} after={values:?}, on-screen={onscreen}; action cancelled",
                expected.id, [expected.origin[0], expected.origin[1], expected.size[0], expected.size[1]]));
        }
    }
    Err(format!(
        "Window validation: pid={pid}, window={}, complete matching entries=0; action cancelled",
        expected.id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_prefers_visible_then_non_minimized_in_front_to_back_order() {
        assert_eq!(
            select_index(&[false, true, true], |_| panic!(
                "visible needs no AX query"
            )),
            Ok(1)
        );
        assert_eq!(select_index(&[false, false, false], |i| Ok(i == 0)), Ok(1));
        assert!(select_index(&[false, false], |_| Ok(true))
            .unwrap_err()
            .contains("Target window is minimized; listed=2"));
        assert!(select_index(&[], |_| unreachable!())
            .unwrap_err()
            .contains("Target has no window; listed=0"));
        let error =
            select_index(&[false], |_| Err("AXMinimized: ax_error(-25204)".into())).unwrap_err();
        assert!(error.contains("unknown=1"));
        assert!(error.contains("ax_error(-25204)"));
    }
}
