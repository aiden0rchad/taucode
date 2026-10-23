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
pub trait Environment {
    fn capture(&mut self) -> BoxFuture<'_, Result<Frame, String>>;
    fn predict<'a>(
        &'a mut self,
        instruction: &'a str,
        frame: &'a Frame,
    ) -> BoxFuture<'a, Result<Prediction, String>>;
    fn context(&self) -> Result<Context, String>;
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
async fn execute(
    env: &impl Environment,
    control: &Control,
    c: &Context,
    action: &Action,
    target: Option<&str>,
) -> Result<(), String> {
    let epoch = control.epoch();
    let key = |name: &str, down| {
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
                if env.context()? != *c {
                    return Err("Focus changed".into());
                }
                env.open(app_or_file, target.ok_or("Missing target app")?)?;
            }
            Action::SwitchApp { app } => {
                control.checkpoint().await?;
                if env.context()? != *c {
                    return Err("Focus changed".into());
                }
                env.open(app, target.ok_or("Missing target app")?)?;
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
) -> Result<Value, String> {
    let start = Instant::now();
    let work = async {
        for step in 0..settings.max_steps {
            control.checkpoint().await?;
            let context = env.context()?;
            if context.app != "dev.taucode.desktop" && !app_allowed(&context.app, settings, apps) {
                return Err(format!("App blocked: {}", context.app));
            }
            let frame = control.until(env.capture()).await?;
            let scoped = format!("{instruction}\n\nHost policy: allowed application bundle IDs: {}. Current frontmost application: {}. If taucode is frontmost, first use switch_applications with an allowed bundle ID. Never interact with taucode itself.", apps.join(", "), context.app);
            let prediction = control.until(env.predict(&scoped, &frame)).await?;
            control.checkpoint().await?;
            let action = prediction.validated(frame.display)?;
            let target = match &action {
                Action::Open { app_or_file } => Some(env.target(app_or_file)?),
                Action::SwitchApp { app } => Some(env.target(app)?),
                _ => None,
            };
            let policy_action = match &target {
                Some(app) => Action::SwitchApp { app: app.clone() },
                None => action.clone(),
            };
            let current = env.context()?;
            if current != context {
                return Err("Focus changed while planning; action cancelled".into());
            }
            match decide(
                &policy_action,
                &prediction.plan_text,
                &current,
                settings,
                apps,
                step,
                start.elapsed(),
            ) {
                Decision::Deny(reason) => return Err(reason),
                Decision::Confirm(reason) => {
                    control
                        .until(env.approve(reason, &action, &current))
                        .await?
                }
                Decision::Allow => (),
            }
            control.checkpoint().await?;
            let after = env.context()?;
            if after != current {
                return Err("Focus changed after approval; action cancelled".into());
            }
            if let Decision::Deny(reason) = decide(
                &policy_action,
                &prediction.plan_text,
                &after,
                settings,
                apps,
                step,
                start.elapsed(),
            ) {
                return Err(reason);
            }
            let outcome = execute(env, control, &after, &action, target.as_deref()).await;
            env.progress(json!({"step":step+1,"thumbnail":frame.thumbnail,"action":action,"reflection":prediction.reflection,"planText":prediction.plan_text,"error":outcome.as_ref().err()}))?;
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
    let result = tokio::time::timeout(Duration::from_secs(settings.max_seconds), work)
        .await
        .map_err(|_| "Maximum wall time reached".to_string())
        .and_then(|r| r);
    control.stop();
    result
}
