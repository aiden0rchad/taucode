// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Panel configuration adapted from cursor-overlay; see background/NOTICE.
//! Run UI never participates in application or keyboard activation.
use super::Computer;
use objc2::{
    class, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel, DefinedClass, MainThreadOnly,
};
use objc2_foundation::{
    MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use serde_json::Value;
use std::{
    cell::RefCell,
    sync::{atomic::Ordering, Arc, OnceLock, Weak},
};

pub const NONACTIVATING: u64 = 1 << 7;
pub const COLLECTION: u64 = 1 | (1 << 8) | (1 << 4) | (1 << 6);
pub const FOCUSABLE: bool = false;
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
enum Action {
    PauseResume,
    Stop,
}
fn dispatch(computer: &Computer, action: Action) {
    match action {
        Action::PauseResume => computer.pause(!computer.control.paused.load(Ordering::SeqCst)),
        Action::Stop => computer.stop(),
    }
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Weak<Computer>]
    struct HudTarget;

    unsafe impl NSObjectProtocol for HudTarget {}
    impl HudTarget {
        #[unsafe(method(pauseResume:))]
        fn pause_resume(&self, _sender: &AnyObject) {
            if let Some(computer) = self.ivars().upgrade() { dispatch(&computer, Action::PauseResume); }
        }
        #[unsafe(method(stop:))]
        fn stop(&self, _sender: &AnyObject) {
            if let Some(computer) = self.ivars().upgrade() { dispatch(&computer, Action::Stop); }
        }
    }
);

struct Hud {
    panel: Retained<AnyObject>,
    text: Retained<AnyObject>,
    pause: Retained<AnyObject>,
    // NSControl does not retain its action target.
    _target: Retained<HudTarget>,
}
thread_local! { static HUD: RefCell<Option<Hud>> = const { RefCell::new(None) }; }

fn step_text(state: &Value) -> String {
    if state["running"] != true {
        return "Computer Use — Stopped".into();
    }
    let text = if let Some(reason) = state["approval"]["reason"].as_str() {
        format!("Approval needed in taucode: {reason}")
    } else if state["paused"] == true {
        "Paused".into()
    } else if let Some(step) = state["steps"].as_array().and_then(|s| s.last()) {
        format!(
            "Step {}: {}",
            step["step"],
            step["target"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| step["action"]["kind"].as_str())
                .unwrap_or("Working")
        )
    } else {
        format!(
            "Starting: {}",
            state["instruction"].as_str().unwrap_or("Computer Use")
        )
    };
    format!(
        "Computer Use — {}",
        text.chars().take(400).collect::<String>()
    )
}

impl Hud {
    // Only called on the main thread. All views belong to this newly allocated panel.
    unsafe fn new(computer: Weak<Computer>) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("HUD requires the main thread")?;
        let allocated = HudTarget::alloc(mtm).set_ivars(computer);
        let target: Retained<HudTarget> = msg_send![super(allocated), init];
        let allocated: *mut AnyObject = msg_send![panel_class(), alloc];
        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(440., 136.));
        let raw: *mut AnyObject = msg_send![allocated, initWithContentRect:frame, styleMask:NONACTIVATING, backing:2u64, defer:false];
        let panel = Retained::from_raw(raw).ok_or("Cannot create computer HUD panel")?;
        configure(&panel);
        let _: () = msg_send![&*panel, setTitle:&*NSString::from_str("Computer Use")];
        let _: () = msg_send![&*panel, setMovableByWindowBackground:true];
        let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        if !screen.is_null() {
            let visible: NSRect = msg_send![screen, visibleFrame];
            let origin = NSPoint::new(
                visible.origin.x + visible.size.width - 464.,
                visible.origin.y + visible.size.height - 160.,
            );
            let _: () = msg_send![&*panel, setFrameOrigin:origin];
        }
        let content: *mut AnyObject = msg_send![&*panel, contentView];
        let text: Retained<AnyObject> =
            msg_send![class!(NSTextField), labelWithString:&*NSString::from_str("Computer Use")];
        let _: () =
            msg_send![&*text, setFrame:NSRect::new(NSPoint::new(16., 62.), NSSize::new(408., 58.))];
        let _: () = msg_send![&*text, setMaximumNumberOfLines:3isize];
        let _: () = msg_send![&*text, setLineBreakMode:0u64];
        let _: () = msg_send![content, addSubview:&*text];
        let pause: Retained<AnyObject> = msg_send![class!(NSButton), buttonWithTitle:&*NSString::from_str("Pause"), target:&*target, action:sel!(pauseResume:)];
        let stop: Retained<AnyObject> = msg_send![class!(NSButton), buttonWithTitle:&*NSString::from_str("Stop"), target:&*target, action:sel!(stop:)];
        for (button, x) in [(&pause, 16.), (&stop, 324.)] {
            let _: () = msg_send![&**button, setFrame:NSRect::new(NSPoint::new(x, 16.), NSSize::new(100., 32.))];
            let _: () = msg_send![&**button, setBezelStyle:1u64];
            let _: () = msg_send![&**button, setRefusesFirstResponder:true];
            let _: () = msg_send![content, addSubview:&**button];
        }
        Ok(Self {
            panel,
            text,
            pause,
            _target: target,
        })
    }
    unsafe fn update(&self, state: &Value) {
        let _: () = msg_send![&*self.text, setStringValue:&*NSString::from_str(&step_text(state))];
        let title = if state["paused"] == true {
            "Resume"
        } else {
            "Pause"
        };
        let _: () = msg_send![&*self.pause, setTitle:&*NSString::from_str(title)];
        if state["running"] == true {
            let _: () = msg_send![&*self.panel, orderFrontRegardless];
        } else {
            let _: () = msg_send![&*self.panel, orderOut:std::ptr::null::<AnyObject>()];
        }
    }
}

pub async fn show_hud(app: tauri::AppHandle, computer: &Arc<Computer>) -> Result<(), String> {
    let mut state = computer.state.subscribe();
    let run_id = state.borrow()["id"].clone();
    let weak = Arc::downgrade(computer);
    let owner = weak.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let result = HUD.with(|slot| -> Result<(), String> {
            let mut hud = slot.borrow_mut();
            unsafe {
                if hud.is_none() {
                    *hud = Some(Hud::new(owner.clone())?);
                }
                if let Some(computer) = owner.upgrade() {
                    hud.as_ref().unwrap().update(&computer.state.borrow());
                }
            }
            Ok(())
        });
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    // Subscribe before awaiting creation: cancellation during startup must still
    // deliver the stopped state and hide a panel already queued on the main thread.
    tokio::spawn(async move {
        while state.changed().await.is_ok() {
            let finished = {
                let current = state.borrow_and_update();
                current["running"] != true || current["id"] != run_id
            };
            let owner = weak.clone();
            if app
                .run_on_main_thread(move || {
                    // Read the latest state here, so a queued old-run update cannot
                    // hide or overwrite a newer run's HUD.
                    if let Some(computer) = owner.upgrade() {
                        HUD.with(|slot| {
                            if let Some(hud) = slot.borrow().as_ref() {
                                unsafe {
                                    hud.update(&computer.state.borrow());
                                }
                            }
                        });
                    }
                })
                .is_err()
                || finished
            {
                break;
            }
        }
    });
    rx.await
        .map_err(|_| "HUD main-thread task cancelled".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_and_cursor_configuration_cannot_activate_or_hide_on_app_switch() {
        assert!(!FOCUSABLE);
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
        for native in [source, include_str!("background/overlay.rs")] {
            for forbidden in [
                "WebviewWindow",
                "tao::",
                "ns_window",
                "setContentView",
                "get_webview_window",
                "makeKeyAndOrderFront",
                "set_focus",
                "unminimize",
            ] {
                assert!(
                    !native.contains(forbidden),
                    "native panel references {forbidden}"
                );
            }
        }
        for path in [source, include_str!("mod.rs"), include_str!("transport.rs")] {
            for forbidden in [
                "WebviewWindowBuilder",
                "get_webview_window",
                "set_focus(",
                "unminimize(",
            ] {
                assert!(
                    !path.contains(forbidden),
                    "computer run references {forbidden}"
                );
            }
        }
        assert!(!include_str!("../../../src/App.tsx").contains("ComputerHud"));
    }

    #[test]
    fn native_buttons_use_control_pause_resume_and_stop_paths() {
        use super::super::control::{Event, Input};
        use super::super::policy::Context;
        use std::sync::Mutex;
        struct FakeInput(Arc<Mutex<Vec<Event>>>);
        impl Input for FakeInput {
            fn post(&mut self, event: &Event) -> Result<(), String> {
                self.0.lock().unwrap().push(event.clone());
                Ok(())
            }
        }
        let computer = Computer::new(std::path::PathBuf::new());
        let events = Arc::new(Mutex::new(vec![]));
        computer.control.replace_input(FakeInput(events.clone()));
        computer.control.start();
        let context = Context {
            app: "target".into(),
            secure: false,
        };
        computer
            .control
            .post(Event::Key(12, true), &context, || Ok(context.clone()))
            .unwrap();
        dispatch(&computer, Action::PauseResume);
        assert!(computer.control.paused.load(Ordering::SeqCst));
        assert_eq!(computer.state.borrow()["paused"], true);
        assert_eq!(
            *events.lock().unwrap(),
            [Event::Key(12, true), Event::Key(12, false)]
        );
        dispatch(&computer, Action::PauseResume);
        assert!(!computer.control.paused.load(Ordering::SeqCst));
        assert_eq!(computer.state.borrow()["paused"], false);
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *computer.pending.lock().unwrap() = Some(("approval".into(), tx));
        dispatch(&computer, Action::Stop);
        assert!(computer.control.stopped());
        assert!(computer.pending.lock().unwrap().is_none());
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ));
        assert!(computer
            .control
            .post(Event::Key(12, true), &context, || Ok(context.clone()))
            .is_err());
    }

    #[test]
    fn current_step_text_tracks_run_approval_pause_and_completion() {
        use serde_json::json;
        assert_eq!(
            step_text(&json!({"running":false})),
            "Computer Use — Stopped"
        );
        assert_eq!(
            step_text(&json!({"running":true,"instruction":"Calculate"})),
            "Computer Use — Starting: Calculate"
        );
        assert_eq!(
            step_text(&json!({"running":true,"paused":true})),
            "Computer Use — Paused"
        );
        assert_eq!(
            step_text(&json!({"running":true,"approval":{"reason":"Secure field"}})),
            "Computer Use — Approval needed in taucode: Secure field"
        );
        assert_eq!(
            step_text(
                &json!({"running":true,"steps":[{"step":2,"target":"Clear","action":{"kind":"click"}}]})
            ),
            "Computer Use — Step 2: Clear"
        );
    }
}
