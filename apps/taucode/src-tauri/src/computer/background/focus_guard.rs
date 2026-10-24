// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from focus_guard.rs, focus_steal.rs and key_focus_guard.rs; see NOTICE.
use super::{ax_actions, skylight, Session};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock, Weak,
    },
    time::{Duration, Instant},
};

pub struct State {
    pub previous: i32,
    pub window: Option<u32>,
    pub target: i32,
    pub host: i32,
    pub armed_until: Instant,
}
impl State {
    // Respect real user app switches; suppress target activation or action-caused helper activation.
    pub fn observe(&mut self, front: i32, user_switched: bool, now: Instant) -> Option<i32> {
        if front == self.previous {
            return None;
        }
        if front == self.host
            || (!user_switched && (front == self.target || now < self.armed_until))
        {
            Some(self.previous)
        } else {
            self.previous = front;
            self.window = None;
            None
        }
    }
}
pub struct Guard {
    state: Mutex<State>,
    pub delivering: AtomicBool,
    pub restorations: Mutex<Vec<Value>>,
}
pub struct UserFocus {
    pid: Option<i32>,
    window: Option<u32>,
}
impl UserFocus {
    // Capture before any run UI is created or shown.
    pub fn capture() -> Self {
        let pid = front_pid();
        Self {
            pid,
            window: pid.and_then(skylight::key_window_of_pid),
        }
    }
}
impl Guard {
    pub fn new(target: i32, prior: UserFocus) -> Self {
        let previous = prior.pid.unwrap_or(target);
        Self {
            state: Mutex::new(State {
                previous,
                target,
                host: std::process::id() as i32,
                window: prior.window,
                armed_until: Instant::now(),
            }),
            restorations: Mutex::new(vec![]),
            delivering: AtomicBool::new(false),
        }
    }
    pub fn arm(&self) {
        self.state.lock().unwrap().armed_until = Instant::now() + Duration::from_millis(250);
    }
    pub fn start(&self, session: Weak<Session>) {
        self.tick();
        install_observer(session.clone());
        std::thread::spawn(move || loop {
            let Some(session) = session.upgrade() else {
                break;
            };
            if !session.active.load(Ordering::SeqCst) {
                break;
            }
            if !session.foreground.load(Ordering::SeqCst) {
                session.guard.tick();
            }
            drop(session);
            std::thread::sleep(Duration::from_millis(10));
        });
    }
    fn tick(&self) {
        let Some(front) = front_pid() else {
            return;
        };
        let mut s = self.state.lock().unwrap();
        // The no-raise prologue must settle before posting. It may temporarily
        // report the target active; the scope restores it after queued input.
        // Never exempt taucode, even during a delivery lease or recent mouse input.
        let delivering = self.delivering.load(Ordering::SeqCst);
        if delivering && front == s.target {
            return;
        }
        let previous = s.previous;
        if s.observe(front, user_switched_recently(), Instant::now())
            .is_some()
        {
            self.restore_locked(&s, "activation");
        } else if s.previous != previous || s.window.is_none() || Instant::now() >= s.armed_until {
            s.window = ax_actions::focused_window(s.previous);
        }
        // A non-activating menu can take key focus without an activation notification.
        if !delivering
            && skylight::key_focus_pid().is_some_and(|pid| {
                pid != s.previous
                    && (pid == s.target || pid == s.host || Instant::now() < s.armed_until)
            })
            && !user_switched_recently()
        {
            if let Some(own) = skylight::steal_key_focus() {
                skylight::release_key_focus(own);
                if release_newest_theft_below(own, skylight::release_key_focus).is_some() {
                    self.restore_locked(&s, "keyFocus");
                }
            }
        }
    }
    pub fn restore(&self) {
        self.restore_locked(&self.state.lock().unwrap(), "foregroundFallback");
    }
    pub fn restore_pid(&self, pid: i32, reason: &str) {
        let s = self.state.lock().unwrap();
        // A real user switch supersedes a scope's snapshot.
        if s.previous == pid {
            self.restore_locked(&s, reason);
        }
    }
    pub fn record(&self, pid: i32, window: Option<u32>, reason: &str) {
        self.restorations
            .lock()
            .unwrap()
            .push(json!({"pid":pid,"windowId":window,"reason":reason}));
    }
    fn restore_locked(&self, s: &State, reason: &str) {
        if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(s.previous)
        {
            #[allow(deprecated)]
            if app.activateWithOptions(NSApplicationActivationOptions(0)) {
                if let Some(window) = s.window {
                    ax_actions::restore_window(s.previous, window);
                }
                self.record(s.previous, s.window, reason);
            }
        }
    }
}

// Port of focus_steal::install_observer. One serial observer queue for the process;
// the weak lease limits it to the current run and never extends a run's lifetime.
static OBSERVED: Mutex<Weak<Session>> = Mutex::new(Weak::new());
fn install_observer(session: Weak<Session>) {
    use block2::RcBlock;
    use objc2_app_kit::NSWorkspaceDidActivateApplicationNotification;
    use objc2_foundation::{NSNotification, NSOperationQueue};
    *OBSERVED.lock().unwrap() = session;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let queue = NSOperationQueue::new();
        queue.setMaxConcurrentOperationCount(1);
        let block = RcBlock::new(move |_: std::ptr::NonNull<NSNotification>| {
            let session = OBSERVED.lock().unwrap().upgrade();
            if let Some(s) = session {
                if s.active.load(Ordering::SeqCst) && !s.foreground.load(Ordering::SeqCst) {
                    s.guard.tick();
                }
            }
        });
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidActivateApplicationNotification),
                None,
                Some(&queue),
                &block,
            )
        };
        // Same process-lifetime ownership as upstream; the callback contains only a Weak lease.
        std::mem::forget(token);
        std::mem::forget(queue);
    });
}
pub(super) fn front_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|a| a.processIdentifier())
        .or_else(skylight::front_process_pid)
}

pub fn release_newest_theft_below(own: i32, mut release: impl FnMut(i32) -> i32) -> Option<i32> {
    (1..=4)
        .map(|back| own.wrapping_sub(back))
        .find(|&id| id != 0 && release(id) == 0)
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
}
fn user_switched_recently() -> bool {
    // Hardware-only state, as in upstream key_focus_guard: synthetic events do not reset this clock.
    unsafe {
        [1, 3, 12]
            .iter()
            .any(|kind| CGEventSourceSecondsSinceLastEventType(1, *kind) < 1.0)
    }
}
