use super::super::policy::{
    app_allowed, decide, map_point, Decision, Display, Settings, ALWAYS_DENIED,
};
use super::*;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

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
            false,
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
            false,
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
        app: settings.allowed_apps[0].clone(),
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
            &settings,
            &settings.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Allow
    );
    assert!(matches!(
        decide(
            &Action::SwitchApp {
                app: settings.allowed_apps[1].clone()
            },
            "",
            &context,
            &settings,
            &settings.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
    for app in ALWAYS_DENIED {
        assert!(!app_allowed(app, &settings, &settings.allowed_apps));
    }
    assert!(matches!(
        decide(
            &Action::SwitchApp { app: context.app },
            "",
            &Context {
                app: "dev.taucode.desktop".into(),
                secure: false
            },
            &settings,
            &settings.allowed_apps,
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
