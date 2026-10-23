use super::{
    control::{Control, Event, Input},
    policy::*,
    runner::{self, Environment, Frame},
};
use crate::shared::providers::BoxFuture;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
struct FakeInput(Arc<Mutex<Vec<Event>>>);
impl Input for FakeInput {
    fn post(&mut self, e: &Event) -> Result<(), String> {
        self.0.lock().unwrap().push(e.clone());
        Ok(())
    }
}
fn display() -> Display {
    Display {
        width: 1440.,
        height: 900.,
        pixel_width: 2880,
        pixel_height: 1800,
    }
}
fn settings() -> Settings {
    Settings {
        allowed_apps: vec!["com.apple.TextEdit".into()],
        ..Default::default()
    }
}
fn context() -> Context {
    Context {
        app: "com.apple.TextEdit".into(),
        secure: false,
    }
}
fn prediction(action: Value) -> Prediction {
    let mut action = action;
    action["grounding_width"] = json!(1920);
    action["grounding_height"] = json!(1080);
    Prediction {
        action,
        reflection: "checked".into(),
        plan_text: "Click an editable document".into(),
    }
}
#[test]
fn coordinates_retina_and_grounding_sizes() {
    assert_eq!(
        map_point([960., 540.], [1920., 1080.], display()).unwrap(),
        [720., 450.]
    );
    assert_eq!(
        map_point([500., 500.], [1000., 1000.], display()).unwrap(),
        [720., 450.]
    );
    for p in [[-1., 1.], [1920., 0.], [f64::NAN, 2.]] {
        assert!(map_point(p, [1920., 1080.], display()).is_err());
    }
    assert!(map_point([0., 0.], [0., 1080.], display()).is_err());
    assert!(prediction(json!({"kind":"hotkey","keys":["invalid"]}))
        .validated(display())
        .is_err());
    assert!(
        prediction(json!({"kind":"wait","seconds":0,"code":"anything"}))
            .validated(display())
            .is_err()
    );
}
#[test]
fn policy_denylist_secure_submit_limits() {
    let mut s = settings();
    let c = context();
    let a = Action::Click {
        x: 1.,
        y: 2.,
        button: "left".into(),
        clicks: 1,
        hold_keys: vec![],
    };
    assert_eq!(
        decide(
            &a,
            "Click document",
            &c,
            &s,
            &s.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Allow
    );
    assert!(matches!(
        decide(&a, "BUY now", &c, &s, &s.allowed_apps, 0, Duration::ZERO),
        Decision::Confirm(_)
    ));
    let text = Action::Type {
        text: "secret".into(),
        x: None,
        y: None,
        overwrite: false,
        enter: false,
    };
    assert_eq!(
        decide(
            &text,
            "",
            &Context {
                secure: true,
                ..c.clone()
            },
            &s,
            &s.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Confirm("Typing into a secure text field")
    );
    assert!(matches!(
        decide(&a, "", &c, &s, &s.allowed_apps, s.max_steps, Duration::ZERO),
        Decision::Deny(_)
    ));
    assert!(matches!(
        decide(
            &a,
            "",
            &c,
            &s,
            &s.allowed_apps,
            0,
            Duration::from_secs(s.max_seconds)
        ),
        Decision::Deny(_)
    ));
    assert!(matches!(
        decide(&a, "", &c, &s, &[], 0, Duration::ZERO),
        Decision::Deny(_)
    ));
    for app in ALWAYS_DENIED {
        s.allowed_apps.push((*app).into());
        assert!(!app_allowed(app, &s, &s.allowed_apps));
    }
    s.denied_apps.push(c.app.clone());
    assert!(!app_allowed(&c.app, &s, &s.allowed_apps));
}
#[test]
fn stop_releases_keys_buttons_and_prevents_late_input() {
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    control
        .post(Event::Key(55, true), &context(), || Ok(context()))
        .unwrap();
    control
        .post(
            Event::Mouse {
                point: [1., 2.],
                button: 0,
                down: true,
                clicks: 1,
            },
            &context(),
            || Ok(context()),
        )
        .unwrap();
    let now = Instant::now();
    control.stop();
    assert!(now.elapsed() < Duration::from_millis(100));
    let events = input.0.lock().unwrap();
    assert!(events.contains(&Event::Key(55, false)));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Mouse { down: false, .. })));
    drop(events);
    assert!(control
        .post(Event::Key(0, true), &context(), || Ok(context()))
        .is_err());
}
struct Fake {
    predictions: std::collections::VecDeque<Prediction>,
    steps: Vec<Value>,
    approval: bool,
    approved: usize,
    captures: usize,
    focus: Context,
    change_after_approval: bool,
}
impl Environment for Fake {
    fn capture(&mut self) -> BoxFuture<'_, Result<Frame, String>> {
        self.captures += 1;
        Box::pin(async {
            Ok(Frame {
                png: "fake-png".into(),
                thumbnail: "fake-thumb".into(),
                display: display(),
            })
        })
    }
    fn predict<'a>(
        &'a mut self,
        _: &'a str,
        _: &'a Frame,
    ) -> BoxFuture<'a, Result<Prediction, String>> {
        Box::pin(async { self.predictions.pop_front().ok_or("No fake action".into()) })
    }
    fn context(&self) -> Result<Context, String> {
        Ok(self.focus.clone())
    }
    fn approve<'a>(
        &'a mut self,
        _: &'a str,
        _: &'a Action,
        _: &'a Context,
    ) -> BoxFuture<'a, Result<(), String>> {
        self.approved += 1;
        if self.change_after_approval {
            self.focus.app = "com.apple.Terminal".into();
        }
        Box::pin(async {
            if self.approval {
                Ok(())
            } else {
                Err("Declined".into())
            }
        })
    }
    fn open(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn progress(&mut self, v: Value) -> Result<(), String> {
        self.steps.push(v);
        Ok(())
    }
}
fn fake(actions: Vec<Value>) -> Fake {
    Fake {
        predictions: actions.into_iter().map(prediction).collect(),
        steps: vec![],
        approval: true,
        approved: 0,
        captures: 0,
        focus: context(),
        change_after_approval: false,
    }
}
#[tokio::test]
async fn loop_fake_capture_planner_input_and_progress() {
    let mut env = fake(vec![
        json!({"kind":"click","x":960,"y":540,"button":"left","clicks":1,"hold_keys":[]}),
        json!({"kind":"done","summary":"Complete"}),
    ]);
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = settings();
    let result = runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
        .await
        .unwrap();
    assert_eq!(result["summary"], "Complete");
    assert_eq!(env.steps.len(), 2);
    assert_eq!(env.captures, 3);
    assert!(input.0.lock().unwrap().contains(&Event::Mouse {
        point: [720., 450.],
        button: 0,
        down: true,
        clicks: 1
    }));
    assert!(control.stopped());
}
#[tokio::test]
async fn declined_approval_and_changed_focus_never_type() {
    for changed in [false, true] {
        let mut env = fake(vec![
            json!({"kind":"type","text":"secret","overwrite":false,"enter":true}),
        ]);
        env.approval = changed;
        env.change_after_approval = changed;
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let s = settings();
        assert!(runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
            .await
            .is_err());
        assert_eq!(env.approved, 1);
        assert!(input.0.lock().unwrap().is_empty());
    }
}
#[tokio::test]
async fn cancellation_interrupts_a_hung_sidecar() {
    let control = Arc::new(Control::new(FakeInput::default()));
    control.start();
    let cloned = control.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(5)).await;
        cloned.stop();
    });
    let now = Instant::now();
    assert!(control
        .until(std::future::pending::<Result<(), String>>())
        .await
        .is_err());
    assert!(now.elapsed() < Duration::from_millis(100));
}
#[test]
fn connector_uses_app_executable_and_action_policy() {
    let mut app = crate::types::AppSettings::default();
    app.computer.enabled = true;
    let mut entries =
        crate::shared::connectors::effective(&app, &Default::default(), None).unwrap();
    crate::shared::connectors::configure_browser(&mut entries, std::path::Path::new("/data"))
        .unwrap();
    let entry = entries.iter().find(|s| s.id == "taucode-computer").unwrap();
    assert!(entry.enabled);
    assert_eq!(entry.approval, crate::types::McpApproval::Auto);
    let crate::types::McpTransport::Stdio { args, .. } = entry.transport.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(args, &["--tau-computer-mcp", "/data"]);
}

#[tokio::test]
async fn bootstrap_can_only_switch_to_allowed_app_and_step_limit_stops() {
    let mut env = fake(vec![
        json!({"kind":"switch_app","app":"com.apple.TextEdit"}),
    ]);
    env.focus.app = "dev.taucode.desktop".into();
    let control = Control::new(FakeInput::default());
    control.start();
    let s = Settings {
        max_steps: 1,
        ..settings()
    };
    assert_eq!(
        runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
            .await
            .unwrap_err(),
        "Maximum steps reached"
    );
    assert_eq!(env.steps.len(), 1);
    let click = Action::Click {
        x: 1.,
        y: 1.,
        button: "left".into(),
        clicks: 1,
        hold_keys: vec![],
    };
    assert!(matches!(
        decide(
            &click,
            "",
            &env.focus,
            &s,
            &s.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
    let terminal = Action::SwitchApp {
        app: "com.apple.Terminal".into(),
    };
    assert!(matches!(
        decide(
            &terminal,
            "",
            &env.focus,
            &s,
            &s.allowed_apps,
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
}

#[tokio::test]
async fn pause_releases_input_and_resume_waits_without_executing() {
    let input = FakeInput::default();
    let control = Arc::new(Control::new(input.clone()));
    control.start();
    control
        .post(Event::Key(55, true), &context(), || Ok(context()))
        .unwrap();
    let epoch = control.epoch();
    control.pause(true);
    assert_ne!(control.epoch(), epoch);
    assert_eq!(input.0.lock().unwrap().last(), Some(&Event::Key(55, false)));
    assert!(
        tokio::time::timeout(Duration::from_millis(30), control.checkpoint())
            .await
            .is_err()
    );
    control.pause(false);
    control.checkpoint().await.unwrap();
    control.stop();
}

#[tokio::test]
async fn wall_limit_cancels_wait_and_denied_app_is_never_captured() {
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = Settings {
        max_seconds: 1,
        ..settings()
    };
    let mut env = fake(vec![json!({"kind":"wait","seconds":10})]);
    assert_eq!(
        runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
            .await
            .unwrap_err(),
        "Maximum wall time reached"
    );
    assert!(control.stopped());
    control.start();
    env.focus.app = "com.apple.Terminal".into();
    env.captures = 0;
    assert!(runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
        .await
        .is_err());
    assert_eq!(env.captures, 0);
    assert!(input.0.lock().unwrap().is_empty());
}

#[test]
fn grounding_mode_defaults_and_pixel_space_maps_to_display_points() {
    let mut s: Settings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(s.grounding_mode, "pixels");
    validate(&s).unwrap();
    s.grounding_mode = "ui-tars".into();
    validate(&s).unwrap();
    s.grounding_mode = "unknown".into();
    assert_eq!(validate(&s), Err("Invalid grounding mode".into()));
    let mut p = prediction(
        json!({"kind":"click","x":1600,"y":100,"button":"left","clicks":1,"hold_keys":[]}),
    );
    p.action["grounding_width"] = json!(1920);
    p.action["grounding_height"] = json!(1200);
    let display = Display {
        width: 1600.,
        height: 1000.,
        pixel_width: 3200,
        pixel_height: 2000,
    };
    let Action::Click { x, y, .. } = p.validated(display).unwrap() else {
        panic!("Expected click")
    };
    assert!((x - 1333.3333333333333).abs() < 0.001);
    assert!((y - 83.33333333333333).abs() < 0.001);
}

#[tokio::test]
async fn prediction_error_is_logged_without_input() {
    let mut env = fake(vec![]);
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = settings();
    let error = runner::run(&mut env, &control, &s, &s.allowed_apps, "test")
        .await
        .unwrap_err();
    assert_eq!(error, "No fake action");
    assert_eq!(env.steps.len(), 1);
    assert_eq!(env.steps[0]["error"], error);
    assert_eq!(env.steps[0]["action"]["reason"], error);
    assert_eq!(env.steps[0]["thumbnail"], "fake-thumb");
    assert!(input.0.lock().unwrap().is_empty());
    assert!(control.stopped());
}
