// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from tools/click.rs restore_user_focus_after_background_pixel,
// tools/drag.rs and input/mouse.rs prepare_background_pixel_click. See NOTICE.
use super::{focus_guard, skylight, Session};
use std::time::{Duration, Instant};

pub trait Focus {
    fn front(&self) -> Option<i32>;
    fn key_window(&self, pid: i32) -> Option<u32>;
    fn prime(&self, pid: i32, window: u32) -> bool;
    fn steal(&self, window: u32) -> Option<i32>;
    fn release(&self, id: i32);
    fn wait(&self, ms: u64);
    fn activate(&self, pid: i32, settle: bool);
    fn key_pid(&self) -> Option<i32>;
    fn restore(&self, prior: i32, key: Option<u32>, pid: i32, window: u32);
}

pub struct Scope {
    pid: i32,
    window: u32,
    prior: Option<i32>,
    prior_key: Option<u32>,
    target_key: Option<u32>,
    primed: bool,
    theft: Option<i32>,
}
impl Scope {
    pub fn enter(f: &impl Focus, pid: i32, window: u32, keyboard: bool) -> Self {
        let prior = f.front();
        let prior_key = prior.and_then(|pid| f.key_window(pid));
        let target_key = f.key_window(pid);
        let primed = f.prime(pid, window);
        // Upstream waits even if the private recipe is missing/partially fails.
        f.wait(50);
        let theft = if keyboard { f.steal(window) } else { None };
        Self {
            pid,
            window,
            prior,
            prior_key,
            target_key,
            primed,
            theft,
        }
    }

    pub fn release_key(&mut self, f: &impl Focus) {
        if let Some(id) = self.theft.take() {
            f.release(id);
        }
    }
    // Called after all synthesized key/button ups, including pause/stop/error.
    pub fn finish(mut self, f: &impl Focus, settle: bool) {
        self.release_key(f);
        if let Some(key) = self.target_key.filter(|key| *key != self.window) {
            f.prime(self.pid, key);
        }
        let Some(prior) = self.prior.filter(|pid| *pid != self.pid) else {
            return;
        };
        if settle {
            f.wait(50);
        }
        // Observe actual state, even when the private prologue reported failure.
        let reactivated = f.front() == Some(self.pid);
        if reactivated {
            f.activate(prior, settle);
        }
        let key_moved = reactivated || (self.primed && f.key_pid().is_some_and(|pid| pid != prior));
        // Respect a real switch to a third app; do not disturb an unchanged key window.
        if key_moved && f.front() == Some(prior) {
            f.restore(prior, self.prior_key, self.pid, self.window);
        }
    }
}

impl Focus for Session {
    fn front(&self) -> Option<i32> {
        focus_guard::front_pid()
    }
    fn key_window(&self, pid: i32) -> Option<u32> {
        skylight::key_window_of_pid(pid)
    }
    fn prime(&self, pid: i32, window: u32) -> bool {
        skylight::is_focus_without_raise_available()
            && skylight::activate_without_raise_with(pid, window, false)
    }
    fn steal(&self, window: u32) -> Option<i32> {
        skylight::steal_window_key_focus(window)
    }
    fn release(&self, id: i32) {
        skylight::release_key_focus(id);
    }
    fn wait(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn activate(&self, pid: i32, settle: bool) {
        self.guard.restore_pid(pid, "backgroundInput");
        let deadline = Instant::now() + Duration::from_millis(300);
        while settle
            && self.active.load(std::sync::atomic::Ordering::SeqCst)
            && self.front() != Some(pid)
            && Instant::now() < deadline
        {
            self.wait(10);
        }
    }
    fn key_pid(&self) -> Option<i32> {
        skylight::key_focus_pid()
    }
    fn restore(&self, prior: i32, key: Option<u32>, pid: i32, window: u32) {
        if skylight::restore_focus_after_without_raise(prior, key, pid, window) {
            self.guard.record(prior, key, "backgroundKeyWindow");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        control::{Control, Event, Input},
        policy::Context,
    };
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc, Mutex,
    };

    struct Fake {
        log: Mutex<Vec<String>>,
        front: AtomicI32,
        key: AtomicI32,
        prime: AtomicBool,
    }
    impl Fake {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                log: Mutex::new(vec![]),
                front: AtomicI32::new(10),
                key: AtomicI32::new(10),
                prime: AtomicBool::new(true),
            })
        }
        fn note(&self, s: impl Into<String>) {
            self.log.lock().unwrap().push(s.into());
        }
    }
    impl Focus for Fake {
        fn front(&self) -> Option<i32> {
            Some(self.front.load(Ordering::SeqCst))
        }
        fn key_window(&self, pid: i32) -> Option<u32> {
            self.note(format!("snapshot:{pid}"));
            Some(pid as u32 * 10)
        }
        fn prime(&self, pid: i32, window: u32) -> bool {
            self.note(format!("prime:{pid}:{window}"));
            self.prime.load(Ordering::SeqCst)
        }
        fn steal(&self, window: u32) -> Option<i32> {
            self.note(format!("steal:{window}"));
            Some(7)
        }
        fn release(&self, id: i32) {
            self.note(format!("release:{id}"));
        }
        fn wait(&self, ms: u64) {
            self.note(format!("wait:{ms}"));
        }
        fn activate(&self, pid: i32, _: bool) {
            self.note(format!("activate:{pid}"));
            self.front.store(pid, Ordering::SeqCst);
        }
        fn key_pid(&self) -> Option<i32> {
            Some(self.key.load(Ordering::SeqCst))
        }
        fn restore(&self, prior: i32, key: Option<u32>, pid: i32, window: u32) {
            self.note(format!("restore:{prior}:{key:?}:{pid}:{window}"));
        }
    }
    #[test]
    fn pixel_prologue_settles_before_post_and_only_restores_moved_focus() {
        // Shared click/scroll/drag scope, including missing/partially failing SPI.
        for primed in [false, true] {
            for front_after in [10, 20, 30] {
                let f = Fake::new();
                f.prime.store(primed, Ordering::SeqCst);
                let scope = Scope::enter(&*f, 20, 200, false);
                f.note("post");
                f.front.store(front_after, Ordering::SeqCst);
                scope.finish(&*f, true);
                let mut expected = vec![
                    "snapshot:10",
                    "snapshot:20",
                    "prime:20:200",
                    "wait:50",
                    "post",
                    "wait:50",
                ];
                if front_after == 20 {
                    expected.extend(["activate:10", "restore:10:Some(100):20:200"]);
                }
                assert_eq!(*f.log.lock().unwrap(), expected);
            }
        }
    }
    #[test]
    fn restores_key_focus_without_reactivating_an_unchanged_front_app() {
        let f = Fake::new();
        let scope = Scope::enter(&*f, 20, 200, false);
        f.key.store(20, Ordering::SeqCst);
        scope.finish(&*f, true);
        assert_eq!(
            f.log.lock().unwrap().last().unwrap(),
            "restore:10:Some(100):20:200"
        );
        assert!(!f
            .log
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.starts_with("activate:")));
    }
    struct ScopedInput {
        f: Arc<Fake>,
        scope: Option<Scope>,
        fail: bool,
    }
    impl Input for ScopedInput {
        fn post(&mut self, event: &Event) -> Result<(), String> {
            self.f.note(format!("{event:?}"));
            if self.fail {
                Err("fake posting failure".into())
            } else {
                Ok(())
            }
        }
        fn end_action(&mut self, cancelled: bool) -> Option<Box<dyn FnOnce() + Send>> {
            let mut scope = self.scope.take()?;
            scope.release_key(&*self.f);
            let f = self.f.clone();
            Some(Box::new(move || scope.finish(&*f, !cancelled)))
        }
    }
    #[test]
    fn keyboard_scope_releases_held_input_before_theft_on_stop_pause_and_post_failure() {
        for mode in ["stop", "pause", "error", "complete"] {
            let f = Fake::new();
            let control = Control::new(ScopedInput {
                f: f.clone(),
                scope: None,
                fail: false,
            });
            control.start();
            let scope = Scope::enter(&*f, 20, 200, true);
            control
                .install_input(ScopedInput {
                    f: f.clone(),
                    scope: Some(scope),
                    fail: mode == "error",
                })
                .unwrap();
            let context = Context {
                app: "target".into(),
                secure: false,
            };
            let posted = control.post(Event::Key(12, true), &context, || Ok(context.clone()));
            assert_eq!(posted.is_err(), mode == "error");
            match mode {
                "pause" => control.pause(true),
                "complete" => control.release(),
                _ => control.stop(),
            }
            control.stop(); // cleanup is idempotent
            let mut expected = vec![
                "snapshot:10",
                "snapshot:20",
                "prime:20:200",
                "wait:50",
                "steal:200",
                "Key(12, true)",
                "Key(12, false)",
                "release:7",
            ];
            if mode == "complete" {
                expected.push("wait:50");
            }
            assert_eq!(*f.log.lock().unwrap(), expected);
        }
    }
}
