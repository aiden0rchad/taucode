use super::super::policy::{
    app_allowed, decide, map_point, Decision, Display, Settings, ALWAYS_DENIED,
};
use super::*;
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

#[test]
fn ax_hit_test_is_not_reported_as_element_grounding() {
    assert_eq!(ax_path(true), "ax-element");
    assert_eq!(ax_path(false), "ax-hit-test");
}

#[test]
fn execution_table_uses_fake_ax_and_posting() {
    let actions = [
        (
            Action::Click {
                x: 1.,
                y: 2.,
                button: "left".into(),
                clicks: 1,
                hold_keys: vec![],
            },
            true,
        ),
        (
            Action::Click {
                x: 1.,
                y: 2.,
                button: "right".into(),
                clicks: 1,
                hold_keys: vec![],
            },
            true,
        ),
        (
            Action::Click {
                x: 1.,
                y: 2.,
                button: "left".into(),
                clicks: 2,
                hold_keys: vec![],
            },
            false,
        ),
        (
            Action::Type {
                text: "test".into(),
                x: None,
                y: None,
                overwrite: true,
                enter: false,
            },
            true,
        ),
        (
            Action::Type {
                text: "test".into(),
                x: None,
                y: None,
                overwrite: false,
                enter: true,
            },
            false,
        ),
        (
            Action::Scroll {
                x: 1.,
                y: 2.,
                clicks: 1,
                shift: false,
            },
            true,
        ),
        (
            Action::Hotkey {
                keys: vec!["enter".into()],
            },
            true,
        ),
        (
            Action::Drag {
                from: [0., 0.],
                to: [1., 1.],
                hold_keys: vec![],
            },
            false,
        ),
    ];
    for (action, eligible) in actions {
        for ax in [false, true] {
            for events in [false, true] {
                for fallback in [false, true] {
                    let called = Cell::new(false);
                    let path = prepare_action(
                        &action,
                        || {
                            called.set(true);
                            Ok(ax)
                        },
                        || events,
                        fallback,
                    )
                    .unwrap();
                    assert_eq!(called.get(), eligible);
                    assert_eq!(
                        path,
                        if ax && eligible {
                            Path::Ax
                        } else if events {
                            Path::Events
                        } else if fallback {
                            Path::ApprovalRequired
                        } else {
                            Path::Unavailable
                        }
                    );
                }
            }
        }
    }
    let action = Action::Type {
        text: "test".into(),
        x: None,
        y: None,
        overwrite: true,
        enter: false,
    };
    assert!(prepare_action(&action, || Err("AX timeout".into()), || true, true).is_err());
}

#[test]
fn missing_skylight_symbol_posts_once_to_public_pid_route() {
    for available in [false, true] {
        let posts = Cell::new(0);
        assert_eq!(
            post_route(available, || posts.set(posts.get() + 1)),
            if available { "skylight" } else { "postToPid" }
        );
        assert_eq!(posts.get(), usize::from(!available));
    }
}

#[test]
fn recoverable_ax_errors_and_lookup_failure_fall_through_once() {
    let action = Action::Click {
        x: 1.,
        y: 2.,
        button: "left".into(),
        clicks: 1,
        hold_keys: vec![],
    };
    for status in [-25204, -25205, -25206] {
        for events_available in [false, true] {
            let calls = Cell::new(0);
            let result = prepare_action(
                &action,
                || ax_actions::action_status(status),
                || {
                    calls.set(calls.get() + 1);
                    events_available
                },
                false,
            )
            .unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(
                result,
                if events_available {
                    Path::Events
                } else {
                    Path::Unavailable
                }
            );
        }
    }
    // Missing elements return false from AX lookup, sharing the one-shot fallback.
    assert_eq!(
        prepare_action(&action, || Ok(false), || true, false).unwrap(),
        Path::Events
    );
    assert_eq!(
        prepare_action(
            &action,
            || Ok(true),
            || panic!("AX already handled action"),
            false
        )
        .unwrap(),
        Path::Ax
    );
    assert!(prepare_action(
        &action,
        || ax_actions::action_status(-25201),
        || panic!("not retryable"),
        false
    )
    .is_err());
    assert_eq!(event_path(true, "skylight"), "pixel-skylight");
    assert_eq!(event_path(true, "postToPid"), "pixel-skylight");
    assert_eq!(event_path(false, "skylight"), "pixel-skylight");
}

#[test]
fn window_coordinates_include_retina_and_negative_display_offsets() {
    let display = Display {
        origin: [-1920., -300.],
        width: 800.,
        height: 600.,
        pixel_width: 1600,
        pixel_height: 1200,
    };
    assert_eq!(
        map_point([800., 600.], [1600., 1200.], display).unwrap(),
        [-1520., 0.]
    );
    assert_eq!(
        map_point([400., 300.], [800., 600.], display).unwrap(),
        [-1520., 0.]
    );
    assert!(map_point([1600., 0.], [1600., 1200.], display).is_err());
    assert!(map_point([f64::NAN, 0.], [1600., 1200.], display).is_err());
}

#[test]
fn focus_guard_fake_workspace_preserves_user_switches() {
    let now = Instant::now();
    let mut s = focus_guard::State {
        previous: 10,
        target: 20,
        host: 99,
        window: Some(100),
        armed_until: now + Duration::from_millis(250),
        baseline_key: Some(10),
        action_start: now,
        action_active: true,
        key_attempted: false,
        activation_attempted: false,
        activation_escalated: false,
        last_key_restore: None,
    };
    assert_eq!(s.observe(10, false, now), None);
    assert_eq!(s.observe(20, false, now), Some(10));
    assert_eq!(s.observe(30, false, now), Some(10)); // helper opened by an action
    assert_eq!(s.observe(40, true, now), None); // user-selected app
    assert_eq!(s.previous, 40);
    assert_eq!(s.observe(20, false, now), Some(40));
    assert_eq!(s.observe(50, false, now + Duration::from_secs(1)), None);
    assert_eq!(s.observe(99, true, now + Duration::from_secs(2)), Some(50));
    assert_eq!(s.previous, 50); // taucode is theft even outside the armed interval
}

#[test]
fn background_policy_checks_bound_app_and_never_bootstraps_taucode() {
    let settings = Settings {
        allowed_apps: vec!["com.apple.TextEdit".into(), "com.apple.Calculator".into()],
        ..Default::default()
    };
    assert_eq!(settings.mode, "background");
    assert!(!settings.allow_foreground_fallback);
    assert!(settings.virtual_cursor);
    let context = Context {
        app: settings.allowed_apps[0].bundle_id.clone(),
        secure: false,
    };
    let action = Action::Click {
        x: 0.,
        y: 0.,
        button: "left".into(),
        clicks: 1,
        hold_keys: vec![],
    };
    assert_eq!(
        decide(
            &action,
            "document",
            &context,
            &super::super::policy::Target::default(),
            &settings,
            &settings.app_ids(),
            0,
            Duration::ZERO
        ),
        Decision::Allow
    );
    assert!(matches!(
        decide(
            &Action::SwitchApp {
                app: settings.allowed_apps[1].bundle_id.clone()
            },
            "",
            &context,
            &super::super::policy::Target::default(),
            &settings,
            &settings.app_ids(),
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
    for app in ALWAYS_DENIED {
        assert!(!app_allowed(app, &settings, &settings.app_ids()));
    }
    assert!(matches!(
        decide(
            &Action::SwitchApp { app: context.app },
            "",
            &Context {
                app: "dev.taucode.desktop".into(),
                secure: false
            },
            &super::super::policy::Target::default(),
            &settings,
            &settings.app_ids(),
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
}

#[test]
fn overlay_geometry_and_capture_flags_are_noninteractive() {
    assert_eq!(overlay::glide([-100., 50.], [200., -50.], 0.), [-100., 50.]);
    assert_eq!(overlay::glide([-100., 50.], [200., -50.], 1.), [200., -50.]);
    assert_eq!(overlay::glide([0., 0.], [100., 100.], 0.5), [50., 50.]);
    assert_eq!(overlay::SHARING_TYPE, 0);
    assert_eq!(overlay::NONACTIVATING_PANEL, 128);
    assert!(overlay::IGNORE_MOUSE);
}

struct FocusWorkspace {
    front: Cell<i32>,
    key: Cell<i32>,
    switched: Cell<bool>,
    effective: bool,
    activations: Cell<usize>,
    releases: RefCell<Vec<i32>>,
    focus_records: Cell<usize>,
    activation_effective: bool,
    modern: bool,
    activation_calls: RefCell<Vec<(&'static str, i32, Option<u32>, i32)>>,
    escalations: Cell<usize>,
    escalation_effective: bool,
    waits: Cell<usize>,
    switch_on_wait: Option<(usize, i32)>,
    activate_on_wait: Option<(usize, i32)>,
}
impl FocusWorkspace {
    fn new(key: i32, effective: bool) -> Self {
        Self {
            front: Cell::new(10),
            key: Cell::new(key),
            switched: Cell::new(false),
            effective,
            activations: Cell::new(0),
            releases: RefCell::new(vec![]),
            focus_records: Cell::new(0),
            activation_effective: true,
            modern: true,
            activation_calls: RefCell::new(vec![]),
            escalations: Cell::new(0),
            escalation_effective: false,
            waits: Cell::new(0),
            switch_on_wait: None,
            activate_on_wait: None,
        }
    }
    fn prior(&self) -> UserFocus {
        UserFocus {
            pid: Some(self.front.get()),
            window: Some(100),
            key_pid: Some(self.key.get()),
        }
    }
}
impl focus_guard::Focus for FocusWorkspace {
    fn front(&self) -> Option<i32> {
        Some(self.front.get())
    }
    fn key_pid(&self) -> Option<i32> {
        Some(self.key.get())
    }
    fn user_switched(&self) -> bool {
        self.switched.get()
    }
    fn user_switched_since(&self, _: Instant) -> bool {
        self.switch_on_wait
            .is_some_and(|(at, _)| self.waits.get() >= at)
    }
    fn window(&self, pid: i32) -> Option<u32> {
        Some(pid as u32 * 10)
    }
    fn supports_activate_from(&self, _: i32) -> bool {
        self.modern
    }
    fn activate_app(&self, pid: i32, window: Option<u32>, from: i32, modern: bool) {
        self.activations.set(self.activations.get() + 1);
        self.activation_calls.borrow_mut().push((
            if modern { "activateFrom" } else { "legacy" },
            pid,
            window,
            from,
        ));
        if self.activation_effective {
            self.front.set(pid);
        }
    }
    fn activate_host(&self, window: Option<u32>, from: i32) {
        let pid = std::process::id() as i32;
        self.activations.set(self.activations.get() + 1);
        self.activation_calls
            .borrow_mut()
            .push(("nsapp", pid, window, from));
        if self.activation_effective {
            self.front.set(pid);
        }
    }
    fn escalate(&self, pid: i32, _: Option<u32>, _: i32) {
        self.escalations.set(self.escalations.get() + 1);
        if self.escalation_effective {
            self.front.set(pid);
        }
    }
    fn wait(&self, duration: Duration) {
        assert_eq!(duration, Duration::from_millis(20));
        let waits = self.waits.get() + 1;
        self.waits.set(waits);
        if let Some((at, pid)) = self.switch_on_wait {
            if waits == at {
                self.front.set(pid);
                self.switched.set(true);
            }
        }
        if let Some((at, pid)) = self.activate_on_wait {
            if waits == at {
                self.front.set(pid);
            }
        }
    }
    fn restore_key(&self, pid: i32, _: Option<u32>, _: i32) {
        self.focus_records.set(self.focus_records.get() + 1);
        if self.effective {
            self.key.set(pid);
        }
    }
    fn release(&self, id: i32) -> bool {
        self.releases.borrow_mut().push(id);
        true
    }
    fn bundle(&self, pid: i32) -> Option<String> {
        Some(format!("app.{pid}"))
    }
}

#[test]
fn focus_guard_ignores_unchanged_helper_target_and_host_baselines() {
    for key in [30, 20, std::process::id() as i32] {
        let f = FocusWorkspace::new(key, false);
        let guard = focus_guard::Guard::new(20, f.prior());
        let now = Instant::now();
        guard.tick_with(&f, now);
        for action in 0..3 {
            let start = now + Duration::from_secs(action);
            guard.begin_action_with(f.prior(), start);
            guard.arm();
            for tick in 1..30 {
                guard.tick_with(&f, start + Duration::from_millis(tick * 10));
            }
        }
        assert!(guard.drain_restorations().is_empty());
        assert_eq!(f.activations.get(), 0);
        assert_eq!(f.focus_records.get(), 0);
        assert!(f.releases.borrow().is_empty());
    }
}

#[test]
fn focus_guard_arm_alone_does_not_attribute_foreign_key_changes() {
    let f = FocusWorkspace::new(30, true);
    let guard = focus_guard::Guard::new(20, f.prior());
    guard.arm();
    f.key.set(40);
    guard.tick_with(&f, Instant::now());
    assert!(guard.drain_restorations().is_empty());
    assert_eq!(f.activations.get(), 0);
    assert_eq!(f.focus_records.get(), 0);
}

#[test]
fn focus_guard_restores_changed_key_once_and_records_unresolved_once() {
    for key in [20, std::process::id() as i32, 40] {
        for effective in [true, false] {
            let f = FocusWorkspace::new(30, effective);
            let guard = focus_guard::Guard::new(20, f.prior());
            let now = Instant::now();
            guard.begin_action_with(f.prior(), now);
            f.key.set(key);
            for tick in 1..200 {
                guard.tick_with(&f, now + Duration::from_millis(tick * 10));
            }
            let records = guard.drain_restorations();
            assert_eq!(records.len(), if effective { 1 } else { 2 });
            assert_eq!(records[0]["reason"], "keyFocus");
            assert_eq!(records[0]["keyPid"], key);
            assert_eq!(records[0]["keyBundle"], format!("app.{key}"));
            assert_eq!(records[0]["frontPid"], 10);
            assert_eq!(records[0]["baselineKeyPid"], 30);
            assert_eq!(records[0]["ms_since_action_start"], 10);
            if !effective {
                assert_eq!(records[1]["reason"], "keyFocusUnresolved");
            }
            assert_eq!(f.activations.get(), 0);
            assert_eq!(f.focus_records.get(), 1);
            assert!(guard.drain_restorations().is_empty());
        }
    }
}

#[test]
fn focus_guard_debounces_across_actions_and_drains_each_step() {
    let f = FocusWorkspace::new(10, true);
    let guard = focus_guard::Guard::new(20, f.prior());
    let now = Instant::now();
    guard.begin_action_with(f.prior(), now);
    f.key.set(20);
    guard.tick_with(&f, now + Duration::from_millis(10));
    assert_eq!(guard.drain_restorations().len(), 1);
    guard.begin_action_with(f.prior(), now + Duration::from_millis(20));
    f.key.set(20);
    guard.tick_with(&f, now + Duration::from_millis(30));
    assert!(guard.drain_restorations().is_empty());
    guard.tick_with(&f, now + Duration::from_millis(1010));
    let second = guard.drain_restorations();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0]["ms_since_action_start"], 990);
    assert_eq!(f.activations.get(), 0);
}

#[test]
fn focus_guard_releases_only_session_thefts() {
    let f = FocusWorkspace::new(10, true);
    let guard = focus_guard::Guard::new(20, f.prior());
    guard.track_theft(8);
    guard.release_theft(7, &f);
    assert!(f.releases.borrow().is_empty());
    guard.release_theft(8, &f);
    guard.release_theft(8, &f);
    assert_eq!(*f.releases.borrow(), [8]);
    guard.track_theft(12);
    let now = Instant::now();
    guard.begin_action_with(f.prior(), now);
    f.key.set(20);
    guard.tick_with(&f, now + Duration::from_millis(10));
    assert_eq!(*f.releases.borrow(), [8, 12]);
}

#[test]
fn focus_guard_workspace_restores_activation_but_respects_user_switch() {
    let f = FocusWorkspace::new(10, true);
    let guard = focus_guard::Guard::new(20, f.prior());
    let now = Instant::now();
    guard.begin_action_with(f.prior(), now);
    f.front.set(20);
    guard.tick_with(&f, now + Duration::from_millis(10));
    guard.tick_with(&f, now + Duration::from_millis(20));
    assert_eq!(f.activations.get(), 1);
    assert_eq!(guard.drain_restorations()[0]["reason"], "activation");
    f.front.set(40);
    f.key.set(41);
    f.switched.set(true);
    guard.tick_with(&f, now + Duration::from_millis(30));
    f.switched.set(false);
    guard.tick_with(&f, now + Duration::from_secs(2));
    assert_eq!(f.front.get(), 40);
    assert_eq!(f.activations.get(), 1);
    assert_eq!(f.focus_records.get(), 0);
    assert!(guard.drain_restorations().is_empty());
}

#[test]
fn activation_restores_host_and_transfers_from_the_active_app() {
    let host = std::process::id() as i32;
    for (previous, source, modern, method) in [
        (host, 20, true, "nsapp"),
        (10, 20, true, "activateFrom"),
        (10, 30, true, "activateFrom"), // action-attributed helper
        (10, 20, false, "legacy"),
    ] {
        let mut f = FocusWorkspace::new(previous, true);
        f.front.set(previous);
        f.modern = modern;
        f.activation_effective = false;
        f.activate_on_wait = Some((3, previous));
        let guard = focus_guard::Guard::new(20, f.prior());
        let now = Instant::now();
        guard.begin_action_with(f.prior(), now);
        f.front.set(source);
        guard.tick_with(&f, now + Duration::from_millis(10));
        guard.tick_with(&f, now + Duration::from_millis(20));
        assert_eq!(
            *f.activation_calls.borrow(),
            [(method, previous, Some(100), source)]
        );
        assert_eq!(f.waits.get(), 3);
        assert_eq!(f.escalations.get(), 0);
        let records = guard.drain_restorations();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["reason"], "activation");
        assert_eq!(records[0]["method"], method);
        assert_eq!(records[0]["verified"], true);
    }
}

#[test]
fn activation_escalates_once_per_action_and_records_actual_verification() {
    let host = std::process::id() as i32;
    for previous in [10, host] {
        for effective in [false, true] {
            let mut f = FocusWorkspace::new(previous, true);
            f.front.set(previous);
            f.activation_effective = false;
            f.escalation_effective = effective;
            let guard = focus_guard::Guard::new(20, f.prior());
            let now = Instant::now();
            guard.begin_action_with(f.prior(), now);
            f.front.set(20);
            for elapsed in [10, 20, 1000] {
                guard.tick_with(&f, now + Duration::from_millis(elapsed));
            }
            assert_eq!(f.activations.get(), 1);
            assert_eq!(f.escalations.get(), 1);
            assert_eq!(f.waits.get(), if effective { 15 } else { 40 });
            let records = guard.drain_restorations();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0]["method"], "skylight");
            assert_eq!(records[0]["verified"], effective);

            f.front.set(previous);
            guard.begin_action_with(f.prior(), now + Duration::from_secs(2));
            f.front.set(20);
            guard.tick_with(&f, now + Duration::from_millis(2010));
            assert_eq!(f.escalations.get(), 2);
            assert_eq!(guard.drain_restorations().len(), 1);
        }
    }
}

#[test]
fn activation_verifies_delayed_escalation_through_its_own_500ms_window() {
    // 370 ms after escalation is observed at the next 20 ms poll; also test the boundary.
    for delay in [370u64, 500, 520] {
        let mut f = FocusWorkspace::new(10, true);
        f.activation_effective = false;
        f.activate_on_wait = Some((15 + delay.div_ceil(20) as usize, 10));
        let guard = focus_guard::Guard::new(20, f.prior());
        let now = Instant::now();
        guard.begin_action_with(f.prior(), now);
        f.front.set(20);
        guard.tick_with(&f, now + Duration::from_millis(10));
        guard.tick_with(&f, now + Duration::from_millis(20));
        assert_eq!(f.escalations.get(), 1);
        assert_eq!(f.waits.get(), 15 + delay.div_ceil(20).min(25) as usize);
        let records = guard.drain_restorations();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["method"], "skylight");
        assert_eq!(records[0]["verified"], delay <= 500);
    }
}

#[test]
fn activation_verification_respects_user_switch_before_and_after_escalation() {
    for (at, destination) in [(1, 40), (15, 40), (16, 40), (1, 20)] {
        let mut f = FocusWorkspace::new(10, true);
        f.activation_effective = false;
        f.switched.set(true); // The initiating click is already recent; detect the next one too.
        f.switch_on_wait = Some((at, destination));
        let guard = focus_guard::Guard::new(20, f.prior());
        let now = Instant::now();
        guard.begin_action_with(f.prior(), now);
        f.front.set(20);
        f.key.set(20);
        guard.tick_with(&f, now + Duration::from_millis(10));
        guard.tick_with(&f, now + Duration::from_millis(20));
        assert_eq!(f.front.get(), destination);
        assert_eq!(f.activations.get(), 1);
        assert_eq!(f.escalations.get(), usize::from(at > 15));
        assert_eq!(f.focus_records.get(), 0);
        let records = guard.drain_restorations();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["verified"], false);
    }
}

#[test]
fn activation_restores_later_thefts_but_never_escalates_twice_in_one_action() {
    for primary_works in [true, false] {
        let mut f = FocusWorkspace::new(10, true);
        f.activation_effective = primary_works;
        f.escalation_effective = true;
        let guard = focus_guard::Guard::new(20, f.prior());
        let now = Instant::now();
        guard.begin_action_with(f.prior(), now);
        for elapsed in [10, 100, 200] {
            f.front.set(20);
            guard.tick_with(&f, now + Duration::from_millis(elapsed));
        }
        assert_eq!(f.activations.get(), if primary_works { 3 } else { 2 });
        assert_eq!(f.escalations.get(), usize::from(!primary_works));
    }
}
