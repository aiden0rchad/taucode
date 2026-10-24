// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from focus_guard.rs, focus_steal.rs and key_focus_guard.rs; see NOTICE.
use super::{ax_actions, skylight, Session};
use objc2::{sel, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSRunningApplication, NSWorkspace,
};
use objc2_foundation::NSObjectProtocol;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock, Weak,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct State {
    pub previous: i32,
    pub window: Option<u32>,
    pub target: i32,
    pub host: i32,
    pub armed_until: Instant,
    pub baseline_key: Option<i32>,
    pub action_start: Instant,
    pub action_active: bool,
    pub key_attempted: bool,
    pub activation_attempted: bool,
    pub activation_escalated: bool,
    pub last_key_restore: Option<Instant>,
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
    action_prior: Mutex<Option<UserFocus>>,
    thefts: Mutex<Vec<i32>>,
    pub delivering: AtomicBool,
    pub restorations: Mutex<Vec<Value>>,
}
pub struct UserFocus {
    pub(super) pid: Option<i32>,
    pub(super) window: Option<u32>,
    pub(super) key_pid: Option<i32>,
}
pub fn action_restoration(previous: Option<i32>, front: Option<i32>, target: i32) -> Option<i32> {
    previous.filter(|pid| *pid != target && front == Some(target))
}
fn restore_action(
    previous: Option<i32>,
    target: i32,
    front: impl Fn() -> Option<i32>,
    restore: impl Fn(i32),
    wait: impl Fn(Duration),
) {
    if previous.is_none() || previous == Some(target) {
        return;
    }
    for delayed in [false, true] {
        if delayed {
            wait(Duration::from_millis(300));
        }
        if let Some(pid) = action_restoration(previous, front(), target) {
            restore(pid);
        }
    }
}
impl UserFocus {
    // Capture before any run UI is created or shown.
    pub fn capture() -> Self {
        let pid = front_pid();
        Self {
            pid,
            window: pid.and_then(skylight::key_window_of_pid),
            key_pid: skylight::key_focus_pid(),
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
                baseline_key: prior.key_pid,
                action_start: Instant::now(),
                action_active: false,
                key_attempted: false,
                activation_attempted: false,
                activation_escalated: false,
                last_key_restore: None,
            }),
            restorations: Mutex::new(vec![]),
            thefts: Mutex::new(vec![]),
            action_prior: Mutex::new(None),
            delivering: AtomicBool::new(false),
        }
    }
    pub fn arm(&self) {
        self.state.lock().unwrap().armed_until = Instant::now() + Duration::from_millis(250);
    }
    pub fn begin_action(&self) {
        self.begin_action_with(UserFocus::capture(), Instant::now());
    }
    pub(super) fn begin_action_with(&self, prior: UserFocus, now: Instant) {
        let mut s = self.state.lock().unwrap();
        if let Some(pid) = prior.pid.filter(|pid| *pid != s.target && *pid != s.host) {
            s.previous = pid;
            s.window = prior.window;
        }
        s.baseline_key = prior.key_pid;
        s.action_start = now;
        s.action_active = true;
        s.key_attempted = false;
        s.activation_attempted = false;
        s.activation_escalated = false;
        s.armed_until = now + Duration::from_millis(250);
        *self.action_prior.lock().unwrap() = Some(prior);
    }
    pub fn end_action(&self) {
        let Some(prior) = self.action_prior.lock().unwrap().take() else {
            return;
        };
        let target = self.state.lock().unwrap().target;
        restore_action(
            prior.pid,
            target,
            front_pid,
            |pid| {
                let mut state = self.state.lock().unwrap();
                if state.previous == pid {
                    self.restore_locked(
                        &mut state,
                        "postActionInvariant",
                        &Workspace,
                        Instant::now(),
                    );
                }
            },
            std::thread::sleep,
        );
        self.state.lock().unwrap().action_active = false;
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
        self.tick_with(&Workspace, Instant::now());
    }
    pub(super) fn tick_with(&self, f: &impl Focus, now: Instant) {
        let Some(front) = f.front() else { return };
        let mut s = self.state.lock().unwrap();
        let key = f.key_pid();
        let previous = s.previous;
        let user_switched = f.user_switched();
        let action_running = self.action_prior.lock().unwrap().is_some();
        let activation_is_user_switch = user_switched && !(front == s.target && action_running);
        if s.observe(front, activation_is_user_switch, now).is_some() {
            if self.restore_locked(&mut s, "activation", f, now) {
                return;
            }
        } else if s.previous != previous {
            s.baseline_key = key;
            s.window = f.window(s.previous);
        } else if s.window.is_none() || now >= s.armed_until {
            s.window = f.window(s.previous);
        }
        // An unchanged helper PID is the user's state, even while armed.
        let taken = key.is_some_and(|pid| {
            Some(pid) != s.baseline_key
                && pid != s.previous
                && (pid == s.target
                    || pid == s.host
                    || s.baseline_key.is_some() && s.action_active && now > s.action_start)
        });
        if taken
            && !user_switched
            && !self.delivering.load(Ordering::SeqCst)
            && !s.key_attempted
            && s.last_key_restore
                .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1))
        {
            s.key_attempted = true;
            s.last_key_restore = Some(now);
            // Never probe neighbouring IDs: they can belong to unrelated apps.
            self.thefts.lock().unwrap().retain(|id| !f.release(*id));
            let destination = s.baseline_key.unwrap_or(s.previous);
            let window = if destination == s.previous {
                s.window
            } else {
                f.window(destination)
            };
            if f.key_pid() != Some(destination) {
                f.restore_key(destination, window, key.unwrap());
            }
            self.record_locked(
                &s,
                s.previous,
                s.window,
                "keyFocus",
                key,
                Some(front),
                f,
                now,
            );
            if f.key_pid() == key {
                self.record_locked(
                    &s,
                    s.previous,
                    s.window,
                    "keyFocusUnresolved",
                    key,
                    f.front(),
                    f,
                    now,
                );
            }
        }
    }
    pub fn track_theft(&self, id: i32) {
        self.thefts.lock().unwrap().push(id);
    }
    pub(super) fn release_theft(&self, id: i32, f: &impl Focus) {
        let mut thefts = self.thefts.lock().unwrap();
        if let Some(index) = thefts.iter().position(|owned| *owned == id) {
            if f.release(id) {
                thefts.remove(index);
            }
        }
    }
    pub fn restore(&self) {
        self.restore_locked(
            &mut self.state.lock().unwrap(),
            "foregroundFallback",
            &Workspace,
            Instant::now(),
        );
    }
    pub fn restore_pid(&self, pid: i32, reason: &str) {
        let mut s = self.state.lock().unwrap();
        // A real user switch supersedes a scope's snapshot.
        if s.previous == pid {
            self.restore_locked(&mut s, reason, &Workspace, Instant::now());
        }
    }
    pub fn record(
        &self,
        pid: i32,
        window: Option<u32>,
        reason: &str,
        key: Option<i32>,
        front: Option<i32>,
    ) {
        self.record_locked(
            &self.state.lock().unwrap(),
            pid,
            window,
            reason,
            key,
            front,
            &Workspace,
            Instant::now(),
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn record_locked(
        &self,
        s: &State,
        pid: i32,
        window: Option<u32>,
        reason: &str,
        key: Option<i32>,
        front: Option<i32>,
        f: &impl Focus,
        now: Instant,
    ) {
        self.restorations.lock().unwrap().push(json!({
            "pid":pid, "windowId":window, "reason":reason,
            "keyPid":key, "keyBundle":key.and_then(|pid| f.bundle(pid)),
            "frontPid":front, "baselineKeyPid":s.baseline_key,
            "ms_since_action_start":now.saturating_duration_since(s.action_start).as_millis() as u64
        }));
    }
    // Returns true when a user switch superseded this restore.
    fn restore_locked(&self, s: &mut State, reason: &str, f: &impl Focus, now: Instant) -> bool {
        let Some(front) = f.front() else { return false };
        if front == s.previous || s.activation_attempted {
            return false;
        }
        s.activation_attempted = true;
        let key = f.key_pid();
        let started = Instant::now();
        let mut method = f.activate(s.previous, s.window, front);
        let mut result = verify_activation(f, s.previous, front, started);
        if result == Some(false) && !s.activation_escalated {
            s.activation_escalated = true;
            f.escalate(s.previous, s.window, front);
            method = "skylight";
            result = verify_activation(f, s.previous, front, started);
        }
        // A later theft can be restored again after success, but a failed restore
        // cannot loop and the action never gets a second escalation.
        s.activation_attempted = result != Some(true);
        self.restorations.lock().unwrap().push(json!({
            "pid":s.previous, "windowId":s.window, "reason":"activation", "trigger":reason,
            "keyPid":key, "keyBundle":key.and_then(|pid| f.bundle(pid)),
            "frontPid":front, "baselineKeyPid":s.baseline_key,
            "ms_since_action_start":now.saturating_duration_since(s.action_start).as_millis() as u64,
            "verified":result == Some(true), "method":method
        }));
        if result.is_none() {
            if let Some(pid) = f.front() {
                s.previous = pid;
                s.window = f.window(pid);
                s.baseline_key = f.key_pid();
            }
            return true;
        }
        false
    }
    pub fn drain_restorations(&self) -> Vec<Value> {
        std::mem::take(&mut *self.restorations.lock().unwrap())
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

// None means the user chose another app (or focus could no longer be observed).
fn verify_activation(f: &impl Focus, previous: i32, from: i32, started: Instant) -> Option<bool> {
    for poll in 0..=15 {
        let front = f.front()?;
        if front == previous {
            return Some(true);
        }
        if front != from || f.user_switched_since(started) {
            return None;
        }
        if poll < 15 {
            f.wait(Duration::from_millis(20));
        }
    }
    Some(false)
}

fn on_main(f: impl Fn(MainThreadMarker) + Send + 'static) {
    if let Some(mtm) = MainThreadMarker::new() {
        f(mtm);
    } else {
        let block = block2::RcBlock::new(move || {
            f(MainThreadMarker::new().expect("main queue"));
        });
        unsafe { objc2_foundation::NSOperationQueue::mainQueue().addOperationWithBlock(&block) };
    }
}

pub(super) fn host_key_window() -> Option<u32> {
    let (tx, rx) = std::sync::mpsc::channel();
    on_main(move |mtm| {
        let window = NSApplication::sharedApplication(mtm)
            .keyWindow()
            .and_then(|window| u32::try_from(window.windowNumber()).ok());
        let _ = tx.send(window);
    });
    rx.recv_timeout(Duration::from_millis(100)).ok().flatten()
}

pub(super) trait Focus {
    fn front(&self) -> Option<i32>;
    fn key_pid(&self) -> Option<i32>;
    fn user_switched(&self) -> bool;
    fn user_switched_since(&self, started: Instant) -> bool;
    fn window(&self, pid: i32) -> Option<u32>;
    fn supports_activate_from(&self, pid: i32) -> bool;
    fn activate_app(&self, pid: i32, window: Option<u32>, from: i32, modern: bool);
    fn activate_host(&self, window: Option<u32>, from: i32);
    fn activate(&self, pid: i32, window: Option<u32>, from: i32) -> &'static str {
        if pid == std::process::id() as i32 {
            self.activate_host(window, from);
            "nsapp"
        } else {
            let modern = self.supports_activate_from(pid);
            self.activate_app(pid, window, from, modern);
            if modern {
                "activateFrom"
            } else {
                "legacy"
            }
        }
    }
    fn escalate(&self, pid: i32, window: Option<u32>, from: i32);
    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
    fn restore_key(&self, pid: i32, window: Option<u32>, from: i32);
    fn release(&self, id: i32) -> bool;
    fn bundle(&self, pid: i32) -> Option<String>;
}
pub(super) struct Workspace;
impl Focus for Workspace {
    fn front(&self) -> Option<i32> {
        front_pid()
    }
    fn key_pid(&self) -> Option<i32> {
        skylight::key_focus_pid()
    }
    fn user_switched(&self) -> bool {
        user_switched_recently()
    }
    fn user_switched_since(&self, started: Instant) -> bool {
        user_event_age() < started.elapsed().as_secs_f64()
    }
    fn window(&self, pid: i32) -> Option<u32> {
        skylight::key_window_of_pid(pid)
    }
    fn supports_activate_from(&self, pid: i32) -> bool {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .is_some_and(|app| app.respondsToSelector(sel!(activateFromApplication:options:)))
    }
    fn activate_app(&self, pid: i32, window: Option<u32>, from: i32, modern: bool) {
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return;
        };
        if front_pid() != Some(from) {
            return;
        }
        if modern {
            if let Some(source) =
                NSRunningApplication::runningApplicationWithProcessIdentifier(from)
            {
                app.activateFromApplication_options(&source, NSApplicationActivationOptions(0));
            }
        } else {
            #[allow(deprecated)]
            app.activateWithOptions(NSApplicationActivationOptions(0));
        }
        if front_pid() == Some(pid) {
            if let Some(window) = window {
                ax_actions::restore_window(pid, window);
            }
        }
    }
    fn activate_host(&self, window: Option<u32>, from: i32) {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(300);
        on_main(move |mtm| {
            // A queued restore must not override a later user switch or run after expiry.
            if Instant::now() >= deadline
                || front_pid() != Some(from)
                || Workspace.user_switched_since(started)
            {
                return;
            }
            let app = NSApplication::sharedApplication(mtm);
            if app.respondsToSelector(sel!(activate)) {
                app.activate();
            } else {
                #[allow(deprecated)]
                app.activateIgnoringOtherApps(true);
            }
            if let Some(window) = window.and_then(|id| app.windowWithWindowNumber(id as isize)) {
                window.makeKeyWindow();
            }
        });
    }
    fn escalate(&self, pid: i32, window: Option<u32>, from: i32) {
        if front_pid() == Some(from) {
            skylight::restore_activation(pid, window, from);
        }
    }
    fn restore_key(&self, pid: i32, window: Option<u32>, from: i32) {
        // HUD panels may have no layer-zero window; PSN lookup falls back to PID.
        let from_window = skylight::key_window_of_pid(from).unwrap_or(0);
        skylight::restore_focus_after_without_raise(pid, window, from, from_window);
    }
    fn release(&self, id: i32) -> bool {
        skylight::release_key_focus(id) == 0
    }
    fn bundle(&self, pid: i32) -> Option<String> {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .and_then(|app| app.bundleIdentifier())
            .map(|id| id.to_string())
    }
}
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
}
fn user_switched_recently() -> bool {
    user_event_age() < 1.0
}
fn user_event_age() -> f64 {
    // Hardware-only state, as in upstream key_focus_guard: synthetic events do not reset this clock.
    unsafe {
        [1, 3, 12]
            .iter()
            .map(|kind| CGEventSourceSecondsSinceLastEventType(1, *kind))
            .fold(f64::INFINITY, f64::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn ax_menu_restores_immediate_and_delayed_activation_but_respects_other_apps() {
        for (before, immediate, delayed, expected) in [
            (10, 20, 10, 1),
            (10, 10, 20, 1),
            (10, 20, 20, 2),
            (10, 30, 30, 0),
            (20, 20, 20, 0),
        ] {
            let front = Cell::new(immediate);
            let records = RefCell::new(vec![]);
            restore_action(
                Some(before),
                20,
                || Some(front.get()),
                |pid| {
                    records.borrow_mut().push(pid);
                    front.set(pid);
                },
                |duration| {
                    assert_eq!(duration, Duration::from_millis(300));
                    front.set(delayed);
                },
            );
            assert_eq!(records.borrow().as_slice(), vec![10; expected]);
        }
    }
}
