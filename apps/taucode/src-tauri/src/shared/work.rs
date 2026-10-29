use super::{checkpoints::Checkpoints, engines::{EngineBackend, WorkspaceSession}, pi::{config, PiEngine}};
use crate::{backend::events::{AppServerEvent, EventSink}, types::{AppSettings, WorkspaceKind}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
use tokio::sync::{mpsc, oneshot, Mutex};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Space {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: WorkspaceKind,
    pub read_only_folders: Vec<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    imported: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    #[serde(default)]
    import_id: Option<String>,
    id: String,
    space_id: String,
    thread_id: Option<String>,
    title: String,
    brief: String,
    attachments: Vec<String>,
    model: String,
    #[serde(default)]
    effort: Option<String>,
    access_mode: String,
    status: String,
    #[serde(default)]
    created_at: Option<i64>,
    #[serde(default)]
    updated_at: Option<i64>,
    plan: Vec<Value>,
    outputs: Vec<String>,
    pending: Vec<Value>,
    error: Option<String>,
    #[serde(default)]
    pending_message: Option<String>,
}
impl Task {
    fn set_status(&mut self, status: &str) {
        if self.status != status {
            self.status = status.into();
            self.updated_at = Some(chrono::Utc::now().timestamp());
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Store {
    spaces: Vec<Space>,
    tasks: Vec<Task>,
    concurrency: usize,
}
impl Default for Store {
    fn default() -> Self { Self { spaces: vec![], tasks: vec![], concurrency: 2 } }
}
struct State {
    store: Store,
    sessions: HashMap<String, Arc<WorkspaceSession>>,
    settings: AppSettings,
    loaded: bool,
    starts: HashMap<String, oneshot::Sender<()>>,
}
pub(crate) struct Work {
    data: PathBuf,
    state: Mutex<State>,
    emit: Arc<dyn Fn(AppServerEvent) + Send + Sync>,
    tx: mpsc::UnboundedSender<AppServerEvent>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<AppServerEvent>>>,
}
fn occupied(status: &str) -> bool { matches!(status, "starting" | "running" | "needs-input" | "stopping") }
fn next_task(store: &Store) -> Option<usize> {
    if store.tasks.iter().filter(|t| occupied(&t.status)).count() >= store.concurrency { return None; }
    // A single writer per space makes checkpoint attribution and rewind deterministic.
    store.tasks.iter().position(|t| t.status == "queued" && !store.tasks.iter().any(|other| other.space_id == t.space_id && occupied(&other.status)))
}
fn directory(value: &str) -> Result<String, String> {
    let path = Path::new(value).canonicalize().map_err(|e| e.to_string())?;
    if !path.is_dir() || path.parent().is_none() { return Err("Choose a folder".into()); }
    Ok(path.to_string_lossy().into_owned())
}
fn contained(root: &str, path: &str) -> Result<PathBuf, String> {
    let root = Path::new(root).canonicalize().map_err(|e| e.to_string())?;
    let path = root.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) || !path.is_file() { return Err("File must be inside this space".into()); }
    Ok(path)
}
fn merge_outputs(task: &mut Task, space: &Space, paths: impl IntoIterator<Item = String>) {
    for path in paths {
        if let Ok(path) = contained(&space.path, &path) {
            let root = Path::new(&space.path);
            if let Ok(relative) = path.strip_prefix(root) {
                let name = relative.to_string_lossy().into_owned();
                if !name.starts_with(".taucode/") && !task.outputs.contains(&name) { task.outputs.push(name); }
            }
        }
    }
    task.outputs.sort();
}
impl Work {
    pub fn new(data: PathBuf, sink: impl EventSink) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            data, state: Mutex::new(State { store: Store::default(), sessions: HashMap::new(), settings: AppSettings::default(), loaded: false, starts: HashMap::new() }),
            emit: Arc::new(move |event| sink.emit_app_server_event(event)), tx, rx: Mutex::new(Some(rx)),
        })
    }
    fn save(&self, store: &Store) -> Result<(), String> {
        let path = self.data.join("work/state.json");
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        config::write_json(&path, &json!(store))
    }
    fn publish(&self, state: &State, task: Option<&Task>, previous: Option<&str>) {
        let needs = state.store.tasks.iter().filter(|t| t.status == "needs-input").count();
        (self.emit)(AppServerEvent { workspace_id: "work".into(), message: json!({
            "method":"work/updated","params":{"task":task,"needsInput":needs,
                "notify":state.settings.system_notifications_enabled && task.is_some_and(|t| previous != Some(&t.status) && matches!(t.status.as_str(),"done"|"needs-input"|"failed"))}
        }) });
    }
    async fn initialize(self: &Arc<Self>, state: &mut State) -> Result<(), String> {
        if !state.loaded {
            state.store = match std::fs::read(self.data.join("work/state.json")) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
                Err(e) => return Err(e.to_string()),
            };
            for task in &mut state.store.tasks {
                if occupied(&task.status) {
                    task.set_status("stopped");
                    task.error = Some("Interrupted by app restart. Send a message below to continue.".into());
                    task.pending.clear();
                }
            }
            self.save(&state.store)?;
            state.loaded = true;
        }
        self.sync_imports(&mut state.store)?;
        if let Some(mut rx) = self.rx.lock().await.take() {
            let weak = Arc::downgrade(self);
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    let Some(this) = weak.upgrade() else { break };
                    let mut state = this.state.lock().await;
                    if let Err(error) = this.observe(&mut state, &event).await {
                        (this.emit)(AppServerEvent { workspace_id: "work".into(), message: json!({"method":"work/error","params":{"message":error}}) });
                    }
                    (this.emit)(event);
                    this.pump(&mut state).await;
                }
            });
        }
        Ok(())
    }
    fn sync_imports(&self, store: &mut Store) -> Result<(), String> {
        let histories = super::imports::work_history(&self.data)?;
        store.tasks.retain(|t| t.import_id.is_none());
        for (id, history) in histories {
            let path = directory(history["cwd"].as_str().unwrap_or_default()).ok()
                .filter(|path| super::checkpoints::validate_space(&self.data, "import-validation", Path::new(path)).is_ok());
            let space = match store.spaces.iter().find(|s| match &path { Some(path) => !s.read_only && s.path == *path, None => s.id == "imported-missing" }) {
                Some(space) => space.clone(),
                None => {
                    let read_only = path.is_none();
                    let space = Space {
                        id: if read_only { "imported-missing".into() } else { uuid::Uuid::new_v4().to_string() },
                        name: path.as_ref().and_then(|p| Path::new(p).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported (folder missing)".into()),
                        path: path.unwrap_or_default(), kind: WorkspaceKind::Work, read_only_folders: vec![], read_only, imported: true,
                    };
                    store.spaces.push(space.clone());
                    space
                }
            };
            let task_id = format!("import-{id}");
            let completed = history["status"].as_str().map_or_else(
                || history["archived"] != true && history["warning"].is_null() && history["messages"].as_array().and_then(|m| m.last()).is_some_and(|m| m["role"] == "assistant"),
                |status| status == "done",
            );
            store.tasks.push(Task {
                id: task_id.clone(), import_id: Some(id), space_id: space.id, thread_id: Some(task_id),
                title: history["title"].as_str().unwrap_or("Imported Cowork task").into(),
                brief: history["messages"].as_array().and_then(|m| m.iter().find(|m| m["role"] == "user")).and_then(|m| m["text"].as_str()).unwrap_or_default().into(),
                created_at: None, updated_at: None, attachments: vec![], model: String::new(), effort: None, access_mode: "read-only".into(),
                status: if completed { "done" } else { "stopped" }.into(), plan: vec![],
                outputs: history["attachments"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
                pending: vec![], pending_message: None, error: history["warning"].as_str().map(str::to_owned),
            });
        }
        store.spaces.retain(|s| !s.imported || store.tasks.iter().any(|t| t.space_id == s.id));
        Ok(())
    }
    fn imported_history(&self, task: &Task) -> Result<Value, String> {
        let id = task.import_id.as_ref().ok_or("Not an imported task")?;
        super::imports::work_history(&self.data)?.into_iter().find(|(key, _)| key == id).map(|(_, value)| value).ok_or("Imported task was removed".into())
    }
    async fn session(&self, state: &mut State, space: &Space) -> Arc<WorkspaceSession> {
        if let Some(session) = state.sessions.get(&space.id) {
            session.pi.configure(&space.id, state.settings.providers.clone(), state.settings.agent_policy.clone()).await;
            return session.clone();
        }
        let mut session = WorkspaceSession::lazy(
            Box::new(|| Box::pin(async { Err("Work tasks use pi".into()) })),
            state.settings.providers.clone(), state.settings.agent_policy.clone(), None, None, space.id.clone(),
        );
        let tx = self.tx.clone();
        session.pi = PiEngine::work(self.data.clone(), space.id.clone(), space.path.clone(), Arc::new(move |workspace, message| {
            let _ = tx.send(AppServerEvent { workspace_id: workspace.into(), message });
        }));
        #[cfg(test)] { session.pi.fake = true; }
        session.pi.configure(&space.id, state.settings.providers.clone(), state.settings.agent_policy.clone()).await;
        session.pi.read_only_folders.lock().await.insert(space.id.clone(), space.read_only_folders.clone());
        #[cfg(not(test))]
        let limits = super::checkpoints::CheckpointLimits::default();
        #[cfg(test)]
        let limits = super::checkpoints::CheckpointLimits::testing();
        session.checkpoints = Some(Checkpoints::with_limits(self.data.clone(), space.id.clone(), space.path.clone(), self.emit.clone(), limits));
        let session = Arc::new(session);
        state.sessions.insert(space.id.clone(), session.clone());
        session
    }
    fn pump<'a>(self: &'a Arc<Self>, state: &'a mut State) -> super::providers::BoxFuture<'a, ()> {
        Box::pin(async move {
            if !super::engine_registry::features(Some(&self.data)).work_mode { return; }
            while let Some(index) = next_task(&state.store) {
                let space = state.store.spaces.iter().find(|s| s.id == state.store.tasks[index].space_id).unwrap().clone();
                let session = self.session(state, &space).await;
                let task = &mut state.store.tasks[index];
                // Work thread creation does not launch a process or access credentials.
                if task.thread_id.is_none() {
                    match session.pi.request(&space.id, "thread/start", json!({"model":task.model})).await {
                        Ok(response) => task.thread_id = response["result"]["thread"]["id"].as_str().map(str::to_owned),
                        Err(error) => { task.set_status("failed"); task.error = Some(error); }
                    }
                    if task.thread_id.is_none() {
                        if task.error.is_none() { task.error = Some("Missing task thread".into()); task.set_status("failed"); }
                        let _ = self.save(&state.store);
                        self.publish(state, Some(&state.store.tasks[index]), Some("queued"));
                        continue;
                    }
                }
                task.pending_message.get_or_insert_with(|| task.brief.clone());
                task.set_status("starting");
                let task = task.clone();
                if let Err(error) = self.save(&state.store) {
                    state.store.tasks[index].set_status("failed");
                    state.store.tasks[index].error = Some(error);
                    self.publish(state, Some(&state.store.tasks[index]), Some("queued"));
                    break;
                }
                let (cancel, cancelled) = oneshot::channel();
                state.starts.insert(task.id.clone(), cancel);
                self.publish(state, Some(&state.store.tasks[index]), Some("queued"));
                let this = self.clone();
                tokio::spawn(async move {
                    let thread = task.thread_id.as_deref().unwrap();
                    let mut text = task.pending_message.clone().unwrap_or_else(|| task.brief.clone());
                    if !task.attachments.is_empty() { text.push_str(&format!("\n\nAttached files (inside this space):\n{}", task.attachments.join("\n"))); }
                    if !space.read_only_folders.is_empty() { text.push_str(&format!("\n\nRead-only folders:\n{}", space.read_only_folders.join("\n"))); }
                    let params = json!({"threadId":thread,"accessMode":task.access_mode,"model":task.model,"effort":task.effort,"input":[{"type":"text","text":text}]});
                    let checkpoints = session.checkpoints.as_ref().unwrap();
                    // Finish checkpoint preparation before cancellation cleanup; its disk
                    // work must not race a later launch after this future is dropped.
                    let prepared = if super::engine_registry::features(Some(&this.data)).checkpoints {
                        checkpoints.before(&space.id, thread, "pi", &params).await
                    } else { Ok(()) };
                    let result = match prepared {
                        Err(error) => Some(Err(error)),
                        Ok(()) => tokio::select! {
                            biased;
                            _ = cancelled => None,
                            result = session.pi.request(&space.id, "turn/start", params) => Some(result.map(|_| ())),
                        },
                    };
                    // Claim completion under the same lock as stop, so a late stop wins.
                    let mut state = this.state.lock().await;
                    state.starts.remove(&task.id);
                    let Some(index) = state.store.tasks.iter().position(|t| t.id == task.id) else { return };
                    let cancelled = result.is_none() || state.store.tasks[index].status == "stopping";
                    let failed = result.as_ref().is_some_and(|r| r.is_err());
                    if cancelled || failed {
                        state.store.tasks[index].set_status("stopping");
                        drop(state);
                        if cancelled { let _ = session.pi.request(&space.id, "turn/interrupt", json!({"threadId":task.thread_id})).await; }
                        let checkpoints = session.checkpoints.as_ref().unwrap();
                        if let Some(finish) = checkpoints.complete(&space.id, task.thread_id.as_deref().unwrap()) { let _ = finish.await; }
                        state = this.state.lock().await;
                    }
                    let Some(index) = state.store.tasks.iter().position(|t| t.id == task.id) else { return };
                    let entry = &mut state.store.tasks[index];
                    if cancelled { entry.set_status("stopped"); entry.pending.clear(); }
                    else if let Some(Err(error)) = result { entry.set_status("failed"); entry.error = Some(error); }
                    else { if entry.status == "starting" { entry.set_status("running"); } entry.pending_message = None; }
                    if let Err(error) = this.save(&state.store) { state.store.tasks[index].error = Some(error); }
                    this.publish(&state, Some(&state.store.tasks[index]), Some("starting"));
                    this.pump(&mut state).await;
                });
            }
        })
    }
    async fn observe(&self, state: &mut State, event: &AppServerEvent) -> Result<(), String> {
        let params = &event.message["params"];
        let Some(index) = state.store.tasks.iter().position(|t| t.space_id == event.workspace_id && t.thread_id.as_deref() == params["threadId"].as_str()) else { return Ok(()) };
        let session = state.sessions.get(&event.workspace_id).ok_or("Missing work session")?;
        if let Some(finish) = session.checkpoints.as_ref().unwrap().observe(event) { let _ = finish.await; }
        let space = state.store.spaces.iter().find(|s| s.id == event.workspace_id).unwrap().clone();
        let task = &mut state.store.tasks[index];
        let previous = task.status.clone();
        let method = event.message["method"].as_str().unwrap_or("");
        if occupied(&task.status) && task.status != "stopping" {
            if method == "turn/started" { task.set_status("running"); task.pending_message = None; }
            if method.ends_with("/requestApproval") || method == "item/tool/requestUserInput" {
                if !task.pending.iter().any(|p| p["id"] == event.message["id"]) { task.pending.push(event.message.clone()); }
                task.set_status("needs-input");
            }
            if method == "turn/completed" {
                task.set_status(match params["turn"]["status"].as_str() { Some("failed") => "failed", Some("interrupted") => "stopped", _ => "done" });
                task.error = params["turn"]["error"]["message"].as_str().map(str::to_owned);
                task.pending.clear();
            }
        }
        if method == "serverRequest/resolved" {
            task.pending.retain(|p| p["id"] != params["requestId"]);
            if task.status == "needs-input" && task.pending.is_empty() { task.set_status("running"); }
        }
        if method == "item/completed" || method == "item/started" {
            let item = &params["item"];
            if item["type"] == "plan" { task.plan = item["plan"].as_array().cloned().unwrap_or_default(); }
            if item["type"] == "fileChange" && method == "item/completed" && item["status"] == "completed" {
                merge_outputs(task, &space, item["changes"].as_array().into_iter().flatten().filter_map(|c| c["path"].as_str().map(str::to_owned)));
            }
        }
        if method == "turn/completed" && !matches!(previous.as_str(), "stopped" | "stopping") {
            match session.checkpoints.as_ref().unwrap().outputs(&space.id, task.thread_id.as_deref().unwrap()).await {
                Ok(paths) => merge_outputs(task, &space, paths),
                Err(error) => task.error = Some(format!("{}Checkpoint outputs unavailable: {error}", task.error.as_ref().map(|e| format!("{e}\n")).unwrap_or_default())),
            }
        }
        self.save(&state.store)?;
        if previous != state.store.tasks[index].status || matches!(method, "item/completed" | "turn/completed") {
            self.publish(state, Some(&state.store.tasks[index]), Some(&previous));
        }
        Ok(())
    }
    pub async fn request(self: &Arc<Self>, settings: &AppSettings, action: &str, params: Value) -> Result<Value, String> {
        if !settings.features().work_mode && !matches!(action, "stop" | "interrupt" | "list" | "read") { return Err("Enable Work in Settings → Features".into()); }
        let mut state = self.state.lock().await;
        self.initialize(&mut state).await?;
        state.settings = settings.clone();
        if action == "read" {
            if let Some(task) = state.store.tasks.iter().find(|t| Some(t.id.as_str()) == params["taskId"].as_str() && t.import_id.is_none()).cloned() {
                let space = state.store.spaces.iter().find(|s| s.id == task.space_id).ok_or("Unknown space")?.clone();
                let thread = task.thread_id.ok_or("Task is queued")?;
                let session = self.session(&mut state, &space).await;
                drop(state);
                return session.pi.request(&space.id, "thread/read", json!({"threadId":thread})).await;
            }
        }
        let result = self.action(&mut state, action, params).await;
        self.save(&state.store)?;
        if !matches!(action, "list" | "read" | "checkpoint") { self.publish(&state, None, None); }
        // Launch after returning to the caller; the queue belongs to Rust, not a view.
        if next_task(&state.store).is_some() {
            let this = self.clone();
            tokio::spawn(async move { let mut state = this.state.lock().await; this.pump(&mut state).await; });
        }
        result
    }
    async fn action(&self, state: &mut State, mut action: &str, mut params: Value) -> Result<Value, String> {
        if action == "continue" {
            let task = state.store.tasks.iter().find(|t| Some(t.id.as_str()) == params["taskId"].as_str()).ok_or("Unknown task")?;
            let history = self.imported_history(task)?;
            let context = super::imports::continuation(history["messages"].as_array().ok_or("No transcript to continue")?);
            params = json!({"spaceId":params["spaceId"].as_str().unwrap_or(&task.space_id), "model":params["model"], "brief":format!("Continue: {}\n\n{}\n\nContinue this task. Ask before changing existing files.", task.title, context), "attachments":[], "accessMode":"ask", "runNow":true});
            action = "create";
        }
        match action {
            "list" => return Ok(json!(state.store)),
            "add-space" => {
                let path = directory(params["path"].as_str().ok_or("Choose a folder")?)?;
                if state.store.spaces.iter().any(|s| s.path == path) { return Err("This folder is already a space".into()); }
                let folders: Vec<String> = serde_json::from_value(params["readOnlyFolders"].clone()).map_err(|_| "Invalid read-only folders")?;
                if folders.len() > 20 { return Err("Choose at most 20 read-only folders".into()); }
                let folders = folders.iter().map(|p| directory(p)).collect::<Result<Vec<_>, _>>()?;
                let space = Space { id: uuid::Uuid::new_v4().to_string(), name: Path::new(&path).file_name().unwrap().to_string_lossy().into_owned(), path, kind: WorkspaceKind::Work, read_only_folders: folders, read_only: false, imported: false };
                // Validate checkpoint support before accepting the space.
                super::checkpoints::validate_space(&self.data, &space.id, Path::new(&space.path))?;
                state.store.spaces.push(space.clone());
                return Ok(json!(space));
            }
            "concurrency" => {
                let count = params["count"].as_u64().filter(|n| (1..=8).contains(n)).ok_or("Concurrency must be 1–8")?;
                state.store.concurrency = count as usize;
                return Ok(json!({}));
            }
            "create" => {
                let space_id = params["spaceId"].as_str().ok_or("Choose a space")?;
                let space = state.store.spaces.iter().find(|s| s.id == space_id).ok_or("Unknown space")?;
                if space.read_only { return Err("Choose an available folder to continue this imported task".into()); }
                let brief = params["brief"].as_str().unwrap_or("").trim();
                if brief.is_empty() || brief.len() > 100_000 { return Err("Brief must be 1–100000 bytes".into()); }
                let mode = params["accessMode"].as_str().unwrap_or("ask");
                if !["ask", "read-only", "full-access"].contains(&mode) { return Err("Invalid access mode".into()); }
                let attachments: Vec<String> = serde_json::from_value(params["attachments"].clone()).map_err(|_| "Invalid attachments")?;
                if attachments.len() > 31 { return Err("Attach at most 31 files".into()); }
                for path in &attachments { contained(&space.path, path)?; }
                let model = params["model"].as_str().ok_or("Select a model")?;
                let (provider, model_id) = model.split_once('/').ok_or("Select a registry model")?;
                if !state.settings.providers.iter().any(|p| p.enabled && p.id == provider && p.models.iter().any(|m| m.enabled && m.id == model_id)) { return Err("Enable this registry model in Settings".into()); }
                let task = Task { created_at: Some(chrono::Utc::now().timestamp()), updated_at: None, id: uuid::Uuid::new_v4().to_string(), import_id: None, space_id: space_id.into(), thread_id: None, title: brief.lines().next().unwrap().chars().take(100).collect(), brief: brief.into(), attachments, model: model.into(), effort: params["effort"].as_str().map(str::to_owned), access_mode: mode.into(), status: "queued".into(), plan: vec![], outputs: vec![], pending: vec![], pending_message: None, error: None };
                let result = json!(task);
                if params["runNow"] == true { state.store.tasks.insert(0, task); } else { state.store.tasks.push(task); }
                return Ok(result);
            }
            "stop-all" => {
                let ids = state.store.tasks.iter().filter(|t| occupied(&t.status) || t.status == "queued").map(|t| t.id.clone()).collect::<Vec<_>>();
                let mut errors = Vec::new();
                for id in ids {
                    if let Err(error) = self.stop(state, &id).await { errors.push(error); }
                }
                return if errors.is_empty() { Ok(json!({})) } else { Err(errors.join("\n")) };
            }
            _ => (),
        }
        let id = params["taskId"].as_str().ok_or("Missing task ID")?;
        if action == "stop" { self.stop(state, id).await?; return Ok(json!({})); }
        let index = state.store.tasks.iter().position(|t| t.id == id).ok_or("Unknown task")?;
        let task = state.store.tasks[index].clone();
        if task.import_id.is_some() {
            return if action == "read" {
                let history = self.imported_history(&task)?;
                Ok(json!({"result":{"thread":{"id":task.thread_id,"turns":history["turns"]}}}))
            } else { Err("Imported tasks are read-only. Continue as a new task.".into()) };
        }
        let space = state.store.spaces.iter().find(|s| s.id == task.space_id).unwrap().clone();
        let thread = task.thread_id.as_deref().ok_or("Task is queued")?;
        let session = self.session(state, &space).await;
        match action {
            "read" => session.pi.request(&space.id, "thread/read", json!({"threadId":thread})).await,
            "follow-up" => {
                let message = params["message"].as_str().unwrap_or("").trim();
                if message.is_empty() || message.len() > 100_000 { return Err("Message must be 1–100000 bytes".into()); }
                if space.read_only { return Err("This space is read-only".into()); }
                if task.status == "needs-input" { return Err("Answer the pending request before sending another message".into()); }
                if matches!(task.status.as_str(), "queued" | "starting" | "stopping") { return Err("Wait for this task to start or finish stopping".into()); }
                let input = json!([{"type":"text","text":message}]);
                if task.status == "running" {
                    if params.get("model").is_some_and(|model| model != &json!(task.model)) || params.get("effort").is_some_and(|effort| effort != &json!(task.effort)) {
                        return Err("Stop this task before changing its model or reasoning".into());
                    }
                    return session.pi.request(&space.id, "turn/steer", json!({"threadId":thread,"input":input})).await;
                }
                if state.store.tasks.iter().any(|t| t.space_id == space.id && occupied(&t.status)) || state.store.tasks.iter().filter(|t| occupied(&t.status)).count() >= state.store.concurrency {
                    return Err("Wait for an available task slot before continuing".into());
                }
                let mode = params["accessMode"].as_str().unwrap_or(&task.access_mode);
                if !["ask", "read-only", "full-access"].contains(&mode) { return Err("Invalid access mode".into()); }
                let model = match params.get("model") { Some(value) => value.as_str().ok_or("Select a registry model")?, None => &task.model };
                let effort = match params.get("effort") {
                    None if model == task.model => task.effort.clone(),
                    None | Some(Value::Null) => None,
                    Some(value) => Some(value.as_str().ok_or("Invalid reasoning level")?.to_owned()),
                };
                if params.get("model").is_some() || params.get("effort").is_some() {
                    let (provider, model_id) = model.split_once('/').ok_or("Select a registry model")?;
                    let selected = state.settings.providers.iter().find(|p| p.enabled && p.id == provider)
                        .and_then(|p| p.models.iter().find(|m| m.enabled && m.id == model_id)).ok_or("Enable this registry model in Settings")?;
                    if let Some(level) = &effort {
                        if !selected.reasoning.as_ref().is_some_and(|r| r.supported && r.levels.contains(level)) {
                            return Err("Choose a reasoning level supported by this model".into());
                        }
                    }
                }
                let task = &mut state.store.tasks[index];
                task.model = model.into();
                task.effort = effort;
                task.pending_message = Some(message.into());
                task.set_status("queued");
                task.access_mode = mode.into();
                task.error = None;
                Ok(json!({}))
            }
            "respond" => {
                let request = task.pending.iter().find(|p| p["id"] == params["requestId"]).ok_or("Input request is no longer pending")?;
                session.pi.respond_to_server_request(request["id"].clone(), params["answer"].clone()).await?;
                let task = &mut state.store.tasks[index];
                task.pending.retain(|p| p["id"] != params["requestId"]);
                if task.pending.is_empty() { task.set_status("running"); }
                Ok(json!({}))
            }
            "checkpoint" => {
                if matches!(task.status.as_str(), "starting" | "stopping") { return Err("Wait for startup or stopping before opening checkpoints".into()); }
                let mut request = params["request"].clone();
                if !request.is_object() { return Err("Invalid checkpoint request".into()); }
                request["threadId"] = json!(thread);
                let result = session.checkpoints.as_ref().unwrap().request(&session, &space.id, request).await?;
                if result["rewound"] == true || result["conversationChanged"] == true {
                    state.store.tasks[index].outputs.retain(|path| contained(&space.path, path).is_ok());
                }
                Ok(result)
            }
            _ => Err("Unknown work action".into()),
        }
    }
    async fn stop(&self, state: &mut State, id: &str) -> Result<(), String> {
        let index = state.store.tasks.iter().position(|t| t.id == id).ok_or("Unknown task")?;
        if let Some(cancel) = state.starts.remove(id) {
            state.store.tasks[index].set_status("stopping");
            let _ = cancel.send(());
            return Ok(());
        }
        if state.store.tasks[index].status == "stopping" { return Ok(()); }
        let task = state.store.tasks[index].clone();
        if occupied(&task.status) {
            if let (Some(thread), Some(session)) = (&task.thread_id, state.sessions.get(&task.space_id)) {
                session.pi.request(&task.space_id, "turn/interrupt", json!({"threadId":thread})).await?;
                let checkpoints = session.checkpoints.as_ref().unwrap();
                if let Some(finish) = checkpoints.complete(&task.space_id, thread) { let _ = finish.await; }
                let space = state.store.spaces.iter().find(|s| s.id == task.space_id).unwrap();
                match checkpoints.outputs(&task.space_id, thread).await {
                    Ok(paths) => merge_outputs(&mut state.store.tasks[index], space, paths),
                    Err(error) => state.store.tasks[index].error = Some(error),
                }
            }
        }
        if occupied(&task.status) || task.status == "queued" {
            state.store.tasks[index].set_status("stopped");
            state.store.tasks[index].pending.clear();
        }
        Ok(())
    }
    // Explicit local picker/drop ingress only: not available to agents or daemon RPC.
    pub async fn attachment(self: &Arc<Self>, settings: &AppSettings, space_id: &str, source: &Path) -> Result<String, String> {
        let mut state = self.state.lock().await;
        self.initialize(&mut state).await?;
        state.settings = settings.clone();
        let space = state.store.spaces.iter().find(|s| s.id == space_id).ok_or("Choose a space")?;
        if space.read_only { return Err("This imported space is read-only".into()); }
        let file = std::fs::File::open(source).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > 50 * 1024 * 1024 { return Err("Attach a regular file up to 50 MB".into()); }
        let name = source.file_name().and_then(|n| n.to_str()).ok_or("Invalid filename")?;
        let base = Path::new(&space.path).join(".taucode");
        if base.exists() && !base.canonicalize().map_err(|e| e.to_string())?.starts_with(Path::new(&space.path)) {
            return Err("Uploads cannot leave the space".into());
        }
        let inbox = base.join("uploads");
        if inbox.exists() && !inbox.canonicalize().map_err(|e| e.to_string())?.starts_with(Path::new(&space.path)) {
            return Err("Uploads cannot leave the space".into());
        }
        std::fs::create_dir_all(&inbox).map_err(|e| e.to_string())?;
        let inbox = inbox.canonicalize().map_err(|e| e.to_string())?;
        if !inbox.starts_with(Path::new(&space.path)) { return Err("Uploads cannot leave the space".into()); }
        let target = inbox.join(format!("{}-{name}", uuid::Uuid::new_v4()));
        let mut dest = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e| e.to_string())?;
        use std::io::Read;
        let count = std::io::copy(&mut file.take(50 * 1024 * 1024 + 1), &mut dest).map_err(|e| e.to_string())?;
        if count > 50 * 1024 * 1024 { let _ = std::fs::remove_file(&target); return Err("Attachment grew beyond 50 MB".into()); }
        Ok(target.strip_prefix(&space.path).unwrap().to_string_lossy().into_owned())
    }
    pub async fn output(self: &Arc<Self>, settings: &AppSettings, task_id: &str, path: &str) -> Result<PathBuf, String> {
        let mut state = self.state.lock().await;
        self.initialize(&mut state).await?;
        state.settings = settings.clone();
        let task = state.store.tasks.iter().find(|t| t.id == task_id).ok_or("Unknown task")?;
        if let Some(id) = &task.import_id {
            if !task.outputs.iter().any(|p| p == path) { return Err("Unknown task output".into()); }
            let root = self.data.canonicalize().map_err(|e| e.to_string())?.join("imports/attachments");
            let folder = root.join(id).canonicalize().map_err(|e| e.to_string())?;
            if !folder.starts_with(&root) { return Err("Output must be inside imported copies".into()); }
            return contained(&folder.to_string_lossy(), path);
        }
        // Explicit local file links may refer to inputs or unchanged outputs, too.
        let space = state.store.spaces.iter().find(|s| s.id == task.space_id).ok_or("This task's space is unavailable")?;
        contained(&space.path, path)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::events::{TerminalExit, TerminalOutput};
    #[derive(Clone, Default)]
    struct Sink(Arc<std::sync::Mutex<Vec<AppServerEvent>>>);
    impl EventSink for Sink {
        fn emit_app_server_event(&self, event: AppServerEvent) { self.0.lock().unwrap().push(event); }
        fn emit_terminal_output(&self, _: TerminalOutput) {}
        fn emit_terminal_exit(&self, _: TerminalExit) {}
    }
    fn task(id: &str, space: &str, status: &str) -> Task {
        Task { created_at: None, updated_at: None, import_id: None, id: id.into(), space_id: space.into(), thread_id: Some(id.into()), title: id.into(), brief: "brief".into(), attachments: vec![], model: "local/test".into(), effort: None, access_mode: "ask".into(), status: status.into(), plan: vec![], outputs: vec![], pending: vec![], pending_message: None, error: None }
    }
    #[test]
    fn task_timestamps_are_backward_compatible_and_track_status_changes() {
        let mut value = json!(task("legacy", "space", "queued"));
        value.as_object_mut().unwrap().remove("createdAt");
        value.as_object_mut().unwrap().remove("updatedAt");
        let mut task: Task = serde_json::from_value(value).unwrap();
        assert_eq!(task.updated_at, None);
        task.set_status("queued");
        assert_eq!(task.updated_at, None);
        task.set_status("running");
        let updated = task.updated_at.unwrap();
        assert!(updated <= chrono::Utc::now().timestamp());
        assert_eq!(json!(task)["updatedAt"], updated);
        task.set_status("running");
        assert_eq!(task.updated_at, Some(updated));
    }
    fn temp() -> PathBuf {
        let root = std::env::temp_dir().join(format!("work-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }
    async fn wait_status(service: &Arc<Work>, id: &str, expected: &str) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let state = service.state.lock().await;
                let task = state.store.tasks.iter().find(|t| t.id == id).unwrap();
                if task.status == expected && !state.starts.contains_key(id) { break; }
                assert_ne!(task.status, "failed", "{:?}", task.error);
                drop(state);
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("Task did not settle");
    }
    #[tokio::test]
    async fn imported_continuation_runs_with_summary_and_survives_undo() {
        let root = temp();
        let folder = root.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        let data = root.join("data");
        let history = data.join("imports/history");
        std::fs::create_dir_all(&history).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let path = history.join(format!("{id}.json"));
        let messages: Vec<_> = (0..15).map(|i| json!({"role":if i % 2 == 0 {"user"} else {"assistant"},"text":format!("Synthetic message {i}")})).collect();
        config::write_json(&path, &json!({"source":"cowork","kind":"work","cwd":folder,"title":"Report","status":"stopped","messages":messages,"turns":[],"attachments":[]})).unwrap();
        let work = Work::new(data, Sink::default());
        let mut state = work.state.lock().await;
        work.initialize(&mut state).await.unwrap();
        state.settings.providers = vec![serde_json::from_value(json!({"id":"local","name":"Synthetic","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"test","enabled":true}]})).unwrap()];
        let imported = state.store.tasks[0].clone();
        let continuation = work.action(&mut state, "continue", json!({"taskId":imported.id,"model":"local/test"})).await.unwrap();
        assert_eq!(continuation["status"], "queued");
        assert_eq!(continuation["accessMode"], "ask");
        assert!(continuation["importId"].is_null());
        let brief = continuation["brief"].as_str().unwrap();
        assert!(brief.contains(&super::super::imports::continuation(&messages)));
        assert_eq!(state.store.tasks.iter().find(|t| t.id == imported.id).unwrap().status, "stopped");
        work.pump(&mut state).await;
        drop(state);
        wait_status(&work, continuation["id"].as_str().unwrap(), "running").await;
        let mut state = work.state.lock().await;
        assert_eq!(state.store.tasks[0].status, "running", "{:?}", state.store.tasks[0].error);
        assert!(state.store.tasks[0].thread_id.is_some());
        let session = state.sessions.get(&imported.space_id).unwrap().clone();
        let requests = std::fs::read_dir(work.data.join("pi/sessions").join(&imported.space_id)).unwrap()
            .map(|f| f.unwrap().path()).find(|p| p.extension().is_some_and(|e| e == "requests")).unwrap();
        let sent = std::fs::read_to_string(requests).unwrap();
        let prompt = sent.lines().map(|s| serde_json::from_str::<Value>(s).unwrap()).find(|v| v["type"] == "prompt").unwrap();
        assert!(prompt["message"].as_str().unwrap().contains(brief));
        std::fs::remove_file(path).unwrap();
        work.sync_imports(&mut state.store).unwrap();
        assert_eq!(state.store.tasks.len(), 1);
        assert_eq!(state.store.tasks[0].id, continuation["id"].as_str().unwrap());
        assert_eq!(state.store.spaces.len(), 1);
        work.action(&mut state, "stop-all", json!({})).await.unwrap();
        session.pi.shutdown().await.unwrap();
        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn global_queue_caps_concurrency_and_serializes_each_space() {
        let mut store = Store::default();
        store.tasks = vec![task("a", "one", "running"), task("b", "one", "queued"), task("c", "two", "queued")];
        assert_eq!(next_task(&store), Some(2));
        store.tasks[2].status = "needs-input".into();
        assert_eq!(next_task(&store), None);
        store.tasks[0].status = "done".into();
        assert_eq!(next_task(&store), Some(1));
        store.concurrency = 1;
        assert_eq!(next_task(&store), None);
        store.tasks[2].status = "stopped".into();
        assert_eq!(next_task(&store), Some(1));
    }
    #[tokio::test]
    async fn spaces_attachments_queue_and_restart_are_persisted_with_boundaries() {
        let root = temp();
        let folder = root.join("folder");
        let reference = root.join("reference");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&reference).unwrap();
        std::fs::create_dir(folder.join("outputs")).unwrap();
        std::fs::write(folder.join("outputs/existing.html"), "Existing dashboard").unwrap();
        std::fs::write(folder.join("notes.txt"), "Source notes").unwrap();
        let service = Work::new(root.join("data"), Sink::default());
        let settings = AppSettings::default();
        let space = service.request(&settings, "add-space", json!({"path":folder,"readOnlyFolders":[reference]})).await.unwrap();
        let id = space["id"].as_str().unwrap();
        assert_eq!(space["kind"], "work");
        let source = reference.join("brief.pdf");
        std::fs::write(&source, "synthetic PDF").unwrap();
        let attachment = service.attachment(&settings, id, &source).await.unwrap();
        assert!(attachment.starts_with(".taucode/uploads/"));
        assert_eq!(std::fs::read(folder.join(&attachment)).unwrap(), b"synthetic PDF");
        assert_ne!(attachment, service.attachment(&settings, id, &source).await.unwrap());
        assert!(!service.data.join("workspaces.json").exists());
        {
            let mut state = service.state.lock().await;
            state.store.tasks.push(task("queued", id, "queued"));
            state.store.tasks.push(task("running", id, "needs-input"));
            service.stop(&mut state, "queued").await.unwrap();
            assert_eq!(state.store.tasks[0].status, "stopped");
            service.save(&state.store).unwrap();
        }
        let restarted = Work::new(service.data.clone(), Sink::default());
        let list = restarted.request(&settings, "list", json!({})).await.unwrap();
        assert_eq!(list["spaces"][0]["readOnlyFolders"][0], json!(reference.canonicalize().unwrap()));
        assert_eq!(list["tasks"][1]["status"], "stopped");
        for path in ["outputs/existing.html", "notes.txt", attachment.as_str()] {
            assert_eq!(restarted.output(&settings, "queued", path).await.unwrap(), folder.join(path).canonicalize().unwrap());
        }
        assert!(restarted.state.lock().await.store.tasks[0].outputs.is_empty(), "Opening an existing file must not report a task change");
        for path in ["../reference/brief.pdf", source.to_str().unwrap(), "outputs", "missing.html"] {
            assert!(restarted.output(&settings, "queued", path).await.is_err(), "{path}");
        }
        assert!(restarted.output(&settings, "unknown-task", "notes.txt").await.is_err());
        assert!(restarted.request(&settings, "concurrency", json!({"count":0})).await.is_err());
        assert!(restarted.request(&settings, "create", json!({"spaceId":id,"brief":"x","model":"none","attachments":["../../escape"]})).await.is_err());
        #[cfg(unix)] {
            std::os::unix::fs::symlink(&source, folder.join("outputs/escape.pdf")).unwrap();
            assert!(restarted.output(&settings, "queued", "outputs/escape.pdf").await.is_err());
            let outside = root.join("outside");
            std::fs::create_dir(&outside).unwrap();
            std::fs::remove_dir_all(folder.join(".taucode")).unwrap();
            std::os::unix::fs::symlink(&outside, folder.join(".taucode")).unwrap();
            assert!(restarted.attachment(&settings, id, &source).await.is_err());
            assert!(!outside.join("uploads").exists());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn progress_approvals_outputs_checkpoint_diff_and_notifications() {
        let root = temp();
        let folder = root.join("space");
        std::fs::create_dir_all(folder.join("outputs")).unwrap();
        std::fs::write(folder.join("existing.txt"), "before").unwrap();
        let sink = Sink::default();
        let service = Work::new(root.join("data"), sink.clone());
        let space = Space { id: "space".into(), name: "Space".into(), path: folder.canonicalize().unwrap().to_string_lossy().into_owned(), kind: WorkspaceKind::Work, read_only_folders: vec![], read_only: false, imported: false };
        let mut state = service.state.lock().await;
        state.store.spaces.push(space.clone());
        state.store.tasks.push(task("thread", "space", "running"));
        let session = service.session(&mut state, &space).await;
        session.checkpoints.as_ref().unwrap().before("space", "thread", "pi", &json!({"input":[]})).await.unwrap();
        let event = |method: &str, params: Value| AppServerEvent { workspace_id: "space".into(), message: json!({"id":"pi:thread:dialog","method":method,"params":params}) };
        service.observe(&mut state, &event("item/fileChange/requestApproval", json!({"threadId":"thread","reason":"Overwrite"}))).await.unwrap();
        assert_eq!(state.store.tasks[0].status, "needs-input");
        assert_eq!(state.store.tasks[0].pending.len(), 1);
        assert!(sink.0.lock().unwrap().iter().any(|e| e.message["params"]["notify"] == true && e.message["params"]["needsInput"] == 1));
        service.observe(&mut state, &event("serverRequest/resolved", json!({"threadId":"thread","requestId":"pi:thread:dialog"}))).await.unwrap();
        assert_eq!(state.store.tasks[0].status, "running");
        assert!(state.store.tasks[0].pending.is_empty());
        service.observe(&mut state, &event("item/completed", json!({"threadId":"thread","item":{"type":"plan","plan":[{"step":"Produce brief","status":"completed"}]}}))).await.unwrap();
        assert_eq!(state.store.tasks[0].plan[0]["status"], "completed");
        std::fs::write(folder.join("outputs/report.md"), "report").unwrap();
        std::fs::write(folder.join("existing.txt"), "after").unwrap();
        std::fs::write(folder.join("outputs/from-shell.csv"), "shell-created").unwrap();
        service.observe(&mut state, &event("item/completed", json!({"threadId":"thread","item":{"type":"fileChange","status":"completed","changes":[{"path":"outputs/report.md"},{"path":"../escape"}]}}))).await.unwrap();
        assert_eq!(state.store.tasks[0].outputs, vec!["outputs/report.md"]);
        service.observe(&mut state, &event("turn/completed", json!({"threadId":"thread","turn":{"status":"completed"}}))).await.unwrap();
        assert_eq!(state.store.tasks[0].status, "done");
        assert_eq!(state.store.tasks[0].outputs, vec!["existing.txt", "outputs/from-shell.csv", "outputs/report.md"]);
        assert!(sink.0.lock().unwrap().iter().any(|e| e.message["params"]["notify"] == true && e.message["params"]["task"]["status"] == "done"));
        state.store.tasks[0].status = "running".into();
        service.observe(&mut state, &event("turn/completed", json!({"threadId":"thread","turn":{"status":"failed","error":{"message":"Provider failed"}}}))).await.unwrap();
        assert_eq!(state.store.tasks[0].status, "failed");
        assert!(sink.0.lock().unwrap().iter().any(|e| e.message["params"]["notify"] == true && e.message["params"]["task"]["status"] == "failed"));
        state.store.tasks[0].status = "stopped".into();
        service.observe(&mut state, &event("turn/completed", json!({"threadId":"thread","turn":{"status":"completed"}}))).await.unwrap();
        assert_eq!(state.store.tasks[0].status, "stopped");
        drop(state);
        assert!(service.output(&AppSettings::default(), "thread", "../escape").await.is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn waiting_for_credentials_keeps_work_responsive_and_cancel_prevents_late_start() {
        use std::sync::{Condvar, Mutex as StdMutex};
        struct WaitingKeys {
            entered: StdMutex<Option<oneshot::Sender<()>>>,
            gate: Arc<(StdMutex<bool>, Condvar)>,
            finished: StdMutex<Option<oneshot::Sender<()>>>,
        }
        impl super::super::providers::KeyStore for WaitingKeys {
            fn get(&self, _: &str) -> Result<Option<String>, String> {
                if let Some(tx) = self.entered.lock().unwrap().take() { let _ = tx.send(()); }
                let (lock, ready) = &*self.gate;
                let (_guard, timeout) = ready.wait_timeout_while(lock.lock().unwrap(), std::time::Duration::from_secs(3), |open| !*open).unwrap();
                assert!(!timeout.timed_out(), "Credential read blocked the async executor");
                if let Some(tx) = self.finished.lock().unwrap().take() { let _ = tx.send(()); }
                Ok(Some("synthetic-key".into()))
            }
            fn set(&self, _: &str, _: &str) -> Result<(), String> { unreachable!() }
            fn delete(&self, _: &str) -> Result<(), String> { unreachable!() }
        }
        let root = temp();
        let service = Work::new(root.join("data"), Sink::default());
        let folder = root.join("space"); std::fs::create_dir(&folder).unwrap();
        let mut settings = AppSettings::default();
        settings.providers = vec![serde_json::from_value(json!({"id":"local","name":"Synthetic","kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"test","enabled":true}]})).unwrap()];
        let space = service.request(&settings, "add-space", json!({"path":folder,"readOnlyFolders":[]})).await.unwrap();
        let id = space["id"].as_str().unwrap();
        let (entered, entering) = oneshot::channel();
        let (finished, finishing) = oneshot::channel();
        let gate = Arc::new((StdMutex::new(false), Condvar::new()));
        {
            let mut state = service.state.lock().await;
            let space = state.store.spaces[0].clone();
            service.session(&mut state, &space).await;
            Arc::get_mut(state.sessions.get_mut(id).unwrap()).unwrap().pi.keys = Arc::new(WaitingKeys { entered: StdMutex::new(Some(entered)), gate: gate.clone(), finished: StdMutex::new(Some(finished)) });
        }
        let created = service.request(&settings, "create", json!({"spaceId":id,"model":"local/test","brief":"Keep this unsent message","attachments":[],"runNow":true})).await.unwrap();
        let task_id = created["id"].as_str().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), entering).await.unwrap().unwrap();
        // A transcript read may wait on this session, but it must not lock Work.
        let (reader, read_settings, read_id) = (service.clone(), settings.clone(), task_id.to_owned());
        let reading = tokio::spawn(async move { reader.request(&read_settings, "read", json!({"taskId":read_id})).await });
        tokio::task::yield_now().await;
        let list = tokio::time::timeout(std::time::Duration::from_millis(250), service.request(&settings, "list", json!({}))).await.unwrap().unwrap();
        assert_eq!(list["tasks"][0]["status"], "starting");
        tokio::time::timeout(std::time::Duration::from_millis(250), service.request(&settings, "stop", json!({"taskId":task_id}))).await.unwrap().unwrap();
        wait_status(&service, task_id, "stopped").await;
        reading.await.unwrap().unwrap();
        *gate.0.lock().unwrap() = true; gate.1.notify_all();
        finishing.await.unwrap();
        tokio::task::yield_now().await;
        let list = service.request(&settings, "list", json!({})).await.unwrap();
        assert_eq!(list["tasks"][0]["status"], "stopped");
        assert_eq!(list["tasks"][0]["pendingMessage"], "Keep this unsent message");
        let thread = list["tasks"][0]["threadId"].as_str().unwrap();
        assert!(!service.data.join("pi/sessions").join(id).join(format!("{thread}.requests")).exists());
        assert!(!service.data.join("pi/agent").join(id).join("models.json").exists(), "Cancelled credential lookup must not write runtime files");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn queue_runs_two_pi_tasks_and_stop_releases_capacity() {
        let root = temp();
        let service = Work::new(root.join("data"), Sink::default());
        let mut state = service.state.lock().await;
        state.settings.providers = vec![serde_json::from_value(json!({"id":"local","name":"Synthetic","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"test","enabled":true}]})).unwrap()];
        for id in ["one", "two", "three"] {
            let path = root.join(id);
            std::fs::create_dir(&path).unwrap();
            state.store.spaces.push(Space { id:id.into(), name:id.into(), path:path.to_string_lossy().into_owned(), kind:WorkspaceKind::Work, read_only_folders:vec![], read_only: false, imported: false });
            let mut entry = task(id, id, "queued");
            entry.thread_id = None;
            state.store.tasks.push(entry);
        }
        service.pump(&mut state).await;
        drop(state);
        wait_status(&service, "one", "running").await;
        wait_status(&service, "two", "running").await;
        let mut state = service.state.lock().await;
        assert_eq!(state.store.tasks.iter().map(|t| t.status.as_str()).collect::<Vec<_>>(), vec!["running","running","queued"], "{:?}", state.store.tasks.iter().map(|t| &t.error).collect::<Vec<_>>());
        assert!(state.store.tasks[0].thread_id.is_some());
        assert!(state.store.tasks[1].thread_id.is_some());
        service.stop(&mut state, "one").await.unwrap();
        assert_eq!(state.store.tasks[0].status, "stopped");
        service.pump(&mut state).await;
        drop(state);
        wait_status(&service, "three", "running").await;
        let mut state = service.state.lock().await;
        assert_eq!(state.store.tasks[2].status, "running");
        service.action(&mut state, "stop-all", json!({})).await.unwrap();
        assert!(state.store.tasks.iter().all(|t| t.status == "stopped"));
        let thread = state.store.tasks[0].thread_id.clone();
        assert!(service.action(&mut state, "follow-up", json!({"taskId":"one","message":"  "})).await.is_err());
        state.store.tasks[0].status = "needs-input".into();
        assert!(service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Continue"})).await.unwrap_err().contains("pending request"));
        state.store.tasks[0].status = "stopped".into();
        let mut rival = task("rival", "one", "running");
        rival.thread_id = None;
        state.store.tasks.push(rival);
        assert!(service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Continue"})).await.unwrap_err().contains("available task slot"));
        state.store.tasks.pop();
        let mut backup = state.settings.providers[0].clone();
        backup.id = "backup".into();
        backup.models[0].reasoning = Some(serde_json::from_value(json!({"supported":true,"levels":["low","high"],"default":"low","control":"effort","source":"manual","detectedAt":0})).unwrap());
        state.settings.providers.push(backup);
        for invalid in [json!({"model":"missing/test"}), json!({"model":"backup/missing"}), json!({"model":7}), json!({"model":"backup/test","effort":"ultra"}), json!({"model":"backup/test","effort":4}), json!({"model":"local/test","effort":"low"})] {
            let mut params = json!({"taskId":"one","message":"Continue"});
            params.as_object_mut().unwrap().extend(invalid.as_object().unwrap().clone());
            assert!(service.action(&mut state, "follow-up", params).await.is_err());
            assert_eq!(state.store.tasks[0].status, "stopped");
            assert_eq!(state.store.tasks[0].model, "local/test");
            assert!(state.store.tasks[0].pending_message.is_none());
        }
        service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Check the output","accessMode":"full-access","model":"backup/test","effort":"low"})).await.unwrap();
        assert_eq!(state.store.tasks[0].thread_id, thread);
        assert_eq!(state.store.tasks[0].status, "queued");
        assert_eq!(state.store.tasks[0].access_mode, "full-access");
        assert_eq!(state.store.tasks[0].model, "backup/test");
        assert_eq!(state.store.tasks[0].effort.as_deref(), Some("low"));
        service.pump(&mut state).await;
        drop(state);
        wait_status(&service, "one", "running").await;
        let mut state = service.state.lock().await;
        assert!(service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Change now","model":"local/test"})).await.unwrap_err().contains("Stop this task"));
        assert!(service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Change now","effort":"high"})).await.unwrap_err().contains("Stop this task"));
        service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Also check keyboard controls"})).await.unwrap();
        assert_eq!(state.store.tasks.len(), 3);
        service.stop(&mut state, "one").await.unwrap();
        service.action(&mut state, "follow-up", json!({"taskId":"one","message":"Continue without reasoning","model":"local/test","effort":null})).await.unwrap();
        assert_eq!(state.store.tasks[0].model, "local/test");
        assert_eq!(state.store.tasks[0].effort, None);
        assert_eq!(state.store.tasks[0].thread_id, thread);
        for session in state.sessions.values() { session.pi.shutdown().await.unwrap(); }
        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }
}
