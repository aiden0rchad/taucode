pub(super) mod config;
mod process;
#[cfg(test)]
mod tests;
mod translate;

use super::{
    engines::EngineBackend,
    providers::{self, BoxFuture, KeyStore, Keychain},
};
use crate::{
    codegraph::CodegraphSetup,
    types::{AgentPolicy, ModelRef, ProviderEntry},
};
use process::Process;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    process::Command,
    sync::{broadcast, Mutex},
};
use translate::Translator;

type Sink = Arc<dyn Fn(&str, Value) + Send + Sync>;
struct Session {
    workspace: String,
    thread: Value,
    file: PathBuf,
    process: Option<Arc<Process>>,
    configuration: String,
}
pub(crate) struct PiEngine {
    data: Option<PathBuf>,
    pub paths: Mutex<HashMap<String, String>>,
    sessions: Mutex<HashMap<String, Session>>,
    registry: Mutex<HashMap<String, (Vec<ProviderEntry>, AgentPolicy)>>,
    events: broadcast::Sender<Value>,
    sink: Sink,
    graph: Option<CodegraphSetup>,
    idle: Duration,
    keys: Arc<dyn KeyStore>,
    #[cfg(test)]
    fake: bool,
}
impl Default for PiEngine {
    fn default() -> Self {
        Self {
            data: None,
            paths: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            registry: Mutex::new(HashMap::new()),
            events: broadcast::channel(1024).0,
            sink: Arc::new(|_, _| {}),
            graph: None,
            idle: Duration::from_secs(600),
            keys: Arc::new(Keychain),
            #[cfg(test)]
            fake: false,
        }
    }
}
impl PiEngine {
    pub fn new(
        data: PathBuf,
        workspace: String,
        cwd: String,
        graph: Option<CodegraphSetup>,
        sink: Sink,
    ) -> Self {
        Self {
            data: Some(data),
            paths: Mutex::new(HashMap::from([(workspace, cwd)])),
            graph,
            sink,
            ..Self::default()
        }
    }
    pub async fn is_running(&self, id: &str) -> bool {
        let sessions = self.sessions.lock().await;
        if let Some(process) = sessions.get(id).and_then(|s| s.process.as_ref()) {
            return process.running.load(Ordering::SeqCst) || process.translator.lock().await.active;
        }
        false
    }
    pub async fn rewind(&self, workspace: &str, id: &str, user_item: Option<&str>, leaf: Option<Value>) -> Result<Value, String> {
        // Load the session through the normal history path without sending a prompt.
        self.request(workspace, "thread/read", json!({"threadId":id})).await?;
        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(id).filter(|s| s.workspace == workspace).ok_or("Unknown pi session")?;
        if let Some(process) = &session.process {
            let _operation = process.operation.lock().await;
            if process.running.load(Ordering::SeqCst) || process.translator.lock().await.active { return Err("Finish or interrupt the pi turn before rewinding".into()); }
            process.shutdown().await?;
        }
        let previous = rewind_session(&session.file, id, user_item, leaf)?;
        session.process = None;
        session.configuration.clear();
        Ok(previous)
    }
    pub async fn configure(
        &self,
        workspace: &str,
        registry: Vec<ProviderEntry>,
        policy: AgentPolicy,
    ) {
        self.registry
            .lock()
            .await
            .insert(workspace.into(), (registry, policy));
    }
    pub async fn credentials_changed(&self) {
        for session in self.sessions.lock().await.values_mut() {
            session.configuration.clear();
        }
    }
    fn emit(&self, workspace: &str, event: Value) {
        let _ = self.events.send(event.clone());
        (self.sink)(workspace, event);
    }
    fn directory(&self, workspace: &str) -> Result<PathBuf, String> {
        if workspace.is_empty()
            || !workspace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid workspace ID".into());
        }
        Ok(self
            .data
            .as_ref()
            .ok_or("Pi runtime is not configured")?
            .join("pi/sessions")
            .join(workspace))
    }
    pub async fn owns(&self, workspace: &str, id: &str) -> bool {
        if self
            .sessions
            .lock()
            .await
            .get(id)
            .is_some_and(|s| s.workspace == workspace)
        {
            return true;
        }
        self.scan(workspace)
            .await
            .unwrap_or_default()
            .iter()
            .any(|t| t["id"] == id)
    }
    async fn scan(&self, workspace: &str) -> Result<Vec<Value>, String> {
        if self.data.is_none() {
            return Ok(vec![]);
        }
        let dir = self.directory(workspace)?;
        let entries = match std::fs::read_dir(dir) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let registry = self
            .registry
            .lock()
            .await
            .get(workspace)
            .cloned()
            .unwrap_or_default()
            .0;
        let mut sessions = self.sessions.lock().await;
        let mut threads = vec![];
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            match read_session(&path, &registry) {
                Ok((mut thread, _)) => {
                    let id = thread["id"]
                        .as_str()
                        .ok_or("Pi session missing ID")?
                        .to_owned();
                    thread["turns"] = json!([]);
                    if let Some(s) = sessions.get_mut(&id) {
                        if s.process
                            .as_ref()
                            .is_none_or(|p| !p.alive.load(Ordering::SeqCst))
                        {
                            s.thread = thread.clone();
                        } else {
                            for key in ["modelProvider", "model", "forkedFromId"] {
                                thread[key] = s.thread[key].clone();
                            }
                            if s.process
                                .as_ref()
                                .is_some_and(|p| p.running.load(Ordering::SeqCst))
                            {
                                thread["status"] = json!({"type":"active","activeFlags":[]});
                            }
                            s.thread = thread.clone();
                            thread["turns"] = json!([]);
                        }
                    } else {
                        sessions.insert(
                            id,
                            Session {
                                workspace: workspace.into(),
                                thread: thread.clone(),
                                file: path,
                                process: None,
                                configuration: String::new(),
                            },
                        );
                    }
                    threads.push(thread);
                }
                Err(e) => eprintln!("pi session {}: {e}", path.display()),
            }
        }
        threads.sort_by_key(|t| std::cmp::Reverse(t["updatedAt"].as_i64().unwrap_or(0)));
        Ok(threads)
    }
    fn model<'a>(
        registry: &'a [ProviderEntry],
        policy: &AgentPolicy,
        value: Option<&Value>,
        current: Option<&Value>,
    ) -> Result<(&'a ProviderEntry, String), String> {
        let selected = match value.filter(|v| !v.is_null()) {
            Some(v) if v.is_object() => serde_json::from_value::<ModelRef>(v.clone())
                .map_err(|_| "Invalid model reference")?,
            Some(v) => {
                let (p, m) = v
                    .as_str()
                    .and_then(|s| s.split_once('/'))
                    .ok_or("Select a registry model (provider/model)")?;
                ModelRef {
                    provider_id: p.into(),
                    model_id: m.into(),
                }
            }
            None => current
                .and_then(|t| {
                    Some(ModelRef {
                        provider_id: t["modelProvider"].as_str()?.into(),
                        model_id: t["model"].as_str()?.into(),
                    })
                })
                .or_else(|| policy.default_model.clone())
                .or_else(|| {
                    registry.iter().filter(|p| p.enabled).find_map(|p| {
                        p.models.iter().find(|m| m.enabled).map(|m| ModelRef {
                            provider_id: p.id.clone(),
                            model_id: m.id.clone(),
                        })
                    })
                })
                .ok_or("Add and enable a provider and model in Settings")?,
        };
        let provider = registry
            .iter()
            .find(|p| {
                p.enabled
                    && p.id == selected.provider_id
                    && p.models
                        .iter()
                        .any(|m| m.enabled && m.id == selected.model_id)
            })
            .ok_or("Enable this registry provider and model in Settings")?;
        Ok((provider, selected.model_id))
    }
    async fn spawn(
        &self,
        s: &mut Session,
        registry: &[ProviderEntry],
        policy: &AgentPolicy,
    ) -> Result<Arc<Process>, String> {
        let connectors = super::connectors::load(
            self.data.as_ref().ok_or("Pi runtime is not configured")?,
            &s.workspace,
            self.graph.as_ref(),
        )?;
        let fingerprint =
            serde_json::to_string(&(registry, &connectors)).map_err(|e| e.to_string())?;
        if let Some(p) = &s.process {
            if p.alive.load(Ordering::SeqCst) && s.configuration == fingerprint {
                return Ok(p.clone());
            }
            if p.alive.load(Ordering::SeqCst) {
                if p.running.load(Ordering::SeqCst) || p.translator.lock().await.active {
                    return Err("Provider settings changed. Finish or interrupt the running pi turn, then retry.".into());
                }
                p.shutdown().await?;
            }
        }
        let (provider, model) = Self::model(registry, policy, None, Some(&s.thread))?;
        let home = self
            .data
            .as_ref()
            .ok_or("Pi runtime is not configured")?
            .join("pi/agent")
            .join(&s.workspace);
        let mut env = config::prepare(&home, registry, self.graph.as_ref(), self.keys.as_ref())?;
        for p in registry.iter().filter(|p| p.enabled) {
            if matches!(p.auth, crate::types::ProviderAuth::Command { .. }) {
                if let Some(key) = providers::token(p, self.keys.as_ref()).await? {
                    env.insert(super::engines::key_env(&p.id), key);
                }
            }
        }
        let delivery = super::connectors::delivery(&connectors, self.keys.as_ref())?;
        config::write_json(&home.join("mcp.json"), &delivery.pi)?;
        config::write_json(&home.join("taucode/connectors.json"), &json!(connectors))?;
        env.extend(delivery.env);
        let mut secrets = delivery.secrets;
        for p in registry.iter().filter(|p| p.enabled) {
            if let Some(value) = env.get(&super::engines::key_env(&p.id)) {
                secrets.push(value.clone());
            }
        }
        let id = s.thread["id"].as_str().ok_or("Pi session missing ID")?;
        config::write_policy(&home, registry, policy)?;
        config::write_access(&home, id, None)?;
        env.insert("TAUCODE_SESSION_KEY".into(), id.into());
        env.insert(
            "TAUCODE_WORKSPACE".into(),
            s.thread["cwd"].as_str().ok_or("Missing cwd")?.into(),
        );
        let node = std::env::var("TAUCODE_NODE").unwrap_or_else(|_| "node".into());
        #[cfg(test)]
        let node = if self.fake { "python3".into() } else { node };
        #[cfg(test)]
        if !self.fake {
            super::engine_runtime::check_node(std::ffi::OsStr::new(&node)).await?;
        }
        let runtime = {
            #[cfg(test)]
            if self.fake { None } else { Some(super::engine_runtime::source("pi", None)?) }
            #[cfg(not(test))]
            Some(super::engine_runtime::prepare(self.data.as_ref().unwrap(), "pi", None).await?)
        };
        let cli = std::env::var_os("TAUCODE_PI_CLI").map(PathBuf::from)
            .unwrap_or_else(|| runtime.as_ref().map(|dir| dir.join("node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js")).unwrap_or_default());
        #[cfg(test)]
        let cli = if self.fake {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_pi.py")
        } else {
            cli
        };
        if !cli.is_file() {
            return Err(
                "Pi runtime missing. Install engines/pi or set TAUCODE_PI_CLI to pi 1.4.0 cli.js"
                    .into(),
            );
        }
        let mut command = Command::new(node);
        let guard = runtime.as_ref().map(|dir| dir.join("no-accounts.mjs")).unwrap_or_default();
        #[cfg(test)]
        if !self.fake {
            command.arg("--import").arg(&guard).env("PI_OFFLINE", "1");
        }
        #[cfg(not(test))]
        command.arg("--import").arg(&guard);
        command
            .env("TAUCODE_PI_CLI", &cli)
            .env_remove("TAUCODE_SUBAGENT_DEPTH")
            .env_remove("TAUCODE_TOOL_ACCESS")
            .arg(cli)
            .arg("-e")
            .arg(home.join("extensions/taucode-gate.ts"))
            .arg("-e")
            .arg(home.join("extensions/taucode-subagents.ts"))
            .args([
                "--no-approve",
                "--no-extensions",
                "-e",
                "builtin:mcp",
                "--mode",
                "rpc",
                "--provider",
                &config::provider_id(provider, registry),
                "--model",
                &model,
                "--session-dir",
            ])
            .arg(self.directory(&s.workspace)?)
            .arg("--session")
            .arg(&s.file)
            .current_dir(s.thread["cwd"].as_str().ok_or("Pi session missing cwd")?)
            .envs(env);
        let (_, history) = read_session(&s.file, registry)?;
        let mut translator = history;
        translator.connectors = connectors
            .iter()
            .map(|s| (s.id.replace('-', "_"), s.name.clone()))
            .collect();
        translator.access_mode = serde_json::from_slice::<Value>(
            &std::fs::read(home.join("taucode/threads").join(format!("{id}.json")))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?["accessMode"]
            .as_str()
            .unwrap_or("ask")
            .into();
        translator.provider = provider.id.clone();
        translator.model = json!({"id":model,"provider":config::provider_id(provider,registry),"contextWindow":provider.models.iter().find(|m|m.id==model).and_then(|m|m.context_window)});
        let events = self.events.clone();
        let sink = self.sink.clone();
        let workspace = s.workspace.clone();
        let emit = Arc::new(move |e: Value| {
            let _ = events.send(e.clone());
            sink(&workspace, e);
        });
        let p = Process::spawn(
            command,
            translator,
            emit,
            self.graph.clone(),
            self.idle,
            secrets,
        )
        .await?;
        let state = match p.request(json!({"type":"get_state"})).await {
            Ok(v) => v,
            Err(e) => {
                let _ = p.shutdown().await;
                return Err(e);
            }
        };
        if state["sessionId"] != id {
            let _ = p.shutdown().await;
            return Err("Pi resumed an unexpected session ID".into());
        }
        let require_extensions = {
            #[cfg(test)]
            {
                !self.fake
            }
            #[cfg(not(test))]
            {
                true
            }
        };
        if require_extensions && p.extensions.lock().await.len() != 2 {
            let _ = p.shutdown().await;
            return Err("Pi policy extensions did not load; refusing to run without the approval gate and subagent policy".into());
        }
        if state["model"]["id"] != model
            || state["model"]["provider"] != config::provider_id(provider, registry)
        {
            let _ = p.shutdown().await;
            return Err("Pi did not select the requested registry model; check provider configuration in Settings".into());
        }
        p.translator.lock().await.model = state["model"].clone();
        s.configuration = fingerprint;
        s.process = Some(p.clone());
        Ok(p)
    }
    async fn dispatch(
        &self,
        workspace: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        if method == "thread/list" {
            let data = self.scan(workspace).await?;
            let (data, next) = page(data, &params)?;
            return Ok(
                json!({"result":{"data":data,"nextCursor":next,"backwardsCursor":null,"readStates":null}}),
            );
        }
        let (registry, policy) = self
            .registry
            .lock()
            .await
            .get(workspace)
            .cloned()
            .unwrap_or_default();
        if let Some(data) = &self.data {
            let home = data.join("pi/agent").join(workspace);
            if home.exists() {
                config::write_policy(&home, &registry, &policy)?;
            }
        }
        if method == "model/list" {
            return Ok(providers::catalog(&registry));
        }
        if method == "thread/start" {
            let (provider, model) = Self::model(&registry, &policy, params.get("model"), None)?;
            let cwd = params["cwd"]
                .as_str()
                .map(str::to_owned)
                .or(self.paths.lock().await.get(workspace).cloned())
                .ok_or("Missing pi workspace path")?;
            let dir = self.directory(workspace)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let id = uuid::Uuid::new_v4().to_string();
            let file = dir.join(format!("{id}.jsonl"));
            // A valid pi session header fixes the ID before MCP children start, so
            // TAUCODE_SESSION_KEY always matches pi's sessionId, including on resume.
            let header = json!({"type":"session","version":3,"id":id,"timestamp":chrono::Utc::now().to_rfc3339(),"cwd":cwd});
            std::fs::write(&file, format!("{header}\n")).map_err(|e| e.to_string())?;
            let mut thread = thread_value(&id, &cwd, &file);
            thread["modelProvider"] = json!(provider.id);
            thread["model"] = json!(model);
            let mut session = Session {
                workspace: workspace.into(),
                thread: thread.clone(),
                file,
                process: None,
                configuration: String::new(),
            };
            if let Err(e) = self.spawn(&mut session, &registry, &policy).await {
                let _ = std::fs::remove_file(&session.file);
                return Err(e);
            }
            self.sessions.lock().await.insert(id, session);
            self.emit(
                workspace,
                json!({"method":"thread/started","params":{"thread":thread}}),
            );
            return Ok(json!({"result":thread_response(thread, false, "ask")}));
        }
        let id = params["threadId"].as_str().ok_or("Missing thread ID")?;
        if !self.sessions.lock().await.contains_key(id) {
            self.scan(workspace).await?;
        }
        let mut sessions = self.sessions.lock().await;
        let s = sessions
            .get_mut(id)
            .filter(|s| s.workspace == workspace)
            .ok_or("Unknown pi thread")?;
        if method == "thread/accessMode/set" || method == "turn/start" {
            let mode = params["accessMode"].as_str().or_else(|| {
                match params["sandboxPolicy"]["type"].as_str() {
                    Some("dangerFullAccess") => Some("full-access"),
                    Some("readOnly") => Some("read-only"),
                    Some("workspaceWrite") => Some("ask"),
                    _ => None,
                }
            });
            if method == "thread/accessMode/set" && mode.is_none() {
                return Err("Missing access mode".into());
            }
            let home = self
                .data
                .as_ref()
                .ok_or("Pi runtime is not configured")?
                .join("pi/agent")
                .join(workspace);
            config::write_access(&home, id, mode)?;
            if let (Some(p), Some(mode)) = (&s.process, mode) {
                p.translator.lock().await.access_mode =
                    if mode == "current" { "ask" } else { mode }.into();
            }
            if method == "thread/accessMode/set" {
                return Ok(json!({"result":{}}));
            }
        }
        if matches!(
            method,
            "thread/read" | "thread/turns/list" | "thread/items/list"
        ) {
            let mut thread = s.thread.clone();
            let file = s.file.clone();
            let p = s.process.clone();
            drop(sessions);
            let mut history = if let Some(p) = p.filter(|p| p.alive.load(Ordering::SeqCst)) {
                let messages = p.request(json!({"type":"get_messages"})).await?;
                let t = p.translator.lock().await;
                let fresh = Translator::history(
                    id,
                    &t.cwd,
                    messages["messages"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                )
                .with_connectors(t.connectors.clone())
                .turns;
                if !t.turns.is_empty() {
                    // get_messages omits compacted history. Refresh matching items
                    // while retaining observed prompt boundaries and older turns.
                    let items: HashMap<_, _> = fresh
                        .iter()
                        .flat_map(|turn| turn["items"].as_array().into_iter().flatten())
                        .filter_map(|item| item["id"].as_str().map(|id| (id, item)))
                        .collect();
                    let mut turns = t.turns.clone();
                    for turn in turns
                        .iter_mut()
                        .filter(|turn| turn["status"] != "inProgress")
                    {
                        for item in turn["items"].as_array_mut().unwrap() {
                            if let Some(updated) = item["id"].as_str().and_then(|id| items.get(id))
                            {
                                *item = (*updated).clone();
                            }
                        }
                    }
                    turns
                } else {
                    fresh
                }
            } else {
                read_session(&file, &registry)?.1.turns
            };
            if method == "thread/items/list" {
                let turn = history
                    .iter()
                    .find(|t| t["id"] == params["turnId"])
                    .ok_or("Unknown pi turn")?;
                let mut items = turn["items"].as_array().cloned().unwrap_or_default();
                if params["sortDirection"] == "desc" {
                    items.reverse();
                }
                let (items, next) = page(items, &params)?;
                return Ok(
                    json!({"result":{"data":items.into_iter().map(|item|json!({"item":item,"turnId":params["turnId"],"startedAtMs":null,"completedAtMs":null})).collect::<Vec<_>>(),"nextCursor":next,"backwardsCursor":null}}),
                );
            }
            if method == "thread/turns/list" {
                if params["sortDirection"].as_str() != Some("asc") {
                    history.reverse();
                }
                if params["itemsView"] == "notLoaded" {
                    for turn in &mut history {
                        turn["items"] = json!([]);
                        turn["itemsView"] = json!("notLoaded");
                    }
                }
                let (data, next) = page(history, &params)?;
                return Ok(
                    json!({"result":{"data":data,"nextCursor":next,"backwardsCursor":null}}),
                );
            }
            thread["turns"] = if params["includeTurns"] == false {
                json!([])
            } else {
                json!(history)
            };
            return Ok(json!({"result":{"thread":thread,"readState":null}}));
        }
        // Interrupt must remain available after credentials/configuration change.
        let p = if matches!(method, "turn/interrupt" | "turn/steer") {
            match &s.process {
                Some(p) if p.alive.load(Ordering::SeqCst) => p.clone(),
                _ => self.spawn(s, &registry, &policy).await?,
            }
        } else {
            self.spawn(s, &registry, &policy).await?
        };
        let thread = s.thread.clone();
        drop(sessions);
        let _operation = if matches!(method, "turn/interrupt" | "turn/steer") {
            None
        } else {
            Some(p.operation.lock().await)
        };
        if method == "thread/fork" {
            if p.running.load(Ordering::SeqCst) {
                return Err("Finish or interrupt the pi turn before forking".into());
            }
            // clone switches the RPC process to its copy. Stop it and reopen each
            // session with its own TAUCODE_SESSION_KEY before running another turn.
            let clone = p.request(json!({"type":"clone"})).await?;
            if clone["cancelled"] == true {
                return Err("Pi clone cancelled".into());
            }
            let state = p.request(json!({"type":"get_state"})).await?;
            p.shutdown().await?;
            let file = PathBuf::from(
                state["sessionFile"]
                    .as_str()
                    .ok_or("Pi clone missing session file")?,
            );
            if !file.starts_with(self.directory(workspace)?) {
                return Err("Pi clone returned a session outside its workspace".into());
            }
            let (mut cloned, _) = read_session(&file, &registry)?;
            cloned["forkedFromId"] = json!(id);
            let (provider, model) =
                Self::model(&registry, &policy, params.get("model"), Some(&thread))?;
            cloned["modelProvider"] = json!(provider.id);
            cloned["model"] = json!(model);
            let new_id = cloned["id"]
                .as_str()
                .ok_or("Pi clone missing ID")?
                .to_owned();
            let mut session = Session {
                workspace: workspace.into(),
                thread: cloned.clone(),
                file,
                process: None,
                configuration: String::new(),
            };
            self.spawn(&mut session, &registry, &policy).await?;
            self.sessions.lock().await.insert(new_id, session);
            let mut notification = cloned.clone();
            notification["turns"] = json!([]);
            self.emit(
                workspace,
                json!({"method":"thread/started","params":{"thread":notification}}),
            );
            return Ok(json!({"result":thread_response(cloned, false, "ask")}));
        }
        if method == "turn/start" {
            if p.running.load(Ordering::SeqCst) || p.translator.lock().await.active {
                return Err("Pi is running; steer or interrupt the active turn".into());
            }
            let (provider, model) =
                Self::model(&registry, &policy, params.get("model"), Some(&thread))?;
            if thread["modelProvider"] != provider.id || thread["model"] != model {
                let actual=p.request(json!({"type":"set_model","provider":config::provider_id(provider,&registry),"modelId":model})).await?;
                if actual["id"] != model
                    || actual["provider"] != config::provider_id(provider, &registry)
                {
                    p.shutdown().await?;
                    return Err("Pi did not select the requested registry model".into());
                }
                let mut t = p.translator.lock().await;
                t.provider = provider.id.clone();
                let events = t.apply(
                    &json!({"type":"response","command":"set_model","success":true,"data":actual}),
                );
                drop(t);
                if let Some(s) = self.sessions.lock().await.get_mut(id) {
                    s.thread["modelProvider"] = json!(provider.id);
                    s.thread["model"] = json!(model);
                }
                for event in events {
                    self.emit(workspace, event);
                }
            }
            let (message, images) = prompt_input(&params["input"])?;
            let turn_id = {
                let t = p.translator.lock().await;
                format!("{}:turn:{}", id, t.turns.len() + 1)
            };
            p.running.store(true, Ordering::SeqCst);
            if let Err(e) = p
                .request(json!({"type":"prompt","message":message,"images":images}))
                .await
            {
                p.running.store(false, Ordering::SeqCst);
                return Err(e);
            }
            return Ok(
                json!({"result":{"turn":{"id":turn_id,"rootTurnId":null,"items":[],"itemsView":"full","status":"inProgress","error":null,"startedAt":null,"completedAt":null,"durationMs":null}}}),
            );
        }
        let result = match method {
            "thread/resume" => {
                let mut thread = thread;
                thread["turns"] = json!(p.translator.lock().await.turns);
                thread_response(thread, true, &p.translator.lock().await.access_mode)
            }
            "turn/steer" => {
                let (message, images) = prompt_input(&params["input"])?;
                let turn_id = {
                    let t = p.translator.lock().await;
                    if !p.running.load(Ordering::SeqCst) {
                        return Err("No running pi turn to steer".into());
                    }
                    if t.active {
                        t.turn_id()
                    } else {
                        format!("{id}:turn:{}", t.turns.len() + 1)
                    }
                };
                if params["expectedTurnId"]
                    .as_str()
                    .is_some_and(|expected| expected != turn_id)
                {
                    return Err("The pi turn changed; retry steering the current turn".into());
                }
                p.request(json!({"type":"steer","message":message,"images":images}))
                    .await?;
                json!({"turnId":turn_id})
            }
            "turn/interrupt" => {
                p.request(json!({"type":"abort"})).await?;
                json!({})
            }
            "thread/compact/start" => {
                match p.request(json!({"type":"compact"})).await {
                    Err(error) if error == "Nothing to compact (session too small)" => (),
                    result => { result?; }
                }
                json!({})
            }
            "thread/name/set" => {
                let name = params["name"].as_str().ok_or("Missing thread name")?;
                p.request(json!({"type":"set_session_name","name":name}))
                    .await?;
                if let Some(s) = self.sessions.lock().await.get_mut(id) {
                    s.thread["name"] = json!(name);
                }
                self.emit(workspace,json!({"method":"thread/name/updated","params":{"threadId":id,"threadName":name}}));
                json!({})
            }
            _ => return Err(format!("Pi does not support {method}")),
        };
        Ok(json!({"result":result}))
    }
}
impl EngineBackend for PiEngine {
    fn request<'a>(
        &'a self,
        workspace: &'a str,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(self.dispatch(workspace, method, params))
    }
    fn respond_to_server_request(
        &self,
        id: Value,
        result: Value,
    ) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let id = id.as_str().ok_or("Invalid pi request ID")?;
            let thread = id
                .strip_prefix("pi:")
                .and_then(|s| s.split_once(':'))
                .map(|(t, _)| t)
                .ok_or("Invalid pi request ID")?;
            let p = self
                .sessions
                .lock()
                .await
                .get(thread)
                .and_then(|s| s.process.clone())
                .ok_or("Pi question expired")?;
            p.answer(id, result).await
        })
    }
    fn events(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }
    fn shutdown(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async {
            let processes = self
                .sessions
                .lock()
                .await
                .values_mut()
                .filter_map(|s| s.process.take())
                .collect::<Vec<_>>();
            for p in processes {
                p.shutdown().await?;
            }
            Ok(())
        })
    }
}

fn page(data: Vec<Value>, params: &Value) -> Result<(Vec<Value>, Option<String>), String> {
    let start = params["cursor"]
        .as_str()
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| "Invalid pi history cursor")?
        .unwrap_or(0);
    let limit = params["limit"].as_u64().unwrap_or(100).clamp(1, 1000) as usize;
    let end = start.saturating_add(limit).min(data.len());
    Ok((
        data.get(start..end).unwrap_or(&[]).to_vec(),
        (end < data.len()).then(|| end.to_string()),
    ))
}
fn thread_value(id: &str, cwd: &str, file: &Path) -> Value {
    let now = chrono::Utc::now().timestamp();
    json!({"id":id,"engine":"pi","sessionId":id,"environments":null,"extra":null,"forkedFromId":null,"parentThreadId":null,"preview":"","ephemeral":false,"section":null,"sectionEnteredAt":null,"projectId":null,"historyMode":"paginated","modelProvider":"","model":null,"reasoningEffort":null,"createdAt":now,"updatedAt":now,"recencyAt":now,"status":{"type":"idle"},"path":file,"cwd":cwd,"cliVersion":"1.4.0","originator":"taucode","source":{"custom":"taucode-pi"},"canAcceptDirectInput":true,"threadSource":null,"agentNickname":null,"agentRole":null,"gitInfo":null,"name":null,"daybreakEnabled":null,"turns":[]})
}
fn thread_response(thread: Value, resume: bool, mode: &str) -> Value {
    let mut response = json!({"model":thread["model"],"modelProvider":thread["modelProvider"],"cwd":thread["cwd"],"runtimeWorkspaceRoots":[thread["cwd"]],"instructionSources":[],"disabledPluginIds":[],"serviceTier":null,"approvalPolicy":if mode == "full-access" {"never"}else{"on-request"},"approvalsReviewer":"user","sandbox":translate::sandbox(mode,thread["cwd"].as_str().unwrap_or("")),"activePermissionProfile":null,"reasoningEffort":null,"multiAgentMode":"explicitRequestOnly","thread":thread});
    if resume {
        for key in [
            "collaborationMode",
            "initialTurnsPage",
            "turnsBackwardsCursor",
            "itemsBackwardsCursor",
        ] {
            response[key] = Value::Null;
        }
    }
    response
}
fn read_session(file: &Path, registry: &[ProviderEntry]) -> Result<(Value, Translator), String> {
    let bytes = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let mut entries = vec![];
    for line in bytes.split_inclusive('\n').filter(|s| !s.trim().is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) if !line.ends_with('\n') => break, // A running writer or interrupted final append.
            Err(e) => return Err(format!("Invalid pi session: {e}")),
        }
    }
    let header = entries
        .first()
        .filter(|v| v["type"] == "session")
        .ok_or("Missing pi session header")?;
    let id = header["id"].as_str().ok_or("Missing pi session ID")?;
    let cwd = header["cwd"].as_str().ok_or("Missing pi cwd")?;
    let mut thread = thread_value(id, cwd, file);
    if let Some(time) = header["timestamp"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
    {
        thread["createdAt"] = json!(time.timestamp());
    }
    if let Ok(time) = file.metadata().and_then(|m| m.modified()) {
        let n = time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        thread["updatedAt"] = json!(n);
        thread["recencyAt"] = json!(n);
    }
    // Follow parent IDs so abandoned branches are never shown as current history.
    let by_id: HashMap<_, _> = entries
        .iter()
        .skip(1)
        .filter_map(|e| e["id"].as_str().map(|id| (id, e)))
        .collect();
    let mut branch = vec![];
    let mut next = entries.last().and_then(|e| e["id"].as_str());
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = next {
        if !seen.insert(id) {
            return Err("Cyclic pi session history".into());
        }
        let Some(e) = by_id.get(id) else {
            break;
        };
        branch.push(*e);
        next = e["parentId"].as_str();
    }
    branch.reverse();
    let mut messages = vec![];
    for entry in branch {
        match entry["type"].as_str() {
            Some("message") => {
                let m = &entry["message"];
                if m["role"] == "user" && thread["preview"] == "" {
                    thread["preview"] = json!(translate::text(&m["content"]));
                }
                messages.push(m.clone());
            }
            Some("model_change") => {
                let pi = entry["provider"].as_str().unwrap_or("");
                thread["modelProvider"] = json!(registry
                    .iter()
                    .find(|p| config::provider_id(p, registry) == pi)
                    .map(|p| p.id.as_str())
                    .unwrap_or(pi));
                thread["model"] = entry["modelId"].clone();
            }
            Some("session_info") => thread["name"] = entry["name"].clone(),
            Some("compaction") => messages.push(json!({"role":"taucodeCompaction","result":entry})),
            _ => (),
        }
    }
    let mut translator = Translator::history(id, cwd, &messages);
    if let Some(workspace_dir) = file.parent() {
        if let (Some(workspace), Some(data)) = (
            workspace_dir.file_name().and_then(|s| s.to_str()),
            workspace_dir
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent),
        ) {
            let connectors = super::connectors::load(data, workspace, None)?;
            translator = translator.with_connectors(
                connectors
                    .into_iter()
                    .map(|s| (s.id.replace('-', "_"), s.name))
                    .collect(),
            );
        }
    }
    thread["turns"] = json!(translator.turns);
    Ok((thread, translator))
}
fn prompt_input(input: &Value) -> Result<(String, Vec<Value>), String> {
    use base64::Engine;
    let parts = input.as_array().ok_or("Pi input must be an array")?;
    let mut texts = vec![];
    let mut images = vec![];
    for part in parts {
        match part["type"].as_str() {
            Some("text") => texts.push(
                part["text"]
                    .as_str()
                    .ok_or("Invalid input text")?
                    .to_owned(),
            ),
            Some("image") => {
                let url = part["url"].as_str().ok_or("Missing image URL")?;
                let (header, data) = url
                    .strip_prefix("data:")
                    .and_then(|s| s.split_once(";base64,"))
                    .ok_or("Pi images require a base64 data URL or localImage")?;
                base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| "Invalid base64 image")?;
                images.push(json!({"type":"image","mimeType":header,"data":data}));
            }
            Some("localImage") => {
                let path = Path::new(part["path"].as_str().ok_or("Missing image path")?);
                let mime = match path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .as_str()
                {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "webp" => "image/webp",
                    "gif" => "image/gif",
                    _ => return Err("Unsupported pi image format".into()),
                };
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                images.push(json!({"type":"image","mimeType":mime,"data":base64::engine::general_purpose::STANDARD.encode(bytes)}));
            }
            _ => return Err("Pi supports text and image input".into()),
        }
    }
    Ok((texts.join("\n"), images))
}

// Appending a custom entry moves the active tree leaf while retaining every old
// entry. Pi's session manager and read_session both follow the last entry's parent.
fn rewind_session(file: &Path, thread: &str, user_item: Option<&str>, leaf: Option<Value>) -> Result<Value, String> {
    use std::io::Write;
    let bytes = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let entries: Vec<Value> = bytes.lines().filter(|s| !s.is_empty()).map(serde_json::from_str).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let previous = entries.last().filter(|e| e["type"] != "session").map(|e| e["id"].clone()).unwrap_or(Value::Null);
    let by_id: HashMap<_, _> = entries.iter().filter_map(|e| e["id"].as_str().map(|id| (id, e))).collect();
    let parent = if let Some(leaf) = leaf {
        if !leaf.is_null() && !leaf.as_str().is_some_and(|id| by_id.contains_key(id)) { return Err("Unknown pi undo leaf".into()); }
        leaf
    } else {
        let user_item = user_item.ok_or("Missing pi user item")?;
        let mut next = previous.as_str();
        let mut seen = std::collections::HashSet::new();
        let mut parent = None;
        while let Some(id) = next {
            if !seen.insert(id) { return Err("Cyclic pi session history".into()); }
            let entry = by_id.get(id).ok_or("Missing pi session entry")?;
            if entry["type"] == "message" && entry["message"]["role"] == "user" && format!("{}:user:{}", thread, entry["message"]["timestamp"]) == user_item {
                parent = Some(entry["parentId"].clone()); break;
            }
            next = entry["parentId"].as_str();
        }
        parent.ok_or("Message is not on the active pi branch")?
    };
    let marker = json!({"type":"custom","customType":"taucode-rewind","id":uuid::Uuid::new_v4().to_string(),"parentId":parent,"timestamp":chrono::Utc::now().to_rfc3339(),"data":{}});
    let mut output = std::fs::OpenOptions::new().append(true).open(file).map_err(|e| e.to_string())?;
    writeln!(output, "{}{}", if bytes.ends_with('\n') { "" } else { "\n" }, marker).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    Ok(previous)
}
