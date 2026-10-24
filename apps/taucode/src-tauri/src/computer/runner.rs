use super::{
    control::{Control, Event},
    policy::*,
};
use crate::shared::providers::BoxFuture;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub struct Frame {
    pub png: String,
    pub thumbnail: String,
    pub display: Display,
}
#[derive(Debug, PartialEq)]
pub enum Preparation {
    Events,
    Handled,
    NeedsForeground,
}
pub trait Environment {
    fn capture(&mut self) -> BoxFuture<'_, Result<Frame, String>>;
    fn predict<'a>(
        &'a mut self,
        instruction: &'a str,
        frame: &'a Frame,
    ) -> BoxFuture<'a, Result<Prediction, String>>;
    fn context(&self) -> Result<Context, String>;
    fn grounding_diagnostics(&self) -> Vec<Value> {
        vec![]
    }
    fn ground_target(
        &self,
        prediction: &Prediction,
        _: &mut Action,
    ) -> Result<(String, bool), String> {
        if prediction.element_target()?.is_some() {
            return Err("Element snapshots unavailable".into());
        }
        Ok((String::new(), false))
    }
    fn policy_target(&self, _: &Action) -> Result<Target, String> {
        Ok(Target::default())
    }
    fn verify_target(&self) -> Result<(), String> {
        Ok(())
    }
    fn prepare(&self, _: &Action, _: &Control, _: &Context) -> Result<Preparation, String> {
        Ok(Preparation::Events)
    }
    fn begin_foreground(&self, _: &Control, _: &Context) -> Result<(), String> {
        Err("Foreground fallback unavailable".into())
    }
    fn end_action(&self) {}
    fn capture_description(&self) -> String {
        "Current frontmost application".into()
    }
    fn planner_error(&self, error: String) -> String {
        error
    }
    fn approve<'a>(
        &'a mut self,
        reason: &'a str,
        action: &'a Action,
        context: &'a Context,
    ) -> BoxFuture<'a, Result<(), String>>;
    fn target(&self, app: &str) -> Result<String, String> {
        Ok(app.into())
    }
    fn event_context(&self, _: &Event) -> Result<Context, String> {
        self.context()
    }
    fn open(&self, app: &str, target: &str) -> Result<(), String>;
    fn progress(&mut self, step: Value) -> Result<(), String>;
}
fn post(
    env: &impl Environment,
    control: &Control,
    c: &Context,
    epoch: u64,
    event: Event,
) -> Result<(), String> {
    if control.epoch() != epoch {
        return Err("Action interrupted by Pause".into());
    }
    let current = env.event_context(&event)?;
    control.post(event, c, || Ok(current))
}
fn verify_app(check: &str, before: &Context, after: &Context) -> Result<(), String> {
    if before.app != after.app {
        return Err(format!(
            "{check}; app before={} after={}; action cancelled",
            before.app, after.app
        ));
    }
    Ok(())
}
fn verify_policy_target(check: &str, before: &Target, after: &Target) -> Result<(), String> {
    if before != after {
        // Never log labels, pane titles, field values, or typed text.
        return Err(format!("{check}; role before={} after={}, secure before={} after={}, label_changed={}, pane_changed={}; action cancelled",
            before.role, after.role, before.secure, after.secure,
            before.label != after.label,
            before.window_title != after.window_title || before.selected_rows != after.selected_rows));
    }
    Ok(())
}
async fn execute(
    env: &impl Environment,
    control: &Control,
    c: &Context,
    action: &Action,
    target: Option<&str>,
    policy_target: &Target,
    grounded: bool,
) -> Result<(), String> {
    let epoch = control.epoch();
    let verify_typing = || -> Result<(), String> {
        if matches!(action, Action::Type { .. }) {
            env.verify_target()?;
            let focused = Action::Hotkey { keys: vec![] };
            let current = env.policy_target(if grounded { action } else { &focused })?;
            verify_policy_target("Typing field changed before input", policy_target, &current)?;
        }
        Ok(())
    };
    let key = |name: &str, down| {
        if down {
            verify_typing()?;
        }
        post(
            env,
            control,
            c,
            epoch,
            Event::Key(keycode(name).ok_or("Unknown key")?, down),
        )
    };
    let click = |point, button, clicks| -> Result<(), String> {
        post(
            env,
            control,
            c,
            epoch,
            Event::Mouse {
                point,
                button,
                down: true,
                clicks,
            },
        )?;
        post(
            env,
            control,
            c,
            epoch,
            Event::Mouse {
                point,
                button,
                down: false,
                clicks,
            },
        )
    };
    let result = async {
        match action {
            Action::Click {
                x,
                y,
                button,
                clicks,
                hold_keys,
            } => {
                for k in hold_keys {
                    key(k, true)?;
                }
                for n in 1..=*clicks {
                    click(
                        [*x, *y],
                        match button.as_str() {
                            "right" => 1,
                            "middle" => 2,
                            _ => 0,
                        },
                        n,
                    )?;
                }
            }
            Action::Type {
                text,
                x,
                y,
                overwrite,
                enter,
            } => {
                if let (Some(x), Some(y)) = (x, y) {
                    click([*x, *y], 0, 1)?;
                }
                // Field focus can change after the click; the exact approved context is rechecked.
                if *overwrite {
                    key("command", true)?;
                    key("a", true)?;
                    key("a", false)?;
                    key("command", false)?;
                    key("backspace", true)?;
                    key("backspace", false)?;
                }
                for ch in text.chars() {
                    tokio::task::yield_now().await;
                    control.checkpoint().await?;
                    verify_typing()?;
                    post(env, control, c, epoch, Event::Text(ch.to_string()))?;
                }
                if *enter {
                    key("enter", true)?;
                    key("enter", false)?;
                }
            }
            Action::Scroll {
                x,
                y,
                clicks,
                shift,
            } => {
                post(env, control, c, epoch, Event::Move([*x, *y], false))?;
                post(env, control, c, epoch, Event::Scroll(*clicks, *shift))?;
            }
            Action::Drag {
                from,
                to,
                hold_keys,
            } => {
                for k in hold_keys {
                    key(k, true)?;
                }
                post(
                    env,
                    control,
                    c,
                    epoch,
                    Event::Mouse {
                        point: *from,
                        button: 0,
                        down: true,
                        clicks: 1,
                    },
                )?;
                for n in 1..=20 {
                    control.checkpoint().await?;
                    let f = n as f64 / 20.;
                    post(
                        env,
                        control,
                        c,
                        epoch,
                        Event::Move(
                            [
                                from[0] + (to[0] - from[0]) * f,
                                from[1] + (to[1] - from[1]) * f,
                            ],
                            true,
                        ),
                    )?;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                post(
                    env,
                    control,
                    c,
                    epoch,
                    Event::Mouse {
                        point: *to,
                        button: 0,
                        down: false,
                        clicks: 1,
                    },
                )?;
            }
            Action::Hotkey { keys } => {
                for k in keys {
                    key(k, true)?;
                }
                for k in keys.iter().rev() {
                    key(k, false)?;
                }
            }
            Action::HoldAndPress { hold, press } => {
                for k in hold {
                    key(k, true)?;
                }
                for k in press {
                    key(k, true)?;
                    key(k, false)?;
                }
            }
            Action::Open { app_or_file } => {
                control.checkpoint().await?;
                control.guarded(
                    c,
                    || env.context(),
                    || env.open(app_or_file, target.ok_or("Missing target app")?),
                )?;
            }
            Action::SwitchApp { app } => {
                control.checkpoint().await?;
                control.guarded(
                    c,
                    || env.context(),
                    || env.open(app, target.ok_or("Missing target app")?),
                )?;
            }
            Action::Wait { seconds } => {
                control
                    .until(async {
                        tokio::time::sleep(Duration::from_secs_f64(*seconds)).await;
                        Ok(())
                    })
                    .await?
            }
            Action::Done { .. } | Action::Fail { .. } => (),
        }
        if matches!(action, Action::Open { .. } | Action::SwitchApp { .. }) {
            control
                .until(async {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    Ok(())
                })
                .await?;
        }
        Ok(())
    }
    .await;
    control.release();
    result
}
pub async fn run(
    env: &mut impl Environment,
    control: &Control,
    settings: &Settings,
    apps: &[String],
    instruction: &str,
    start: Instant,
) -> Result<Value, String> {
    let mut pending: Option<Value> = None;
    let mut phase = "predictMs";
    let mut phase_start = Instant::now();
    let work = async {
        for step in 0..settings.max_steps {
            control.checkpoint().await?;
            let context = env
                .context()
                .map_err(|e| format!("Pre-flight context: {e}"))?;
            if (settings.mode == "background" || context.app != "dev.taucode.desktop")
                && !app_allowed(&context.app, settings, apps)
            {
                return Err(format!("App blocked: {}", context.app));
            }
            let frame = control
                .until(env.capture())
                .await
                .map_err(|e| format!("Pre-flight capture: app={}, {e}", context.app))?;
            let scoped = format!("{instruction}\n\nHost policy: allowed application bundle IDs: {}. {}: {}. {} Never interact with taucode itself.", apps.join(", "), env.capture_description(), context.app, if settings.mode == "background" { "This run is bound to this app. Coordinates refer only to its captured window. Do not switch to another app." } else { "If taucode is frontmost, first use switch_applications with an allowed bundle ID." });
            phase = "predictMs";
            phase_start = Instant::now();
            pending = Some(
                json!({"step":step+1,"thumbnail":frame.thumbnail,"action":{"kind":"predict"},"reflection":"","predictMs":0,"executeMs":0,"waitMs":0}),
            );
            let prediction = match control.until(env.predict(&scoped, &frame)).await {
                Ok(prediction) => prediction,
                Err(error) => {
                    let mut entry = pending.take().unwrap();
                    entry["predictMs"] = json!(phase_start.elapsed().as_millis() as u64);
                    entry["action"] = json!({"kind":"fail","reason":error});
                    entry["error"] = json!(error);
                    entry["grounding"] = json!(env.grounding_diagnostics());
                    env.progress(entry)?;
                    return Err(error);
                }
            };
            pending.as_mut().unwrap()["grounding"] = json!(prediction.grounding);
            let predict_ms = phase_start.elapsed().as_millis() as u64;
            let mut action = prediction
                .validated(frame.display)
                .map_err(|e| env.planner_error(e))?;
            let (element_label, secure) = env.ground_target(&prediction, &mut action)?;
            let description = format!(
                "{} {}",
                prediction.action["target"].as_str().unwrap_or(""),
                element_label
            )
            .trim()
            .to_owned();
            pending = Some(
                json!({"step":step+1,"thumbnail":frame.thumbnail,"action":action,"target":description,"reflection":prediction.reflection,"planText":prediction.plan_text,"grounding":prediction.grounding,"predictMs":predict_ms,"executeMs":0,"waitMs":0}),
            );
            if let Some((id, index)) = prediction.element_target()? {
                pending.as_mut().unwrap()["action"]["snapshot_id"] = json!(id);
                pending.as_mut().unwrap()["action"]["element_index"] = json!(index);
            }
            phase = "waitMs";
            phase_start = Instant::now();
            control.checkpoint().await?;
            let target = match &action {
                Action::Open { app_or_file } => Some(env.target(app_or_file)?),
                Action::SwitchApp { app } => Some(env.target(app)?),
                _ => None,
            };
            if let (Action::Open { app_or_file }, Some(app)) = (&action, &target) {
                if app_or_file != app && access(app, settings) == Access::ReadOnly {
                    return Err(
                        "App has read-only access: open the app by bundle ID, not a file".into(),
                    );
                }
            }
            let policy_action = match &target {
                Some(app) => Action::SwitchApp { app: app.clone() },
                None => action.clone(),
            };
            let current = env.context()?;
            verify_app("Target binding changed while planning", &context, &current)?;
            let mut policy_context = Context {
                secure: current.secure || secure,
                ..current.clone()
            };
            env.verify_target()?;
            let policy_target = env.policy_target(&action)?;
            let after = loop {
                match decide(
                    &policy_action,
                    &description,
                    &policy_context,
                    &policy_target,
                    settings,
                    apps,
                    step,
                    start.elapsed(),
                ) {
                    Decision::Deny(reason) => return Err(reason),
                    Decision::Confirm(reason) => {
                        let entry = pending.as_mut().unwrap();
                        entry["kind"] = json!("approval_wait");
                        entry["reason"] = json!(reason);
                        env.progress(entry.clone())?;
                        let approved = control.until(env.approve(reason, &action, &current)).await;
                        let entry = pending.as_mut().unwrap();
                        entry["waitMs"] = json!(phase_start.elapsed().as_millis() as u64);
                        entry["error"] = json!(approved.as_ref().err());
                        env.progress(entry.clone())?;
                        approved?;
                        entry.as_object_mut().unwrap().remove("kind");
                        entry.as_object_mut().unwrap().remove("reason");
                    }
                    Decision::Allow => (),
                }
                control.checkpoint().await?;
                let after = env.context()?;
                verify_app("Target binding changed after approval", &current, &after)?;
                verify_policy_target(
                    "AX policy target changed after approval",
                    &policy_target,
                    &env.policy_target(&action)?,
                )?;
                // A new fail-closed sample adds a confirmation, never a focus cancellation.
                if after.secure && !policy_context.secure && matches!(action, Action::Type { .. }) {
                    policy_context.secure = true;
                    continue;
                }
                if let Decision::Deny(reason) = decide(
                    &policy_action,
                    &description,
                    &after,
                    &policy_target,
                    settings,
                    apps,
                    step,
                    start.elapsed(),
                ) {
                    return Err(reason);
                }
                break after;
            };
            pending.as_mut().unwrap()["waitMs"] = json!(phase_start.elapsed().as_millis() as u64);
            phase = if matches!(action, Action::Wait { .. }) {
                "waitMs"
            } else {
                "executeMs"
            };
            phase_start = Instant::now();
            env.verify_target()?;
            let preparation = env.prepare(&action, control, &after)?;
            if preparation == Preparation::NeedsForeground {
                if !settings.allow_foreground_fallback {
                    return Err(
                        "Background input unavailable; foreground fallback is disabled".into(),
                    );
                }
                let reason =
                    "This app ignored background input; take over the mouse for this step?";
                phase = "waitMs";
                phase_start = Instant::now();
                let entry = pending.as_mut().unwrap();
                entry["kind"] = json!("approval_wait");
                entry["reason"] = json!(reason);
                env.progress(entry.clone())?;
                control.until(env.approve(reason, &action, &after)).await?;
                control.checkpoint().await?;
                verify_app(
                    "Target changed after fallback approval",
                    &after,
                    &env.context()?,
                )?;
                if let Decision::Deny(reason) = decide(
                    &policy_action,
                    &description,
                    &after,
                    &policy_target,
                    settings,
                    apps,
                    step,
                    start.elapsed(),
                ) {
                    return Err(reason);
                }
                env.verify_target()?;
                verify_policy_target(
                    "AX policy target changed after fallback approval",
                    &policy_target,
                    &env.policy_target(&action)?,
                )?;
                env.begin_foreground(control, &after)?;
                entry["waitMs"] = json!(
                    entry["waitMs"].as_u64().unwrap_or(0)
                        + phase_start.elapsed().as_millis() as u64
                );
                entry["foregroundApproval"] = json!(true);
                entry.as_object_mut().unwrap().remove("kind");
                phase = "executeMs";
                phase_start = Instant::now();
                // Activation is asynchronous; no input until the exact app regains focus.
                control
                    .until(async {
                        tokio::time::sleep(Duration::from_millis(150)).await;
                        Ok(())
                    })
                    .await?;
            }
            if preparation != Preparation::Handled {
                verify_policy_target(
                    "AX policy target changed before input",
                    &policy_target,
                    &env.policy_target(&action)?,
                )?;
            }
            let outcome = if preparation == Preparation::Handled {
                Ok(())
            } else {
                execute(
                    env,
                    control,
                    &after,
                    &action,
                    target.as_deref(),
                    &policy_target,
                    prediction.element_target()?.is_some(),
                )
                .await
            };
            env.end_action();
            let outcome = outcome.and_then(|()| {
                if let Action::Fail { reason } = &action {
                    Err(env.planner_error(reason.clone()))
                } else {
                    Ok(())
                }
            });
            let mut entry = pending.take().unwrap();
            entry[phase] = json!(
                entry[phase].as_u64().unwrap_or(0) + phase_start.elapsed().as_millis() as u64
            );
            entry["error"] = json!(outcome.as_ref().err());
            env.progress(entry)?;
            if outcome
                .as_ref()
                .err()
                .is_some_and(|e| e == "Action interrupted by Pause")
            {
                control.checkpoint().await?;
                continue;
            }
            outcome?;
            match action {
                Action::Done { summary } => {
                    let last = control.until(env.capture()).await?;
                    return Ok(json!({"summary":summary,"screenshot":last.png}));
                }
                Action::Fail { reason } => return Err(reason),
                _ => (),
            }
        }
        Err("Maximum steps reached".into())
    };
    let result = tokio::time::timeout(
        Duration::from_secs(settings.max_seconds).saturating_sub(start.elapsed()),
        work,
    )
    .await
    .map_err(|_| {
        if let Some(entry) = pending.as_ref().filter(|s| s["kind"] == "approval_wait") {
            format!(
                "Timed out waiting for approval: {} ({})",
                entry["reason"].as_str().unwrap_or(""),
                entry["target"].as_str().unwrap_or("")
            )
        } else if phase == "predictMs" && pending.is_some() {
            env.planner_error("Maximum wall time reached".into())
        } else {
            "Maximum wall time reached".into()
        }
    })
    .and_then(|r| r);
    control.stop();
    if let (Err(error), Some(mut entry)) = (&result, pending) {
        entry[phase] = json!(phase_start.elapsed().as_millis() as u64);
        entry["error"] = json!(error);
        env.progress(entry)?;
    }
    result
}

pub fn failure_step(step: u32, thumbnail: &str, error: &str) -> Value {
    json!({"step":step,"thumbnail":thumbnail,"action":{"kind":"fail","reason":error},"reflection":"","error":error,"predictMs":0,"executeMs":0,"waitMs":0})
}
