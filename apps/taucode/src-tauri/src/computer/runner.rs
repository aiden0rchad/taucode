use super::{
    control::{Control, Event},
    policy::*,
};
use crate::shared::providers::BoxFuture;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const STALE_OBSERVATION: &str = "Stale observation: ";
pub const RECOVERABLE: &str = "Action not executed: ";
pub const UNCERTAIN: &str = "Action outcome uncertain: ";

#[derive(Default)]
struct Recovery {
    consecutive: usize,
    total: usize,
    previous: Option<String>,
}
impl Recovery {
    fn record(&mut self, reason: &str) -> Result<(), String> {
        self.consecutive += 1;
        self.total += 1;
        self.previous = Some(format!(
            "Previous action was NOT executed: {reason}. Choose a different approach.\n\n"
        ));
        if self.consecutive >= 3 || self.total >= 6 {
            Err(reason.into())
        } else {
            Ok(())
        }
    }
}

pub async fn settle<T: PartialEq, F: std::future::Future<Output = Result<T, String>>>(
    mut read: impl FnMut() -> F,
) {
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        let mut previous = None;
        loop {
            let current = read().await.ok();
            if current.is_some() && current == previous {
                break;
            }
            previous = current;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
}

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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Observing,
    Planning,
    Acting,
    WaitingForApproval,
    Settling,
}
pub trait Environment {
    fn phase(&mut self, _: Phase) {}
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
    /// Checked after an action's last event, before key focus is handed back.
    fn input_finished(&self) -> Result<(), String> {
        Ok(())
    }
    fn prepare(&self, _: &Action, _: &Control, _: &Context) -> Result<Preparation, String> {
        Ok(Preparation::Events)
    }
    fn begin_foreground(&self, _: &Control, _: &Context) -> Result<(), String> {
        Err("Foreground fallback unavailable".into())
    }
    fn begin_action(&self) {}
    fn end_action(&self) {}
    fn settle(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
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
fn verify_app(check: &str, before: &Context, after: &Context) -> Result<(), String> {
    if before.app != after.app {
        return Err(format!(
            "{check}; app before={} after={}; action cancelled",
            before.app, after.app
        ));
    }
    Ok(())
}
/// Done, Fail and Wait send no input, so they have no input target to resolve or verify.
fn input_target(env: &impl Environment, action: &Action) -> Result<Target, String> {
    if matches!(action, Action::Done { .. } | Action::Fail { .. } | Action::Wait { .. }) {
        Ok(Target::default())
    } else {
        env.policy_target(action)
    }
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
    let input_started = std::sync::atomic::AtomicBool::new(false);
    let post = |event: Event| {
        if control.epoch() != epoch {
            return Err("Action interrupted by Pause".into());
        }
        let current = env.event_context(&event)?;
        input_started.store(true, std::sync::atomic::Ordering::Relaxed);
        control.post(event, c, || Ok(current))
    };
    let verify_typing = || -> Result<(), String> {
        if matches!(action, Action::Type { .. }) {
            env.verify_target()?;
            let focused = Action::Hotkey { keys: vec![] };
            let current = env.policy_target(if grounded { action } else { &focused })?;
            // A targeted field keeps its identity (checked separately) when its label changes, as
            // a search field's does when its placeholder goes away on focus. Policy still holds
            // unless the new label is a password or similar field.
            let relabelled = grounded
                && Target { label: current.label.clone(), ..policy_target.clone() } == current
                && !super::policy::sensitive_field(&current.label);
            if !relabelled {
                verify_policy_target("Typing field changed before input", policy_target, &current)?;
            }
        }
        Ok(())
    };
    let key = |name: &str, down| {
        let code = keycode(name).ok_or("Unknown key")?;
        if down {
            verify_typing()?;
            post(Event::Key(code, true))
        } else {
            control.release_key(code)
        }
    };
    let click = |point, button, clicks| -> Result<(), String> {
        post(Event::Mouse {
            point,
            button,
            down: true,
            clicks,
        })?;
        post(Event::Mouse {
            point,
            button,
            down: false,
            clicks,
        })
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
                    // Let the app move focus to the clicked field before keystrokes are checked.
                    tokio::time::sleep(Duration::from_millis(120)).await;
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
                    post(Event::Text(ch.to_string()))?;
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
                post(Event::Move([*x, *y], false))?;
                post(Event::Scroll(*clicks, *shift))?;
            }
            Action::Drag {
                from,
                to,
                hold_keys,
            } => {
                for k in hold_keys {
                    key(k, true)?;
                }
                post(Event::Mouse {
                    point: *from,
                    button: 0,
                    down: true,
                    clicks: 1,
                })?;
                for n in 1..=20 {
                    control.checkpoint().await?;
                    let f = n as f64 / 20.;
                    post(Event::Move(
                        [
                            from[0] + (to[0] - from[0]) * f,
                            from[1] + (to[1] - from[1]) * f,
                        ],
                        true,
                    ))?;
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                post(Event::Mouse {
                    point: *to,
                    button: 0,
                    down: false,
                    clicks: 1,
                })?;
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
    .await
    // Last chance to notice the user typing while key focus is still redirected.
    .and_then(|()| env.input_finished());
    control.release();
    result.map_err(|error: String| {
        if input_started.load(std::sync::atomic::Ordering::Relaxed)
            && error.starts_with(STALE_OBSERVATION)
        {
            format!("Input already started; {error}")
        } else {
            error
        }
    })
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
        let mut step = 0;
        let mut stale_retries = 0;
        let mut run_stale_retries = 0;
        let mut recovery = Recovery::default();
        let mut verify_only: Option<String> = None;
        'steps: while step < settings.max_steps {
            macro_rules! recover {
                ($reason:expr) => {{
                    let reason: String = $reason;
                    control.checkpoint().await?;
                    control.release();
                    env.end_action();
                    if let Some(mut entry) = pending.take() {
                        entry[phase] = json!(phase_start.elapsed().as_millis() as u64);
                        entry["recovered"] = json!(true);
                        entry["error"] = json!(reason);
                        entry["reason"] = json!(reason);
                        env.progress(entry)?;
                    }
                    recovery.record(&reason)?;
                    step += 1;
                    stale_retries = 0;
                    continue 'steps;
                }};
            }
            // Only pre-input checks may discard a prediction. Never replay partial input.
            macro_rules! observation {
                ($result:expr) => {
                    match $result {
                        Ok(value) => value,
                        Err(error)
                            if error.starts_with(STALE_OBSERVATION)
                                && stale_retries < 2
                                && run_stale_retries < 6 =>
                        {
                            stale_retries += 1;
                            run_stale_retries += 1;
                            recovery.previous = Some(format!(
                                "Previous action was NOT executed: {error}. Re-plan from the fresh observation.\n\n"
                            ));
                            if let Some(entry) = pending.as_mut() {
                                entry["staleRetries"] = json!(stale_retries);
                            }
                            control.release();
                            env.end_action();
                            continue 'steps;
                        }
                        Err(error)
                            if error.starts_with(STALE_OBSERVATION)
                                || error.starts_with(RECOVERABLE) =>
                        {
                            recover!(error)
                        }
                        Err(error) if error.starts_with(UNCERTAIN) => {
                            control.release();
                            env.end_action();
                            control.checkpoint().await?;
                            if let Some(mut entry) = pending.take() {
                                entry[phase] = json!(phase_start.elapsed().as_millis() as u64);
                                entry["error"] = json!(error);
                                env.progress(entry)?;
                            }
                            recovery.previous = Some(format!(
                                "READ-ONLY VERIFICATION: {error} The previous action may have executed. Inspect this fresh observation. Return done ONLY if the entire requested task is visibly complete; otherwise return fail explaining what remains or is uncertain. No click, edit, hotkey, retry, or other input is allowed.\n\n"
                            ));
                            verify_only = Some(error);
                            step += 1;
                            env.phase(Phase::Settling);
                            control.until(env.settle()).await?;
                            continue 'steps;
                        }
                        Err(error) => return Err(error),
                    }
                };
            }
            control.checkpoint().await?;
            let observation_epoch = control.epoch();
            let verify_observation = || {
                if control.epoch() == observation_epoch {
                    Ok(())
                } else {
                    Err(format!(
                        "{STALE_OBSERVATION}Pause invalidated the planned action"
                    ))
                }
            };
            let context = env
                .context()
                .map_err(|e| format!("Pre-flight context: {e}"))?;
            if (settings.mode == "background" || context.app != "dev.taucode.desktop")
                && !app_allowed(&context.app, settings, apps)
            {
                return Err(format!("App blocked: {}", context.app));
            }
            env.phase(Phase::Observing);
            let frame = control
                .until(env.capture())
                .await
                .map_err(|e| format!("Pre-flight capture: app={}, {e}", context.app))?;
            control.checkpoint().await?;
            observation!(verify_observation());
            let scoped = format!("{instruction}\n\nHost policy: allowed application bundle IDs: {}. {}: {}. {} Never interact with taucode itself.", apps.join(", "), env.capture_description(), context.app, if settings.mode == "background" { "This run is bound to this app. Coordinates refer only to its captured window. Do not switch to another app." } else { "If taucode is frontmost, first use switch_applications with an allowed bundle ID." });
            let scoped = format!("{}{scoped}", recovery.previous.as_deref().unwrap_or(""));
            phase = "predictMs";
            phase_start = Instant::now();
            pending = Some(
                json!({"step":step+1,"staleRetries":stale_retries,"thumbnail":frame.thumbnail,"action":{"kind":"predict"},"reflection":"","predictMs":0,"executeMs":0,"waitMs":0}),
            );
            env.phase(Phase::Planning);
            let prediction = match control.until(env.predict(&scoped, &frame)).await {
                Ok(prediction) => prediction,
                Err(error) => {
                    let diagnostics = env.grounding_diagnostics();
                    if diagnostics
                        .iter()
                        .any(|d| d["pixel_result"] == "unparseable")
                    {
                        pending.as_mut().unwrap()["grounding"] = json!(diagnostics);
                        recover!(error);
                    }
                    let mut entry = pending.take().unwrap();
                    entry["predictMs"] = json!(phase_start.elapsed().as_millis() as u64);
                    entry["action"] = json!({"kind":"fail","reason":error});
                    entry["error"] = json!(error);
                    entry["grounding"] = json!(env.grounding_diagnostics());
                    env.progress(entry)?;
                    return Err(error);
                }
            };
            // The user can change the UI while paused, even if the same app and
            // accessibility target remain valid. Never resume a pre-pause plan.
            control.checkpoint().await?;
            observation!(verify_observation());
            pending.as_mut().unwrap()["grounding"] = json!(prediction.grounding);
            let predict_ms = phase_start.elapsed().as_millis() as u64;
            let mut action = match prediction.validated(frame.display) {
                Ok(action) => action,
                Err(error) => {
                    // Invalid proposals have not reached input or approval.
                    // Replan with explicit feedback under the existing retry caps.
                    pending.as_mut().unwrap()["planText"] = json!(prediction.plan_text);
                    recover!(format!("Invalid proposed action: {error}"));
                }
            };
            observation!(env.verify_target());
            let (element_label, secure) = observation!(env.ground_target(&prediction, &mut action));
            let description = format!(
                "{} {}",
                prediction.action["target"].as_str().unwrap_or(""),
                element_label
            )
            .trim()
            .to_owned();
            pending = Some(
                json!({"step":step+1,"staleRetries":stale_retries,"thumbnail":frame.thumbnail,"action":action,"target":description,"reflection":prediction.reflection,"planText":prediction.plan_text,"grounding":prediction.grounding,"predictMs":predict_ms,"executeMs":0,"waitMs":0}),
            );
            if let Some(reason) = &verify_only {
                match &action {
                    Action::Done { summary } => {
                        pending.as_mut().unwrap()["verificationOnly"] = json!(true);
                        env.progress(pending.take().unwrap())?;
                        return Ok(json!({"summary":summary,"screenshot":frame.png}));
                    }
                    Action::Fail { reason: remaining } => return Err(format!("{reason}\nVerification: {remaining}")),
                    _ => {
                        pending.as_mut().unwrap()["recovered"] = json!(true);
                        return Err(format!("{reason}\nRead-only verification rejected further input; no second action was sent."));
                    }
                }
            }
            if let Action::Fail { reason } = &action {
                if reason.starts_with(RECOVERABLE) || prediction
                    .grounding
                    .iter()
                    .any(|d| d["pixel_result"] == "unparseable")
                {
                    recover!(reason.clone());
                }
            }
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
                    recover!(
                        "App has read-only access: open the app by bundle ID, not a file".into()
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
            observation!(env.verify_target());
            let policy_target = observation!(input_target(&*env, &action));
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
                    Decision::Deny(reason) => recover!(reason),
                    Decision::Confirm(reason) => {
                        let entry = pending.as_mut().unwrap();
                        entry["kind"] = json!("approval_wait");
                        entry["reason"] = json!(reason);
                        env.progress(entry.clone())?;
                        env.phase(Phase::WaitingForApproval);
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
                observation!(verify_observation());
                let after = env.context()?;
                verify_app("Target binding changed after approval", &current, &after)?;
                if let Err(reason) = verify_policy_target(
                    "AX policy target changed after approval",
                    &policy_target,
                    &observation!(input_target(&*env, &action)),
                ) {
                    // Nothing has been sent. Discard this approval and plan again.
                    recover!(reason);
                }
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
                    recover!(reason);
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
            observation!(verify_observation());
            observation!(env.verify_target());
            env.phase(Phase::Acting);
            env.begin_action();
            let preparation = observation!(env.prepare(&action, control, &after));
            if preparation == Preparation::NeedsForeground {
                if !settings.allow_foreground_fallback {
                    recover!(
                        "Background input unavailable; foreground fallback is disabled".into()
                    );
                }
                let reason =
                    "This app ignored background input; take over the mouse for this step?";
                phase = "waitMs";
                phase_start = Instant::now();
                let entry = pending.as_mut().unwrap();
                // With approvals off the fallback setting is the user's consent.
                if settings.ask_approval {
                    entry["kind"] = json!("approval_wait");
                    entry["reason"] = json!(reason);
                    env.progress(entry.clone())?;
                    env.phase(Phase::WaitingForApproval);
                    control.until(env.approve(reason, &action, &after)).await?;
                }
                control.checkpoint().await?;
                observation!(verify_observation());
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
                    recover!(reason);
                }
                observation!(env.verify_target());
                verify_policy_target(
                    "AX policy target changed after fallback approval",
                    &policy_target,
                    &observation!(input_target(&*env, &action)),
                )?;
                env.phase(Phase::Acting);
                observation!(env.begin_foreground(control, &after));
                entry["waitMs"] = json!(
                    entry["waitMs"].as_u64().unwrap_or(0)
                        + phase_start.elapsed().as_millis() as u64
                );
                entry["foregroundApproval"] = json!(settings.ask_approval);
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
                control.checkpoint().await?;
                observation!(verify_observation());
                verify_policy_target(
                    "AX policy target changed before input",
                    &policy_target,
                    &observation!(input_target(&*env, &action)),
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
            let outcome = if outcome
                .as_ref()
                .err()
                .is_some_and(|e| e.starts_with(STALE_OBSERVATION))
            {
                observation!(outcome);
                Ok(())
            } else {
                outcome
            };
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
            step += 1;
            stale_retries = 0;
            if let Some(error) = outcome.as_ref().err().filter(|e| e.starts_with(UNCERTAIN)) {
                control.release();
                recovery.previous = Some(format!(
                    "READ-ONLY VERIFICATION: {error} The previous action may have partly executed. Inspect this fresh observation. Return done ONLY if the entire requested task is visibly complete; otherwise return fail explaining what remains or is uncertain. No click, edit, hotkey, retry, or other input is allowed.\n\n"
                ));
                verify_only = Some(error.clone());
                control.checkpoint().await?;
                env.phase(Phase::Settling);
                control.until(env.settle()).await?;
                continue;
            }
            if outcome
                .as_ref()
                .err()
                .is_some_and(|e| e == "Action interrupted by Pause")
            {
                recovery.previous = Some("Previous action was interrupted after input started: Pause may have left partial changes. Inspect the fresh observation, preserve completed work, and continue only the remaining action. Do not replay the original action.\n\n".into());
                control.checkpoint().await?;
                continue;
            }
            outcome?;
            recovery.consecutive = 0;
            recovery.previous = None;
            match action {
                Action::Done { summary } => {
                    // Done sends no input: return the observation the planner verified.
                    return Ok(json!({"summary":summary,"screenshot":frame.png}));
                }
                Action::Fail { reason } => return Err(reason),
                _ => (),
            }
            env.phase(Phase::Settling);
            control.until(env.settle()).await?;
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
    env.end_action();
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
