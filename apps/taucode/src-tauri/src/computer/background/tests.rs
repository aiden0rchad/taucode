use super::super::policy::{
    app_allowed, decide, map_point, Decision, Display, Settings, ALWAYS_DENIED,
};
use super::*;
use std::{
    cell::Cell,
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
            false,
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
    let mut tried = vec![];
    assert_eq!(
        focus_guard::release_newest_theft_below(8, |id| {
            tried.push(id);
            if id == 6 {
                0
            } else {
                1010
            }
        }),
        Some(6)
    );
    assert_eq!(tried, [7, 6]);
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
