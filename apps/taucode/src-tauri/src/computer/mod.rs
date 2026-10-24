#[cfg(target_os = "macos")]
mod background;
mod control;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod panels;
pub mod policy;
mod runner;
mod sidecar;
#[cfg(test)]
mod tests;
mod transport;

use control::Control;
use policy::*;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};
use tokio::sync::{oneshot, watch};

pub struct Computer {
    owned: std::sync::atomic::AtomicBool,
    pub data: PathBuf,
    pub control: Control,
    pub state: watch::Sender<Value>,
    busy: tokio::sync::Mutex<()>,
    pending: Mutex<Option<(String, oneshot::Sender<bool>)>>,
    shortcut: Mutex<Option<String>>,
}
#[cfg(not(target_os = "macos"))]
struct Unsupported;
#[cfg(not(target_os = "macos"))]
impl control::Input for Unsupported {
    fn post(&mut self, _: &control::Event) -> Result<(), String> {
        Err("Computer Use requires macOS".into())
    }
}
impl Computer {
    pub fn new(data: PathBuf) -> Arc<Self> {
        #[cfg(target_os = "macos")]
        let input = native::NativeInput::default();
        #[cfg(not(target_os = "macos"))]
        let input = Unsupported;
        let (state, _) = watch::channel(json!({"running":false,"steps":[]}));
        Arc::new(Self {
            owned: std::sync::atomic::AtomicBool::new(false),
            data,
            control: Control::new(input),
            state,
            busy: tokio::sync::Mutex::new(()),
            pending: Mutex::new(None),
            shortcut: Mutex::new(None),
        })
    }
    pub fn stop(&self) {
        self.control.stop();
        self.pending.lock().unwrap().take();
        self.state.send_modify(|s| {
            s["approval"] = Value::Null;
            s["paused"] = json!(false);
        });
    }
    pub fn pause(&self, paused: bool) {
        self.control.pause(paused);
        self.state.send_modify(|s| s["paused"] = json!(paused));
    }
    pub fn answer(&self, id: &str, accept: bool) -> Result<(), String> {
        let mut pending = self.pending.lock().map_err(|_| "Approval unavailable")?;
        if pending.as_ref().is_none_or(|(key, _)| key != id) {
            return Err("Approval expired".into());
        }
        let (_, sender) = pending.take().unwrap();
        let _ = sender.send(accept);
        Ok(())
    }
    #[cfg(target_os = "macos")]
    fn arm(&self, app: &AppHandle, s: &Settings) -> Result<(), String> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        {
            let mut old = self.shortcut.lock().map_err(|_| "Shortcut unavailable")?;
            if old.as_deref() != Some(s.shortcut.as_str()) {
                app.global_shortcut()
                    .register(s.shortcut.as_str())
                    .map_err(|e| format!("Cannot register kill switch: {e}"))?;
                if let Some(old) = old.take() {
                    let _ = app.global_shortcut().unregister(old.as_str());
                }
                *old = Some(s.shortcut.clone());
            }
        }
        Ok(())
    }
    pub async fn request(self: &Arc<Self>, app: AppHandle, p: Value) -> Result<Value, String> {
        if !self.owned.load(std::sync::atomic::Ordering::SeqCst) {
            return transport::ipc(&self.data, p).await;
        }
        match p["action"].as_str() {
            Some("requestPermissions") => tokio::task::spawn_blocking(request_permissions)
                .await
                .map_err(|e| e.to_string())?,
            Some("runLog") => {
                let id = p["id"].as_str().ok_or("Missing run ID")?;
                uuid::Uuid::parse_str(id).map_err(|_| "Invalid run ID")?;
                let bytes =
                    std::fs::read(self.data.join("computer/runs").join(format!("{id}.json")))
                        .map_err(|_| "Computer run log unavailable")?;
                serde_json::from_slice(&bytes).map_err(|_| "Invalid computer run log".into())
            }
            Some("status") => {
                let mut state = self.state.borrow().clone();
                state["permissions"] = permissions();
                Ok(state)
            }
            Some("stop") => {
                self.stop();
                Ok(Value::Null)
            }
            Some("pause") => {
                self.pause(p["paused"] == true);
                Ok(Value::Null)
            }
            Some("prepare") => {
                let _lock = self.busy.try_lock().map_err(|_| "Computer Use is busy")?;
                self.state
                    .send_modify(|s| s["setup"] = json!("Preparing Python 3.12 environment…"));
                let resource = app.path().resource_dir().ok();
                let result = sidecar::prepare(&self.data, resource.as_deref()).await;
                self.state.send_modify(|s| {
                    s["setup"] = json!(if result.is_ok() {
                        "Ready"
                    } else {
                        "Setup failed"
                    })
                });
                result.map(|_| json!({"ok":true}))
            }
            Some("screenshot") => {
                let settings = self.enabled()?;
                #[cfg(target_os = "macos")]
                if settings.computer.mode == "foreground" {
                    let current = native::context()?;
                    if current.app != "dev.taucode.desktop"
                        && !app_allowed(
                            &current.app,
                            &settings.computer,
                            &settings.computer.allowed_apps,
                        )
                    {
                        return Err("Screenshot blocked by app policy".into());
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    let allowed: Vec<_> = settings
                        .computer
                        .allowed_apps
                        .iter()
                        .filter(|a| {
                            app_allowed(a, &settings.computer, &settings.computer.allowed_apps)
                        })
                        .cloned()
                        .collect();
                    let background = settings.computer.mode == "background";
                    let frame = tokio::task::spawn_blocking(move || {
                        if background {
                            background::screenshot(
                                allowed.first().ok_or("Select an allowed target app")?,
                            )
                        } else {
                            native::capture(&allowed)
                        }
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                    Ok(json!({"screenshot":frame.png}))
                }
                #[cfg(not(target_os = "macos"))]
                Err("Computer Use requires macOS".into())
            }
            Some("run") => self.run(app, p).await,
            _ => Err("Unknown computer action".into()),
        }
    }
    fn enabled(&self) -> Result<crate::types::AppSettings, String> {
        let s = crate::storage::read_settings(&self.data.join("settings.json"))?;
        validate(&s.computer)?;
        if !s.computer.enabled {
            return Err("Enable Computer Use in Settings".into());
        }
        Ok(s)
    }
    fn private_dir(&self) -> Result<PathBuf, String> {
        let dir = self.data.join("computer");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        Ok(dir)
    }
    async fn run(self: &Arc<Self>, app: AppHandle, p: Value) -> Result<Value, String> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, p);
            Err("Computer Use requires macOS".into())
        }
        #[cfg(target_os = "macos")]
        {
            let _lock = self
                .busy
                .try_lock()
                .map_err(|_| "A computer run is already active")?;
            let settings = self.enabled()?;
            if permissions()["screen"] != true || permissions()["accessibility"] != true {
                return Err("Grant Screen Recording and Accessibility in System Settings".into());
            }
            let instruction = p["instruction"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 32768)
                .ok_or("Provide a bounded instruction")?;
            let mut limits = settings.computer.clone();
            if let Some(v) = p.get("max_steps") {
                let n = v
                    .as_u64()
                    .filter(|n| *n > 0 && *n <= limits.max_steps as u64)
                    .ok_or("max_steps must be within configured limit")?;
                limits.max_steps = n as u32;
            }
            let apps: Vec<String> = match p.get("apps") {
                None => limits.allowed_apps.clone(),
                Some(v) => {
                    serde_json::from_value(v.clone()).map_err(|_| "Expected app bundle IDs")?
                }
            };
            if apps.is_empty() || apps.iter().any(|a| !app_allowed(a, &limits, &apps)) {
                return Err("Select allowed apps in Computer Use settings".into());
            }
            self.private_dir()?;
            let previous_focus = background::UserFocus::capture();
            self.arm(&app, &limits)?;
            self.control.replace_input(native::NativeInput::default());
            self.control.start();
            let _stop = StopOnDrop(self.clone());
            let id = uuid::Uuid::new_v4().to_string();
            self.state.send_replace(json!({"id":id,"running":true,"paused":false,"steps":[],"instruction":instruction,"shortcut":limits.shortcut}));
            let resource = app.path().resource_dir().ok();
            let start = std::time::Instant::now();
            let result = async {
                panels::show_hud(app.clone(), self).await?;
                let sidecar = self
                    .control
                    .until(async {
                        tokio::time::timeout(
                            std::time::Duration::from_secs(limits.max_seconds),
                            async {
                                let prepared =
                                    sidecar::prepare(&self.data, resource.as_deref()).await?;
                                sidecar::Sidecar::launch(&prepared, &self.data, &settings).await
                            },
                        )
                        .await
                        .map_err(|_| {
                            sidecar::setup_error(
                                &self.data,
                                "Maximum wall time reached during planner setup".into(),
                            )
                        })?
                    })
                    .await?;
                let session = if limits.mode == "background" {
                    let session = self
                        .control
                        .until(async {
                            tokio::time::timeout(
                                std::time::Duration::from_secs(limits.max_seconds)
                                    .saturating_sub(start.elapsed()),
                                background::Session::bind(
                                    &apps[0],
                                    app.clone(),
                                    limits.virtual_cursor,
                                    previous_focus,
                                ),
                            )
                            .await
                            .map_err(|_| {
                                "Maximum wall time reached while binding target".to_string()
                            })?
                        })
                        .await?;
                    self.control
                        .install_input(native::NativeInput::background(session.clone()))?;
                    Some(session)
                } else {
                    None
                };
                let mut env = Live {
                    session,
                    computer: self.clone(),
                    sidecar,
                    apps: apps.clone(),
                    id: id.clone(),
                    app: app.clone(),
                };
                let result =
                    runner::run(&mut env, &self.control, &limits, &apps, instruction, start).await;
                if let Some(s) = &env.session {
                    self.state.send_modify(|state| {
                        state["targetApp"] = json!({"bundleId":s.bundle,"pid":s.pid});
                        state["focusRestorations"] = json!(*s.guard.restorations.lock().unwrap());
                    });
                }
                result
            }
            .await;
            self.state.send_modify(|s| {
                s["running"] = json!(false);
                s["approval"] = Value::Null;
                s["error"] = json!(result.as_ref().err());
                if let Err(error) = &result {
                    let steps = s["steps"].as_array_mut().unwrap();
                    if steps
                        .last()
                        .is_none_or(|step| step["error"].as_str() != Some(error))
                    {
                        let next = steps.last().and_then(|s| s["step"].as_u64()).unwrap_or(0) + 1;
                        steps.push(runner::failure_step(next as u32, "", error));
                    }
                }
                s["summary"] = result
                    .as_ref()
                    .ok()
                    .map(|v| v["summary"].clone())
                    .unwrap_or(Value::Null);
            });
            self.persist(&id)?;
            let mut result = result.unwrap_or_else(|error| json!({"summary":error,"error":error}));
            result["computerRun"] = json!({"id":id,"stored":true,"steps":[],"summary":result["summary"],"error":result.get("error"),"instruction":instruction});
            Ok(result)
        }
    }
    fn persist(&self, id: &str) -> Result<(), String> {
        let dir = self.private_dir()?.join("runs");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{id}.json"));
        let temp = path.with_extension("tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec(&*self.state.borrow()).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    }
    pub async fn serve(self: &Arc<Self>, app: AppHandle) -> Result<(), String> {
        self.private_dir()?;
        transport::serve(self.clone(), app).await
    }
}
struct StopOnDrop(Arc<Computer>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.stop();
        self.0.state.send_modify(|s| {
            if s["running"] == true {
                s["error"] = json!("Stopped");
            }
            s["running"] = json!(false);
        });
        let id = self.0.state.borrow()["id"].as_str().map(str::to_owned);
        if let Some(id) = id {
            let _ = self.0.persist(&id);
        }
    }
}
pub fn permissions() -> Value {
    #[cfg(target_os = "macos")]
    {
        native::permissions()
    }
    #[cfg(not(target_os = "macos"))]
    {
        json!({"screen":false,"accessibility":false,"unsupported":true})
    }
}
#[cfg(any(target_os = "macos", test))]
trait PermissionRequests {
    fn request_screen(&self) -> bool;
    fn request_accessibility(&self) -> Result<bool, String>;
}
#[cfg(any(target_os = "macos", test))]
fn request_permissions_with(api: &impl PermissionRequests) -> Result<Value, String> {
    let screen = api.request_screen();
    let accessibility = api.request_accessibility()?;
    Ok(json!({"screen": screen, "accessibility": accessibility}))
}
fn request_permissions() -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        request_permissions_with(&native::NativePermissions)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(permissions())
    }
}
#[cfg(target_os = "macos")]
struct Live {
    session: Option<Arc<background::Session>>,
    computer: Arc<Computer>,
    sidecar: sidecar::Sidecar,
    apps: Vec<String>,
    id: String,
    app: AppHandle,
}
#[cfg(target_os = "macos")]
impl runner::Environment for Live {
    fn capture(
        &mut self,
    ) -> crate::shared::providers::BoxFuture<'_, Result<runner::Frame, String>> {
        let apps = self.apps.clone();
        let session = self.session.clone();
        Box::pin(async move {
            let f = tokio::task::spawn_blocking(move || match session {
                Some(s) => {
                    *s.path.lock().unwrap() = None;
                    s.capture()
                }
                None => native::capture(&apps),
            })
            .await
            .map_err(|e| e.to_string())??;
            Ok(runner::Frame {
                thumbnail: f.thumbnail,
                png: f.png,
                display: f.display,
            })
        })
    }
    fn predict<'a>(
        &'a mut self,
        instruction: &'a str,
        frame: &'a runner::Frame,
    ) -> crate::shared::providers::BoxFuture<'a, Result<Prediction, String>> {
        Box::pin(
            self.sidecar.predict(
                instruction,
                &frame.png,
                [frame.display.pixel_width, frame.display.pixel_height],
                self.session
                    .as_ref()
                    .map(|s| s.snapshot_json())
                    .unwrap_or(Value::Null),
            ),
        )
    }
    fn ground_target(
        &self,
        prediction: &Prediction,
        action: &mut Action,
    ) -> Result<(String, bool), String> {
        let target = prediction.element_target()?;
        match &self.session {
            Some(s) => s.select_element(target, action),
            None if target.is_none() => Ok((String::new(), false)),
            None => Err("Element snapshots unavailable".into()),
        }
    }
    fn verify_target(&self) -> Result<(), String> {
        self.session.as_ref().map_or(Ok(()), |s| s.verify_element())
    }
    fn planner_error(&self, error: String) -> String {
        self.sidecar.with_log(error)
    }
    fn context(&self) -> Result<Context, String> {
        match &self.session {
            Some(s) => s.context(),
            None => native::context(),
        }
    }
    fn capture_description(&self) -> String {
        match &self.session {
            Some(s) => format!(
                "Background target (window {})",
                s.window
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|w| format!("{}: {}", w.id, w.title))
                    .unwrap_or_default()
            ),
            None => "Current frontmost application".into(),
        }
    }
    fn prepare(
        &self,
        action: &Action,
        control: &Control,
        context: &Context,
    ) -> Result<runner::Preparation, String> {
        let Some(s) = &self.session else {
            return Ok(runner::Preparation::Events);
        };
        s.dragging.store(
            matches!(action, Action::Drag { .. }),
            std::sync::atomic::Ordering::SeqCst,
        );
        if matches!(
            action,
            Action::Wait { .. }
                | Action::Done { .. }
                | Action::Fail { .. }
                | Action::SwitchApp { .. }
                | Action::Open { .. }
        ) {
            return Ok(runner::Preparation::Events);
        }
        if let Action::Scroll { clicks, .. } = action {
            let mut handled = false;
            for _ in 0..clicks.unsigned_abs().max(1) {
                let next = control.guarded(context, || s.context(), || s.try_ax(action))?;
                if !next {
                    if handled {
                        return Err("AX scroll interrupted after partial delivery".into());
                    }
                    break;
                }
                handled = true;
            }
            if handled {
                return Ok(runner::Preparation::Handled);
            }
        }
        control.guarded(
            context,
            || s.context(),
            || {
                s.window()?;
                Ok(
                    match background::prepare_action(
                        action,
                        || {
                            if matches!(action, Action::Scroll { .. }) {
                                Ok(false)
                            } else {
                                s.try_ax(action)
                            }
                        },
                        || s.prepare_events(action),
                        true,
                    )? {
                        background::Path::Ax => runner::Preparation::Handled,
                        background::Path::Events => {
                            s.record_attempt(action);
                            runner::Preparation::Events
                        }
                        _ => runner::Preparation::NeedsForeground,
                    },
                )
            },
        )
    }
    fn begin_foreground(&self, control: &Control, context: &Context) -> Result<(), String> {
        let s = self.session.as_ref().ok_or("No background target")?;
        control.guarded(context, || s.context(), || s.begin_foreground())
    }
    fn end_action(&self) {
        if let Some(s) = &self.session {
            s.end_foreground();
        }
    }

    fn target(&self, app: &str) -> Result<String, String> {
        native::target(app).map(|v| v.0)
    }
    fn event_context(&self, event: &control::Event) -> Result<Context, String> {
        if let Some(s) = &self.session {
            s.window()?;
            if matches!(event, control::Event::Key(..) | control::Event::Text(_))
                && !s.events_available(&Action::Hotkey { keys: vec![] })
            {
                return Err("Target window no longer has the app's keyboard focus".into());
            }
            if s.foreground.load(std::sync::atomic::Ordering::SeqCst) {
                return native::event_context(event);
            }
            return s.context();
        }
        native::event_context(event)
    }
    fn open(&self, app: &str, target: &str) -> Result<(), String> {
        if let Some(s) = &self.session {
            if target != s.bundle {
                return Err("Background runs can only address their bound app".into());
            }
            return s.open(app);
        }
        native::open_app(app, target)
    }
    fn approve<'a>(
        &'a mut self,
        reason: &'a str,
        action: &'a Action,
        context: &'a Context,
    ) -> crate::shared::providers::BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = oneshot::channel();
            *self
                .computer
                .pending
                .lock()
                .map_err(|_| "Approval unavailable")? = Some((id.clone(), tx));
            let target = self.computer.state.borrow()["steps"]
                .as_array()
                .and_then(|steps| steps.last())
                .and_then(|s| s["target"].as_str())
                .unwrap_or("")
                .to_owned();
            self.computer.state.send_modify(|s| {
                s["approval"] = json!({"id":id,"reason":reason,"target":target,"action":action,"app":context.app})
            });
            use tauri_plugin_notification::NotificationExt;
            if let Err(error) = self
                .app
                .notification()
                .builder()
                .title("taucode needs your approval")
                .body(format!("{reason} ({target})"))
                .show()
            {
                self.computer
                    .state
                    .send_modify(|s| s["notificationError"] = json!(error.to_string()));
            }
            let approved = rx.await.unwrap_or(false);
            self.computer
                .state
                .send_modify(|s| s["approval"] = Value::Null);
            if approved {
                // Approving in the main UI may activate taucode. Restore only the exact app
                // the user just approved, then let the loop recheck the secure field.
                if self.session.is_none() && native::context()?.app == "dev.taucode.desktop" {
                    native::open_app(&context.app, &context.app)?;
                    for _ in 0..50 {
                        if native::context()? == *context {
                            return Ok(());
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                    return Err("Approved application did not regain its focus".into());
                }
                Ok(())
            } else {
                Err("Approval declined or cancelled".into())
            }
        })
    }
    fn progress(&mut self, mut step: Value) -> Result<(), String> {
        if let Some(s) = &self.session {
            step["path"] = json!(*s.path.lock().unwrap());
            step["window"] = json!(*s.window.lock().unwrap());
            step["focusRestorations"] = json!(*s.guard.restorations.lock().unwrap());
        } else {
            step["path"] = json!("foreground");
        }
        self.computer.state.send_modify(|s| {
            let steps = s["steps"].as_array_mut().unwrap();
            if let Some(previous) = steps
                .last_mut()
                .filter(|s| s["step"] == step["step"] && s["kind"] == step["kind"])
            {
                *previous = step;
            } else {
                steps.push(step);
            }
        });
        self.computer.persist(&self.id)
    }
}
pub use transport::{ipc, mcp_cli};
