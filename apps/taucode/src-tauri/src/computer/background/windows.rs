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
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> *mut c_void;
}

// CGWindowList is front to back; SCShareableContent does not promise ordering.
pub fn order(pid: i32) -> Vec<u32> {
    unsafe {
        let Some(list) =
            Retained::<NSArray<AnyObject>>::from_raw(CGWindowListCopyWindowInfo(1 | 16, 0).cast())
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
    }
}
pub fn select(content: &SCShareableContent, pid: i32) -> Result<SCWindow, String> {
    let windows = content.windows();
    order(pid)
        .iter()
        .find_map(|id| {
            windows
                .iter()
                .find(|w| {
                    w.window_id() == *id
                        && w.is_on_screen()
                        && w.window_layer() == 0
                        && w.owning_application()
                            .is_some_and(|a| a.process_id() == pid)
                })
                .cloned()
        })
        .ok_or("Target has no on-screen window".into())
}
pub fn validate(expected: &Window, pid: i32) -> Result<(), String> {
    unsafe {
        let list = Retained::<NSArray<AnyObject>>::from_raw(
            CGWindowListCopyWindowInfo(8 | 16, expected.id).cast(),
        )
        .ok_or("Target window unavailable")?;
        for dict in list.iter() {
            let get = |object: *const AnyObject, key: &str| -> *mut AnyObject {
                msg_send![object, objectForKey:&*NSString::from_str(key)]
            };
            let owner = get(&*dict, "kCGWindowOwnerPID");
            let bounds = get(&*dict, "kCGWindowBounds");
            let onscreen = get(&*dict, "kCGWindowIsOnscreen");
            if owner.is_null() || bounds.is_null() || onscreen.is_null() {
                continue;
            }
            let owner: i32 = msg_send![owner, intValue];
            let onscreen: bool = msg_send![onscreen, boolValue];
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
                && onscreen
                && values.as_deref()
                    == Some(&[
                        expected.origin[0],
                        expected.origin[1],
                        expected.size[0],
                        expected.size[1],
                    ])
            {
                return Ok(());
            }
        }
    }
    Err("Target window moved, closed, or changed; action cancelled".into())
}
