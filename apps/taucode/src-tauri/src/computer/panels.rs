// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Panel configuration adapted from cursor-overlay; see background/NOTICE.
//! Run UI never participates in application or keyboard activation.
use objc2::{
    class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use objc2_foundation::NSRect;
use std::{cell::RefCell, sync::OnceLock};

pub const NONACTIVATING: u64 = 1 << 7;
pub const COLLECTION: u64 = 1 | (1 << 8) | (1 << 4) | (1 << 6);
pub const FOCUS: bool = false;
pub const FOCUSABLE: bool = false;
pub const INITIALLY_VISIBLE: bool = false;
pub const KEY_ONLY_IF_NEEDED: bool = true;
pub const HIDES_ON_DEACTIVATE: bool = false;
pub const LEVEL: i64 = 25;

pub fn panel_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut class = ClassBuilder::new(c"TaucodeComputerPanel", class!(NSPanel)).unwrap();
        extern "C-unwind" fn can_key(_: &AnyObject, _: Sel) -> Bool {
            Bool::new(FOCUSABLE)
        }
        unsafe {
            class.add_method(
                sel!(canBecomeKeyWindow),
                can_key as extern "C-unwind" fn(_, _) -> _,
            );
            class.add_method(
                sel!(canBecomeMainWindow),
                can_key as extern "C-unwind" fn(_, _) -> _,
            );
        }
        class.register()
    })
}
pub unsafe fn configure(panel: &AnyObject) {
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded:KEY_ONLY_IF_NEEDED];
    let _: () = msg_send![panel, setHidesOnDeactivate:HIDES_ON_DEACTIVATE];
    let _: () = msg_send![panel, setCollectionBehavior:COLLECTION];
    let _: () = msg_send![panel, setLevel:LEVEL];
    let _: () = msg_send![panel, setReleasedWhenClosed:false];
    let _: () = msg_send![panel, setSharingType:0u64];
}
thread_local! { static HUD: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) }; }

pub async fn show_hud(hud: tauri::WebviewWindow) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let window = hud.clone();
    hud.run_on_main_thread(move || {
        let result = HUD.with(|slot| -> Result<(), String> {
            let mut panel = slot.borrow_mut();
            unsafe {
                if panel.is_none() {
                    let native = window.ns_window().map_err(|e| e.to_string())? as *mut AnyObject;
                    let frame: NSRect = msg_send![native, frame];
                    let allocated: *mut AnyObject = msg_send![panel_class(), alloc];
                    let raw: *mut AnyObject = msg_send![allocated, initWithContentRect:frame, styleMask:NONACTIVATING, backing:2u64, defer:false];
                    let new = Retained::from_raw(raw).ok_or("Cannot create computer HUD panel")?;
                    configure(&new);
                    // TaoWindow has a private ivar. An isa swap to NSPanel would
                    // violate objc2's layout rules. Convert the native host by
                    // moving its retained webview into a real NSPanel instead.
                    let content: *mut AnyObject = msg_send![native, contentView];
                    let content = Retained::retain(content).ok_or("HUD has no content view")?;
                    let _: () = msg_send![native, setContentView:std::ptr::null::<AnyObject>()];
                    let _: () = msg_send![&*new, setContentView:&*content];
                    *panel = Some(new);
                }
                let _: () = msg_send![panel.as_ref().unwrap(), orderFrontRegardless];
            }
            Ok(())
        });
        let _ = tx.send(result);
    }).map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "HUD main-thread task cancelled".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_and_cursor_configuration_cannot_activate_or_hide_on_app_switch() {
        assert!(!FOCUS && !FOCUSABLE && !INITIALLY_VISIBLE);
        assert_eq!(NONACTIVATING, 128);
        assert_eq!(COLLECTION, 1 | 256 | 16 | 64);
        assert!(KEY_ONLY_IF_NEEDED);
        assert!(!HIDES_ON_DEACTIVATE);
        assert_eq!(LEVEL, 25);
        // No actual AppKit windows are created by this test.
        let source = include_str!("panels.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(source.contains("orderFrontRegardless"));
        assert!(!source.contains("makeKeyAndOrderFront") && !source.contains("set_focus"));
        let transport = include_str!("transport.rs");
        assert!(!transport.contains("set_focus") && !transport.contains("get_webview_window"));
    }
}
