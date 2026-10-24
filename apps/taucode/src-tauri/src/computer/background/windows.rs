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
// Retry missing usable windows; reopen once only when AXWindows is readable and empty.
// Minimized windows remain failures. Injected time keeps launch tests deterministic.
pub fn retry_windowless<T>(
    mut select: impl FnMut() -> Result<T, String>,
    mut reopen: impl FnMut() -> Result<(), String>,
    mut now: impl FnMut() -> std::time::Instant,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<T, String> {
    let deadline = now() + std::time::Duration::from_secs(5);
    let mut reopened = false;
    loop {
        let result = select();
        if !result
            .as_ref()
            .err()
            .is_some_and(|e| e.contains("Target has no window;"))
            || now() >= deadline
        {
            return result;
        }
        if !reopened {
            reopen()?;
            reopened = true;
        }
        wait(std::time::Duration::from_millis(200).min(deadline.saturating_duration_since(now())));
    }
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
            let id = get(&*dict, "kCGWindowNumber");
            let bounds = get(&*dict, "kCGWindowBounds");
            let onscreen = get(&*dict, "kCGWindowIsOnscreen");
            if owner.is_null() || bounds.is_null() || id.is_null() {
                continue;
            }
            let owner: i32 = msg_send![owner, intValue];
            let id: u32 = msg_send![id, unsignedIntValue];
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
            validate_observation(expected, pid, id, owner, values.as_deref(), onscreen)?;
            if !onscreen && super::ax_actions::minimized(pid, expected.id)? {
                return Err(format!(
                    "Target window is minimized; pid={pid}, window={}",
                    expected.id
                ));
            }
            return Ok(onscreen);
        }
    }
    Err(format!(
        "Window validation: pid={pid}, window={}, complete matching entries=0; action cancelled",
        expected.id
    ))
}

fn validate_observation(
    expected: &Window,
    pid: i32,
    id: u32,
    owner: i32,
    values: Option<&[f64]>,
    on_screen: bool,
) -> Result<(), String> {
    let before = [
        expected.origin[0],
        expected.origin[1],
        expected.size[0],
        expected.size[1],
    ];
    let valid_bounds = values
        .is_some_and(|v| v.len() == 4 && v.iter().all(|n| n.is_finite()) && v[2] > 0. && v[3] > 0.);
    let unchanged = values.is_some_and(|v| {
        if !valid_bounds {
            return false;
        }
        let edges = |v: &[f64]| [v[0], v[1], v[0] + v[2], v[1] + v[3]];
        v == before
            || (on_screen
                && edges(v)
                    .iter()
                    .zip(edges(&before))
                    .all(|(a, b)| (a - b).abs() <= 4.))
    });
    if owner == pid && id == expected.id && unchanged {
        return Ok(());
    }
    let diagnostic = format!("Window validation: window={} after={id}, owner before={pid} after={owner}, bounds before={before:?} after={values:?}, on-screen={on_screen}; action cancelled", expected.id);
    // Missing/invalid geometry and ownership changes are never retryable.
    Err(if owner == pid && valid_bounds && on_screen {
        format!("{}{diagnostic}", super::super::runner::STALE_OBSERVATION)
    } else {
        diagnostic
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_tolerates_four_points_per_edge_and_rejects_identity_changes() {
        let window = Window {
            id: 7,
            title: String::new(),
            origin: [10., 20.],
            size: [100., 80.],
            on_screen: true,
        };
        for bounds in [
            [10., 20., 100., 80.],
            [14., 16., 100., 80.],
            [6., 24., 108., 72.],
        ] {
            assert_eq!(
                validate_observation(&window, 42, 7, 42, Some(&bounds), true),
                Ok(())
            );
        }
        for (id, bounds) in [
            (7, [14.01, 20., 100., 80.]),
            (7, [14., 20., 101., 80.]),
            (8, [10., 20., 100., 80.]),
        ] {
            assert!(
                validate_observation(&window, 42, id, 42, Some(&bounds), true)
                    .unwrap_err()
                    .starts_with(super::super::super::runner::STALE_OBSERVATION)
            );
        }
        for (owner, bounds, visible) in [
            (43, Some([10., 20., 100., 80.]), true),
            (42, None, true),
            (42, Some([f64::NAN, 20., 100., 80.]), true),
            (42, Some([14., 20., 100., 80.]), false),
        ] {
            assert!(!validate_observation(
                &window,
                42,
                7,
                owner,
                bounds.as_ref().map(|v| v.as_slice()),
                visible
            )
            .unwrap_err()
            .starts_with(super::super::super::runner::STALE_OBSERVATION));
        }
    }

    #[test]
    fn windowless_launch_reopens_once_polls_and_stops_at_five_seconds() {
        use std::{
            cell::Cell,
            time::{Duration, Instant},
        };
        for arrives in [Some(3), None] {
            let clock = Cell::new(Instant::now());
            let start = clock.get();
            let calls = Cell::new(0);
            let reopens = Cell::new(0);
            let result = retry_windowless(
                || {
                    calls.set(calls.get() + 1);
                    if arrives == Some(calls.get()) {
                        Ok(7)
                    } else {
                        Err(
                            "Target has no window; listed=14, on-screen=0, minimized=0, unknown=14"
                                .into(),
                        )
                    }
                },
                || {
                    reopens.set(reopens.get() + 1);
                    Ok(())
                },
                || clock.get(),
                |duration| {
                    assert!(duration <= Duration::from_millis(200));
                    clock.set(clock.get() + duration);
                },
            );
            assert_eq!(reopens.get(), 1);
            if arrives.is_some() {
                assert_eq!(result, Ok(7));
                assert_eq!(calls.get(), 3);
            } else {
                assert!(result.is_err());
                assert_eq!(calls.get(), 26);
                assert_eq!(clock.get() - start, Duration::from_secs(5));
            }
        }
        for error in [
            "Target window is minimized; listed=1",
            "Target process exited",
            "SCK window list unavailable",
        ] {
            let result: Result<(), _> = retry_windowless(
                || Err(error.into()),
                || panic!("must not reopen"),
                Instant::now,
                |_| panic!("must not retry"),
            );
            assert_eq!(result.unwrap_err(), error);
        }
    }

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
