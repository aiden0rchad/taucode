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
#[test]
fn mcp_steps_keep_planner_actions_and_diagnostics_without_images_or_ax_values() {
    let steps = json!([{"step":1,"action":{"kind":"type","text":"planner text"},"thumbnail":"image","planText":"界".repeat(1500),"path":"ax-menu","menuTitle":"File > New","grounding":[{"value":"private AX text"}],"executeMs":13,"focusRestorations":[{"pid":7}],"error":null}, {"step":2,"action":{"kind":"fail"},"error":"failed"}]);
    let compact = super::compact_steps(&steps);
    assert_eq!(compact.len(), 2);
    assert_eq!(compact[0]["action"], steps[0]["action"]);
    assert_eq!(
        compact[0]["planText"].as_str().unwrap().chars().count(),
        1024
    );
    for key in ["thumbnail", "grounding"] {
        assert!(compact[0].get(key).is_none());
    }
    for key in ["path", "menuTitle", "executeMs", "focusRestorations"] {
        assert_eq!(compact[0][key], steps[0][key]);
    }
    assert_eq!(compact[1]["error"], "failed");
}
#[test]
fn permission_requests_map_each_status_without_short_circuiting() {
    struct FakePermissions {
        screen: bool,
        accessibility: Result<bool, String>,
        calls: std::cell::RefCell<Vec<&'static str>>,
    }
    impl super::PermissionRequests for FakePermissions {
        fn request_screen(&self) -> bool {
            self.calls.borrow_mut().push("screen");
            self.screen
        }
        fn request_accessibility(&self) -> Result<bool, String> {
            self.calls.borrow_mut().push("accessibility");
            self.accessibility.clone()
        }
    }
    for screen in [false, true] {
        for accessibility in [Ok(false), Ok(true), Err("Options unavailable".into())] {
            let api = FakePermissions {
                screen,
                accessibility: accessibility.clone(),
                calls: Default::default(),
            };
            assert_eq!(
                super::request_permissions_with(&api),
                accessibility
                    .map(|accessibility| json!({"screen": screen, "accessibility": accessibility}))
            );
            assert_eq!(*api.calls.borrow(), ["screen", "accessibility"]);
        }
    }
}
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
        origin: [0., 0.],
        width: 1440.,
        height: 900.,
        pixel_width: 2880,
        pixel_height: 1800,
    }
}
fn settings() -> Settings {
    Settings {
        mode: "foreground".into(),
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
    if action.get("target").is_none() {
        action["target"] = json!("Document");
    }
    action["grounding_width"] = json!(1920);
    action["grounding_height"] = json!(1080);
    Prediction {
        action,
        reflection: "checked".into(),
        plan_text: "Click an editable document".into(),
        grounding: vec![],
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
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
            0,
            Duration::ZERO
        ),
        Decision::Allow
    );
    assert!(matches!(
        decide(
            &a,
            "BUY now",
            &c,
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
            0,
            Duration::ZERO
        ),
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
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
            0,
            Duration::ZERO
        ),
        Decision::Confirm("Typing into a secure text field")
    );
    assert!(matches!(
        decide(
            &a,
            "",
            &c,
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
            s.max_steps,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
    assert!(matches!(
        decide(
            &a,
            "",
            &c,
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
            0,
            Duration::from_secs(s.max_seconds)
        ),
        Decision::Deny(_)
    ));
    assert!(matches!(
        decide(
            &a,
            "",
            &c,
            &super::policy::Target::default(),
            &s,
            &[],
            0,
            Duration::ZERO
        ),
        Decision::Deny(_)
    ));
    for app in ALWAYS_DENIED {
        s.allowed_apps.push((*app).into());
        assert!(!app_allowed(app, &s, &s.app_ids()));
    }
    s.denied_apps.push(c.app.clone());
    assert!(!app_allowed(&c.app, &s, &s.app_ids()));
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
    instructions: Vec<String>,
    prepare_error: Option<String>,
    ax_handled: bool,
    event_errors: std::cell::RefCell<std::collections::VecDeque<Option<String>>>,
    observation_errors: std::cell::RefCell<std::collections::VecDeque<Option<String>>>,
    native_front: std::cell::Cell<i32>,
    prior_front: std::cell::Cell<Option<i32>>,
    activate_during_action: bool,
    fail_prepare: bool,
    restorations: std::cell::Cell<usize>,
    policy_evidence: Target,
    policy_reads: std::cell::Cell<usize>,
    policy_change: Option<(usize, Target)>,
    contexts: std::cell::RefCell<std::collections::VecDeque<Context>>,
    typing_focus_changes: bool,
    element_label: String,
    element_secure: bool,
    target_changes: bool,
    needs_foreground: bool,
    takeovers: std::cell::Cell<usize>,
    predictions: std::collections::VecDeque<Prediction>,
    steps: Vec<Value>,
    approval: bool,
    approved: usize,
    captures: usize,
    focus: Context,
    change_after_approval: bool,
    approval_delay: Duration,
}

#[test]
fn focus_settle_does_not_hold_the_kill_switch_input_lock() {
    use std::sync::{mpsc, Mutex};
    struct DeferredInput(Mutex<Option<Box<dyn FnOnce() + Send>>>);
    impl Input for DeferredInput {
        fn post(&mut self, _: &Event) -> Result<(), String> {
            Ok(())
        }
        fn end_action(&mut self, _: bool) -> Option<Box<dyn FnOnce() + Send>> {
            self.0.lock().unwrap().take()
        }
    }
    let control = Arc::new(Control::new(FakeInput::default()));
    control.start();
    let (entered, started) = mpsc::channel();
    let (resume, wait) = mpsc::channel();
    control
        .install_input(DeferredInput(Mutex::new(Some(Box::new(move || {
            entered.send(()).unwrap();
            wait.recv().unwrap();
        })))))
        .unwrap();
    let releasing = control.clone();
    let thread = std::thread::spawn(move || releasing.release());
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let stopping = control.clone();
    let (stopped, done) = mpsc::channel();
    let stop_thread = std::thread::spawn(move || {
        stopping.stop();
        stopped.send(()).unwrap();
    });
    let result = done.recv_timeout(Duration::from_millis(100));
    resume.send(()).unwrap(); // Always unblock cleanup, including a failing assertion.
    thread.join().unwrap();
    stop_thread.join().unwrap();
    assert!(result.is_ok(), "focus restoration blocked Stop");
}

#[test]
fn stop_releases_target_input_before_overlay_cleanup_and_rejects_late_work() {
    struct LifecycleInput(Arc<std::sync::Mutex<Vec<&'static str>>>);
    impl Input for LifecycleInput {
        fn post(&mut self, event: &Event) -> Result<(), String> {
            self.0.lock().unwrap().push(match event {
                Event::Key(_, false) => "target key up",
                Event::Mouse { down: false, .. } => "target button up",
                _ => "target down",
            });
            Ok(())
        }
        fn stop(&mut self) {
            self.0.lock().unwrap().push("hide overlay");
        }
    }
    let events = Arc::new(std::sync::Mutex::new(vec![]));
    let control = Control::new(LifecycleInput(events.clone()));
    control.start();
    control
        .post(Event::Key(55, true), &context(), || Ok(context()))
        .unwrap();
    control
        .post(
            Event::Mouse {
                point: [10., 20.],
                button: 0,
                down: true,
                clicks: 1,
            },
            &context(),
            || Ok(context()),
        )
        .unwrap();
    control.stop();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "target down",
            "target down",
            "target key up",
            "target button up",
            "hide overlay"
        ]
    );
    assert!(control.install_input(FakeInput::default()).is_err());
    assert!(control
        .guarded(
            &context(),
            || Ok(context()),
            || -> Result<(), String> { panic!("late AX mutation") }
        )
        .is_err());
}
impl Environment for Fake {
    fn event_context(&self, _: &Event) -> Result<Context, String> {
        if let Some(Some(error)) = self.event_errors.borrow_mut().pop_front() {
            return Err(error);
        }
        self.context()
    }
    fn begin_action(&self) {
        self.prior_front.set(Some(self.native_front.get()));
    }
    fn end_action(&self) {
        if let Some(pid) = super::background::action_restoration(
            self.prior_front.take(),
            Some(self.native_front.get()),
            20,
        ) {
            self.native_front.set(pid);
            self.restorations.set(self.restorations.get() + 1);
        }
    }
    fn policy_target(&self, action: &Action) -> Result<Target, String> {
        self.policy_reads.set(self.policy_reads.get() + 1);
        if let Some((after, target)) = &self.policy_change {
            if self.policy_reads.get() > *after {
                return Ok(target.clone());
            }
        }
        let mut target = self.policy_evidence.clone();
        if (self.typing_focus_changes && matches!(action, Action::Hotkey { .. }))
            || (self.target_changes && self.approved > 0)
        {
            target.label = "Password".into();
        }
        Ok(target)
    }
    fn ground_target(&self, _: &Prediction, _: &mut Action) -> Result<(String, bool), String> {
        Ok((self.element_label.clone(), self.element_secure))
    }
    fn verify_target(&self) -> Result<(), String> {
        if let Some(Some(error)) = self.observation_errors.borrow_mut().pop_front() {
            return Err(error);
        }
        if self.target_changes && self.approved > 0 {
            Err("AX target changed".into())
        } else {
            Ok(())
        }
    }
    fn prepare(
        &self,
        action: &Action,
        _: &Control,
        _: &Context,
    ) -> Result<runner::Preparation, String> {
        if self.activate_during_action {
            self.native_front.set(20);
        }
        if self.fail_prepare {
            return Err("prepare failed after activation".into());
        }
        if let Some(error) = &self.prepare_error {
            return Err(error.clone());
        }
        if self.ax_handled {
            return Ok(runner::Preparation::Handled);
        }
        Ok(
            if self.needs_foreground && matches!(action, Action::Click { .. }) {
                runner::Preparation::NeedsForeground
            } else {
                runner::Preparation::Events
            },
        )
    }
    fn begin_foreground(&self, _: &Control, _: &Context) -> Result<(), String> {
        self.takeovers.set(self.takeovers.get() + 1);
        Ok(())
    }
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
        instruction: &'a str,
        _: &'a Frame,
    ) -> BoxFuture<'a, Result<Prediction, String>> {
        self.instructions.push(instruction.into());
        Box::pin(async { self.predictions.pop_front().ok_or("No fake action".into()) })
    }
    fn context(&self) -> Result<Context, String> {
        Ok(self
            .contexts
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| self.focus.clone()))
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
            tokio::time::sleep(self.approval_delay).await;
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
        if let Some(previous) = self
            .steps
            .last_mut()
            .filter(|s| s["step"] == v["step"] && s["kind"] == v["kind"])
        {
            *previous = v;
        } else {
            self.steps.push(v);
        }
        Ok(())
    }
}
fn fake(actions: Vec<Value>) -> Fake {
    Fake {
        instructions: vec![],
        prepare_error: None,
        ax_handled: false,
        event_errors: Default::default(),
        observation_errors: Default::default(),
        native_front: std::cell::Cell::new(10),
        prior_front: std::cell::Cell::new(None),
        activate_during_action: false,
        fail_prepare: false,
        restorations: std::cell::Cell::new(0),
        policy_evidence: Target::default(),
        policy_reads: std::cell::Cell::new(0),
        policy_change: None,
        contexts: Default::default(),
        typing_focus_changes: false,
        element_label: String::new(),
        element_secure: false,
        target_changes: false,
        needs_foreground: false,
        takeovers: std::cell::Cell::new(0),
        predictions: actions.into_iter().map(prediction).collect(),
        steps: vec![],
        approval: true,
        approved: 0,
        captures: 0,
        focus: context(),
        change_after_approval: false,
        approval_delay: Duration::ZERO,
    }
}

#[tokio::test]
async fn recoverable_errors_have_caps_feedback_and_no_input() {
    let denied = json!({"kind":"switch_app","app":"com.apple.Terminal"});
    let done = json!({"kind":"done","summary":"done"});
    for (actions, recovered, succeeds) in [
        (vec![denied.clone(), done.clone()], 1, true),
        (vec![denied.clone(); 5], 3, false),
        (
            (0..6)
                .flat_map(|_| [denied.clone(), json!({"kind":"wait","seconds":0})])
                .collect(),
            6,
            false,
        ),
    ] {
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let mut env = fake(actions);
        let s = settings();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "task", Instant::now()).await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        assert_eq!(
            env.steps.iter().filter(|s| s["recovered"] == true).count(),
            recovered
        );
        assert!(env.instructions[1].starts_with("Previous action was NOT executed:"));
        assert!(env.instructions[1].contains("Choose a different approach."));
        assert!(input.0.lock().unwrap().is_empty());
        assert_eq!(env.approved, 0);
    }
}

#[tokio::test]
async fn recoverable_vs_fatal_preparation_errors() {
    for (reason, recovered, predictions) in [
        (
            "Action not executed: AX action unsupported; no viable fallback",
            true,
            3,
        ),
        (
            "Action not executed: Background hotkey menu disabled: File > New Folder",
            true,
            3,
        ),
        ("Target binding changed", false, 1),
        ("System Settings pane inspection failed", false, 1),
        (
            "Input already started; Stale observation: changed",
            false,
            1,
        ),
        (
            "AX action completion unknown after timeout; background fallback unavailable",
            false,
            1,
        ),
        ("Stopped", false, 1),
        ("Declined", false, 1),
        ("Timed out waiting for approval", false, 1),
    ] {
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let mut env = fake(vec![json!({"kind":"wait","seconds":0}); 5]);
        env.prepare_error = Some(reason.into());
        let s = settings();
        let error = runner::run(&mut env, &control, &s, &s.app_ids(), "task", Instant::now())
            .await
            .unwrap_err();
        assert_eq!(error, reason);
        assert_eq!(env.instructions.len(), predictions);
        assert_eq!(env.steps[0]["recovered"] == true, recovered);
        assert!(input.0.lock().unwrap().is_empty());
        assert_eq!(env.approved, 0);
    }
}

#[tokio::test]
async fn unparseable_grounding_recovers_but_provider_errors_are_fatal() {
    for (result, recovers) in [("unparseable", true), ("provider_error", false)] {
        let mut env = fake(vec![
            json!({"kind":"fail","reason":"grounding failed"}),
            json!({"kind":"done","summary":"done"}),
        ]);
        env.predictions[0].grounding = vec![json!({"pixel_result":result})];
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let s = settings();
        assert_eq!(
            runner::run(&mut env, &control, &s, &s.app_ids(), "task", Instant::now())
                .await
                .is_ok(),
            recovers
        );
        assert_eq!(env.steps[0]["recovered"] == true, recovers);
        assert!(input.0.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn exhausted_stale_step_recovers_and_ax_menu_restores_focus() {
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let mut env = fake(vec![json!({"kind":"wait","seconds":0}); 3]);
    env.predictions
        .push_back(prediction(json!({"kind":"done","summary":"replanned"})));
    *env.observation_errors.borrow_mut() =
        vec![Some(format!("{}geometry changed", runner::STALE_OBSERVATION)); 3].into();
    env.ax_handled = true;
    env.activate_during_action = true;
    let s = settings();
    assert!(
        runner::run(&mut env, &control, &s, &s.app_ids(), "task", Instant::now())
            .await
            .is_ok()
    );
    assert_eq!(env.steps[0]["recovered"], true);
    assert_eq!(env.steps[0]["staleRetries"], 2);
    assert!(env.instructions[3].starts_with("Previous action was NOT executed:"));
    assert_eq!(env.native_front.get(), 10);
    assert_eq!(env.restorations.get(), 1);
    assert!(input.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn stale_before_first_event_replans_but_partial_input_is_never_replayed() {
    for partial in [false, true] {
        let input = FakeInput::default();
        let events = input.0.clone();
        let control = Control::new(input);
        control.start();
        let mut env = fake(vec![
            json!({"kind":"hotkey","keys":["escape"]}),
            json!({"kind":"done","summary":"fresh"}),
        ]);
        let mut errors = env.event_errors.borrow_mut();
        if partial {
            errors.push_back(None);
        }
        errors.push_back(Some(format!(
            "{}Window validation: drift",
            runner::STALE_OBSERVATION
        )));
        drop(errors);
        let settings = settings();
        let result = runner::run(
            &mut env,
            &control,
            &settings,
            &settings.app_ids(),
            "test",
            Instant::now(),
        )
        .await;
        assert_eq!(result.is_err(), partial);
        assert_eq!(env.steps[0]["staleRetries"], u64::from(!partial));
        if partial {
            assert!(result.unwrap_err().contains("Input already started"));
            assert_eq!(env.predictions.len(), 1);
            assert_eq!(
                *events.lock().unwrap(),
                [Event::Key(53, true), Event::Key(53, false)]
            );
        } else {
            assert!(env.predictions.is_empty());
            assert!(events.lock().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn stale_observations_replan_without_consuming_steps_and_enforce_both_caps() {
    let stale = || {
        Some(format!(
            "{}Window validation: geometry changed; action cancelled",
            runner::STALE_OBSERVATION
        ))
    };
    for (errors, retries, succeeds) in [
        (vec![stale()], vec![1], true),
        (vec![stale(), stale()], vec![2], true),
        (vec![stale(), stale(), stale()], vec![2], false),
        // Each successful step has three pre-input validation calls.
        (
            vec![vec![stale(), stale(), None, None, None]; 3]
                .concat()
                .into_iter()
                .chain([stale()])
                .collect(),
            vec![2, 2, 2, 0],
            false,
        ),
        (
            vec![Some(
                "Window validation: owner before=42 after=43; action cancelled".into(),
            )],
            vec![0],
            false,
        ),
        (
            vec![Some(
                "Window validation: complete matching entries=0; action cancelled".into(),
            )],
            vec![0],
            false,
        ),
    ] {
        let input = FakeInput::default();
        let events = input.0.clone();
        let control = Control::new(input);
        control.start();
        let mut env = fake(vec![json!({"kind":"wait","seconds":0}); 20]);
        for index in 0..2 {
            env.predictions[index] = prediction(
                json!({"kind":"click","x":100,"y":100,"button":"left","clicks":1,"hold_keys":[]}),
            );
        }
        if succeeds {
            env.predictions[retries[0] as usize] =
                prediction(json!({"kind":"done","summary":"fresh"}));
        }
        *env.observation_errors.borrow_mut() = errors.into();
        let mut settings = settings();
        settings.max_steps = retries.len() as u32;
        let result = runner::run(
            &mut env,
            &control,
            &settings,
            &settings.app_ids(),
            "test",
            Instant::now(),
        )
        .await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        assert_eq!(
            env.steps
                .iter()
                .map(|step| step["staleRetries"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            retries
        );
        let predictions = 20 - env.predictions.len();
        assert_eq!(
            predictions,
            retries.iter().sum::<u64>() as usize + retries.len()
        );
        assert_eq!(env.captures, predictions + usize::from(succeeds));
        assert!(
            events.lock().unwrap().is_empty(),
            "discarded predictions must not dispatch input"
        );
    }
}

#[tokio::test]
async fn ax_settle_requires_consecutive_equal_reads_and_bounds_slow_reads() {
    use std::cell::Cell;
    for (readings, expected) in [
        (vec![Ok(("General", 2)), Ok(("General", 2))], 2),
        (
            vec![
                Ok(("General", 2)),
                Ok(("About", 2)),
                Ok(("About", 3)),
                Ok(("About", 3)),
            ],
            4,
        ),
        (
            vec![
                Ok(("About", 3)),
                Err("transient"),
                Ok(("About", 3)),
                Ok(("About", 3)),
            ],
            4,
        ),
    ] {
        let mut readings = readings.into_iter();
        let calls = Cell::new(0);
        runner::settle(|| {
            calls.set(calls.get() + 1);
            std::future::ready(readings.next().unwrap().map_err(str::to_owned))
        })
        .await;
        assert_eq!(calls.get(), expected);
    }
    for slow in [false, true] {
        let calls = Cell::new(0);
        let began = Instant::now();
        runner::settle(|| {
            calls.set(calls.get() + 1);
            let value = calls.get();
            async move {
                if slow {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Ok(value)
            }
        })
        .await;
        assert!(began.elapsed() >= Duration::from_secs(1));
        assert!(began.elapsed() < Duration::from_millis(1300));
        assert!(calls.get() <= 10);
    }
}

#[tokio::test]
async fn post_action_restores_frontmost_with_fake_environment_even_on_prepare_failure() {
    for fails in [false, true] {
        let mut env = fake(vec![json!({"kind":"done","summary":"done"})]);
        env.activate_during_action = true;
        env.fail_prepare = fails;
        let control = Control::new(FakeInput::default());
        control.start();
        let settings = settings();
        let result = runner::run(
            &mut env,
            &control,
            &settings,
            &settings.app_ids(),
            "test",
            Instant::now(),
        )
        .await;
        assert_eq!(result.is_err(), fails);
        assert_eq!(env.native_front.get(), 10);
        assert_eq!(env.restorations.get(), 1);
    }
    assert_eq!(
        super::background::action_restoration(Some(10), Some(30), 20),
        None
    );
    assert_eq!(
        super::background::action_restoration(Some(20), Some(20), 20),
        None
    );
}
#[tokio::test]
async fn fallback_requires_setting_and_fresh_approval_before_any_input() {
    for (enabled, approved, changed) in [
        (false, true, false),
        (true, false, false),
        (true, true, true),
        (true, true, false),
    ] {
        let mut env = fake(vec![
            serde_json::json!({"kind":"click","x":10,"y":10,"button":"left","clicks":1,"hold_keys":[]}),
            serde_json::json!({"kind":"done","summary":"done"}),
        ]);
        env.needs_foreground = true;
        env.approval = approved;
        env.change_after_approval = changed;
        let s = Settings {
            mode: "background".into(),
            allow_foreground_fallback: enabled,
            ..settings()
        };
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now()).await;
        let allowed = enabled && approved && !changed;
        assert_eq!(result.is_ok(), !enabled || allowed);
        if !enabled {
            assert_eq!(env.steps[0]["recovered"], true);
        }
        assert_eq!(env.takeovers.get(), usize::from(allowed));
        assert_eq!(input.0.lock().unwrap().is_empty(), !allowed);
        assert_eq!(env.approved, usize::from(enabled));
        if allowed {
            assert!(env.steps.iter().any(|s| s["foregroundApproval"] == true));
        }
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
    let result = runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
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
        assert!(
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
                .await
                .is_err()
        );
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
        runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
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
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
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
            &super::policy::Target::default(),
            &s,
            &s.app_ids(),
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
        runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
            .await
            .unwrap_err(),
        "Maximum wall time reached"
    );
    assert!(control.stopped());
    control.start();
    env.focus.app = "com.apple.Terminal".into();
    env.captures = 0;
    assert!(
        runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
            .await
            .is_err()
    );
    assert_eq!(env.captures, 0);
    assert!(input.0.lock().unwrap().is_empty());
}

#[test]
fn grounding_mode_defaults_and_pixel_space_maps_to_display_points() {
    let mut s: Settings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(s.grounding_mode, "elements");
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
        origin: [0., 0.],
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
    let error = runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
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

#[tokio::test]
async fn click_policy_uses_target_not_agent_s_plan_or_reflection() {
    for (target, approvals) in [
        ("The = button in Calculator", 0),
        ("Border settings", 0),
        ("Sender name", 0),
        ("Delete account", 1),
        ("Send", 1),
    ] {
        let mut env = fake(vec![
            json!({"kind":"click","target":target,"x":100,"y":100,"button":"left","clicks":1,"hold_keys":[]}),
            json!({"kind":"done","summary":"84"}),
        ]);
        let prediction = env.predictions.front_mut().unwrap();
        prediction.plan_text = "(Previous action verification) The expression 12 × 7 is entered. (Next Action) In order to complete the calculation, click the = button to confirm the result.\n```python\nagent.click(\"The = button in Calculator\")\n```".into();
        prediction.reflection =
            "Confirm the result before proceeding; do not send or delete anything.".into();
        let control = Control::new(FakeInput::default());
        control.start();
        let s = settings();
        runner::run(
            &mut env,
            &control,
            &s,
            &s.app_ids(),
            "12 × 7",
            Instant::now(),
        )
        .await
        .unwrap();
        assert_eq!(env.approved, approvals, "{target}");
        for step in env.steps {
            for timing in ["predictMs", "executeMs", "waitMs"] {
                assert!(step[timing].is_u64(), "{step}");
            }
        }
    }
}

#[test]
fn risky_clicks_match_whole_words_case_insensitively() {
    let click = Action::Click {
        x: 1.,
        y: 1.,
        button: "left".into(),
        clicks: 1,
        hold_keys: vec![],
    };
    let s = settings();
    for word in [
        "submit",
        "send",
        "buy",
        "purchase",
        "pay",
        "checkout",
        "confirm",
        "order",
        "delete",
        "transfer",
        "remove",
        "erase",
        "unsubscribe",
        "publish",
        "post",
    ] {
        for target in [word.to_uppercase(), format!("Button: ({word})!")] {
            assert!(
                matches!(
                    decide(
                        &click,
                        &target,
                        &context(),
                        &super::policy::Target::default(),
                        &s,
                        &s.app_ids(),
                        0,
                        Duration::ZERO
                    ),
                    Decision::Confirm(_)
                ),
                "{target}"
            );
        }
        for target in [
            format!("x{word}"),
            format!("{word}x"),
            format!("_{word}_"),
            format!("1{word}2"),
        ] {
            assert_eq!(
                decide(
                    &click,
                    &target,
                    &context(),
                    &super::policy::Target::default(),
                    &s,
                    &s.app_ids(),
                    0,
                    Duration::ZERO
                ),
                Decision::Allow,
                "{target}"
            );
        }
    }
    let mut p =
        prediction(json!({"kind":"click","x":1,"y":1,"button":"left","clicks":1,"hold_keys":[]}));
    for target in [Value::Null, json!(42), json!("bad\0target")] {
        p.action["target"] = target;
        assert!(p.validated(display()).is_err());
    }
    p.action.as_object_mut().unwrap().remove("target");
    assert!(p.validated(display()).is_err());
}

#[tokio::test]
async fn approval_timeout_logs_reason_target_and_wait_duration_without_input() {
    let mut env = fake(vec![
        json!({"kind":"click","target":"Delete account","x":1,"y":1,"button":"left","clicks":1,"hold_keys":[]}),
    ]);
    env.approval_delay = Duration::from_secs(10);
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = Settings {
        max_seconds: 1,
        ..settings()
    };
    let error = runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
        .await
        .unwrap_err();
    assert_eq!(error, "Timed out waiting for approval: Potential submit, send, purchase or destructive click (Delete account)");
    assert_eq!(env.steps.len(), 1);
    assert_eq!(env.steps[0]["kind"], "approval_wait");
    assert_eq!(env.steps[0]["target"], "Delete account");
    assert_eq!(env.steps[0]["error"], error);
    assert!(env.steps[0]["waitMs"].as_u64().unwrap() >= 900);
    assert_eq!(env.steps[0]["executeMs"], 0);
    assert!(input.0.lock().unwrap().is_empty());
    assert!(control.stopped());
}

#[test]
fn element_target_requires_snapshot_and_strict_index() {
    let mut p =
        prediction(json!({"kind":"click","x":1,"y":1,"button":"left","clicks":1,"hold_keys":[]}));
    p.action["snapshot_id"] = json!(1);
    assert!(p.validated(display()).is_err());
    for index in [json!(-1), json!(true), json!(1.5), json!("1"), json!(250)] {
        p.action["element_index"] = index;
        assert!(p.validated(display()).is_err());
    }
    p.action["element_index"] = json!(0);
    assert_eq!(p.element_target().unwrap(), Some((1, 0)));
    assert!(p.validated(display()).is_ok());
}

#[tokio::test]
async fn grounding_diagnostics_and_index_zero_survive_approval_and_final_progress() {
    let mut env = fake(vec![
        json!({"kind":"click","x":1,"y":1,"button":"left","clicks":1,"hold_keys":[],"snapshot_id":4,"element_index":0,"target":"Buy"}),
        json!({"kind":"done","summary":"done"}),
    ]);
    let diagnostics = json!({"configured_mode":"elements","mode":"elements","usable_elements":6,"snapshot_id":4,"model_result":"index","raw_reply":"0","element_index":0,"pixel_fallback_reason":null});
    env.predictions[0].grounding = vec![diagnostics.clone()];
    let control = Control::new(FakeInput::default());
    control.start();
    let s = settings();
    runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
        .await
        .unwrap();
    assert_eq!(env.approved, 1);
    let entries: Vec<_> = env.steps.iter().filter(|s| s["step"] == 1).collect();
    assert!(entries.len() >= 2);
    for step in entries {
        assert_eq!(step["action"]["element_index"], 0);
        assert_eq!(step["action"]["snapshot_id"], 4);
        assert_eq!(step["grounding"], json!([diagnostics]));
    }
}

#[tokio::test]
async fn element_label_policy_cannot_be_bypassed_by_benign_description() {
    let mut env = fake(vec![
        json!({"kind":"click","x":1,"y":1,"button":"left","clicks":1,"hold_keys":[],"target":"ordinary button"}),
    ]);
    env.element_label = "AXButton Buy now".into();
    env.approval = false;
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = settings();
    assert!(
        runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
            .await
            .is_err()
    );
    assert_eq!(env.approved, 1);
    assert!(input.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn changed_element_after_approval_cancels_input() {
    let mut env = fake(vec![
        json!({"kind":"type","text":"secret","overwrite":true,"enter":false}),
    ]);
    env.element_secure = true;
    env.target_changes = true;
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = settings();
    assert!(
        runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
            .await
            .is_err()
    );
    assert_eq!(env.approved, 1);
    assert!(env
        .steps
        .iter()
        .any(|step| step["reason"] == "Typing into a secure text field"));
    assert!(input.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn typing_policy_runs_before_events_and_rechecks_the_focused_field() {
    for (mode, access, changed, approvals, succeeds) in [
        (ConfirmTyping::Sensitive, Access::Full, false, 0, true),
        (ConfirmTyping::Always, Access::Full, false, 1, true),
        (ConfirmTyping::Sensitive, Access::ReadOnly, false, 0, true),
        (ConfirmTyping::Sensitive, Access::Full, true, 0, false),
    ] {
        let mut env = fake(vec![
            json!({"kind":"type","text":"hello","overwrite":false,"enter":false}),
            json!({"kind":"done","summary":"done"}),
        ]);
        env.policy_evidence = Target {
            role: "AXTextArea".into(),
            label: "Document".into(),
            ..Default::default()
        };
        env.typing_focus_changes = changed;
        let mut s = settings();
        s.confirm_typing = mode;
        s.allowed_apps[0].access = access;
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now()).await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        assert_eq!(env.approved, approvals);
        assert_eq!(
            input
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, Event::Text(_)))
                .count(),
            if succeeds && access != Access::ReadOnly {
                5
            } else {
                0
            }
        );
    }
}
#[tokio::test]
async fn changed_sensitive_metadata_after_approval_cancels_before_input() {
    let mut env = fake(vec![
        json!({"kind":"type","text":"hello","overwrite":false,"enter":false}),
    ]);
    env.policy_evidence = Target {
        role: "AXTextArea".into(),
        label: "Document".into(),
        ..Default::default()
    };
    env.target_changes = true;
    let mut s = settings();
    s.confirm_typing = ConfirmTyping::Always;
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let error = runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
        .await
        .unwrap_err();
    assert!(error.contains("AX policy target changed"));
    assert_eq!(env.approved, 1);
    assert!(input.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn secure_samples_change_confirmation_without_cancelling_planning() {
    for (samples, approvals) in [
        (vec![false, true, false], 1),
        (vec![true, false, false], 0),
        (vec![false, false, true, false], 1),
    ] {
        let mut env = fake(vec![
            json!({"kind":"type","text":"hello","overwrite":false,"enter":false}),
            json!({"kind":"done","summary":"done"}),
        ]);
        env.policy_evidence.role = "AXTextArea".into();
        *env.contexts.borrow_mut() = samples
            .into_iter()
            .map(|secure| Context {
                secure,
                ..context()
            })
            .collect();
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let mut s = settings();
        s.mode = "background".into();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now()).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(env.approved, approvals);
        assert_eq!(
            input
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| matches!(e, Event::Text(_)))
                .count(),
            5
        );
        if approvals > 0 {
            assert!(env
                .steps
                .iter()
                .any(|step| step["reason"] == "Typing into a secure text field"));
        }
    }
}

#[tokio::test]
async fn planning_app_change_cancels_with_binding_diagnostics() {
    let mut env = fake(vec![
        json!({"kind":"type","text":"private text","overwrite":false,"enter":false}),
    ]);
    *env.contexts.borrow_mut() = [
        context(),
        Context {
            app: "com.apple.Finder".into(),
            secure: false,
        },
    ]
    .into();
    let input = FakeInput::default();
    let control = Control::new(input.clone());
    control.start();
    let s = settings();
    let error = runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now())
        .await
        .unwrap_err();
    assert!(error.contains("app before=com.apple.TextEdit after=com.apple.Finder"));
    assert!(!error.contains("private text"));
    assert!(input.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn grounded_typing_rechecks_selected_metadata_instead_of_focus() {
    for changed in [None, Some("role"), Some("label"), Some("secure")] {
        let mut env = fake(vec![
            json!({"kind":"type","text":"hello","overwrite":false,"enter":false,"snapshot_id":1,"element_index":0}),
            json!({"kind":"done","summary":"done"}),
        ]);
        env.policy_evidence.role = "AXTextArea".into();
        env.policy_evidence.label = "Document".into();
        env.typing_focus_changes = true; // Focused field differs from the grounded field.
        if let Some(field) = changed {
            let mut target = env.policy_evidence.clone();
            match field {
                "role" => target.role = "AXTextField".into(),
                "label" => target.label = "private label".into(),
                "secure" => target.secure = true,
                _ => unreachable!(),
            }
            env.policy_change = Some((3, target)); // Immediately before the first input.
        }
        let input = FakeInput::default();
        let control = Control::new(input.clone());
        control.start();
        let s = settings();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now()).await;
        if changed.is_some() {
            let error = result.unwrap_err();
            assert!(
                error.contains("Typing field changed before input"),
                "{error}"
            );
            assert!(!error.contains("private label"));
            assert!(input.0.lock().unwrap().is_empty());
        } else {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(
                input
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|e| matches!(e, Event::Text(_)))
                    .count(),
                5
            );
        }
    }
}

#[tokio::test]
async fn grounded_secure_and_sensitive_fields_still_require_confirmation() {
    for secure in [false, true] {
        let mut env = fake(vec![
            json!({"kind":"type","text":"hello","overwrite":false,"enter":false,"snapshot_id":1,"element_index":0}),
            json!({"kind":"done","summary":"done"}),
        ]);
        env.policy_evidence.role = "AXTextField".into();
        env.policy_evidence.secure = secure;
        env.policy_evidence.label = if secure { "Document" } else { "Password" }.into();
        env.typing_focus_changes = true;
        let control = Control::new(FakeInput::default());
        control.start();
        let s = settings();
        let result =
            runner::run(&mut env, &control, &s, &s.app_ids(), "test", Instant::now()).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(env.approved, 1);
    }
}
