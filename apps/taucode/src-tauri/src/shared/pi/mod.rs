pub(super) mod config;
pub(crate) mod agents;
mod process;
mod preview;
mod profile;
mod recovery;
#[cfg(test)]
mod tests;
mod translate;

use super::{
    engines::EngineBackend,
    providers::{self, BoxFuture, KeyStore, Keychain},
};
use crate::{
    codegraph::CodegraphSetup,
    types::{AgentPolicy, ModelRef, ProviderAuth, ProviderEntry, ProviderKind},
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
// A cancelled startup must not leave a child alive before it joins the session.
struct StartingProcess(Option<Arc<Process>>);
impl Drop for StartingProcess {
    fn drop(&mut self) {
        if let Some(process) = self.0.take() {
            tokio::spawn(async move { let _ = process.shutdown().await; });
        }
    }
}
pub(crate) struct PiEngine {
    pub(super) data: Option<PathBuf>,
    profile: profile::EngineProfile,
    chat: bool,
    work: bool,
    pub previews: Option<Arc<super::managed_tasks::ManagedTasks>>,
    pub read_only_folders: Mutex<HashMap<String, Vec<String>>>,
    pub paths: Mutex<HashMap<String, String>>,
    sessions: Mutex<HashMap<String, Session>>,
    registry: Mutex<HashMap<String, (Vec<ProviderEntry>, AgentPolicy)>>,
    events: broadcast::Sender<Value>,
    sink: Sink,
    graph: Option<CodegraphSetup>,
    idle: Duration,
    pub(super) keys: Arc<dyn KeyStore>,
    #[cfg(test)]
    pub(super) fake: bool,
    #[cfg(test)]
    fixture_transport: bool,
}
impl Default for PiEngine {
    fn default() -> Self {
        Self {
            data: None,
            profile: Default::default(),
            chat: false,
            work: false,
            previews: None,
            read_only_folders: Mutex::new(HashMap::new()),
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
            #[cfg(test)]
            fixture_transport: false,
        }
    }
}
impl PiEngine {
    pub fn with_definition(&self, definition: crate::types::EngineDefinition) -> Self {
        Self { data: self.data.clone(), profile: profile::EngineProfile { definition }, sink: self.sink.clone(), graph: self.graph.clone(), keys: self.keys.clone(), ..Self::default() }
    }
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
    pub fn chat(data: PathBuf, sink: Sink) -> Self {
        Self { data: Some(data), chat: true, sink, ..Self::default() }
    }
    pub fn work(data: PathBuf, workspace: String, cwd: String, sink: Sink) -> Self {
        Self { work: true, ..Self::new(data, workspace, cwd, None, sink) }
    }
    fn chat_scratch(&self, id: &str) -> Result<PathBuf, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid chat ID")?;
        let path = self.data.as_ref().ok_or("Missing app data")?.join("chats/scratch").join(id);
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        Ok(path)
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
        let mut policy = super::engines::feature_policy(policy, &super::engine_registry::features(self.data.as_deref()));
        // Chain of Command belongs to Code. Keep the saved role/model choices,
        // but run Chat and Work as a single model when inheriting that policy.
        if (self.chat || self.work) && policy.subagents.mode == crate::types::SubagentMode::ChainOfCommand {
            policy.subagents.enabled = false;
            policy.subagents.mode = crate::types::SubagentMode::Custom;
        }
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
    pub(super) fn emit(&self, workspace: &str, event: Value) {
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
            .join(if self.chat { "chats/sessions".into() } else { format!("{}/sessions", self.profile.id()) })
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
                    thread["engine"] = json!(self.profile.id());
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
                            for key in ["modelProvider", "model", "forkedFromId", "chatWeb"] {
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
        let web = self.chat && s.thread["chatWeb"] == true;
        let mut connectors = if self.chat && !web { vec![] } else { super::connectors::load(
            self.data.as_ref().ok_or("Pi runtime is not configured")?,
            &s.workspace,
            self.graph.as_ref(),
        )? };
        if self.chat {
            connectors.retain(|c| c.enabled && c.id == "taucode-browser");
            for connector in &mut connectors {
                let allowed = ["browser_navigate", "browser_snapshot", "browser_take_screenshot"].into_iter()
                    .filter(|name| connector.tool_allowlist.as_ref().is_none_or(|list| list.iter().any(|n| n == name)))
                    .map(str::to_owned).collect();
                connector.tool_allowlist = Some(allowed);
            }
        }
        let fingerprint =
            serde_json::to_string(&(registry, policy, &connectors, super::engine_registry::features(self.data.as_deref()))).map_err(|e| e.to_string())?;
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
        let id = s.thread["id"].as_str().ok_or("Pi session missing ID")?.to_owned();
        let home = self
            .data
            .as_ref()
            .ok_or("Pi runtime is not configured")?
            .join(if self.chat { "chats/agent".into() } else { format!("{}/agent", self.profile.id()) })
            .join(if self.chat { &id } else { &s.workspace });
        let (credentials, delivery) = {
            let (registry, connectors, keys) = (registry.to_vec(), connectors.clone(), self.keys.clone());
            tokio::task::spawn_blocking(move || -> Result<_, String> {
                Ok((config::credentials(&registry, keys.as_ref())?, super::connectors::delivery(&connectors, keys.as_ref())?))
            }).await.map_err(|_| "Credential lookup failed".to_string())??
        };
        let mut env = config::prepare(&home, registry, self.graph.as_ref())?;
        env.extend(credentials);
        let features = super::engine_registry::features(self.data.as_deref());
        self.profile.prepare(&home, registry, &features, &mut env)?;
        for p in registry.iter().filter(|p| p.enabled) {
            if let crate::types::ProviderAuth::Env { var } = &p.auth {
                env.insert(var.clone(), std::env::var(var).map_err(|_| format!("Missing provider environment variable {var}"))?);
            }
            if matches!(p.auth, crate::types::ProviderAuth::Command { .. }) {
                if let Some(key) = providers::token(p, self.keys.as_ref()).await? {
                    env.insert(super::engines::key_env(&p.id), key);
                }
            }
        }
        config::write_json(&home.join("mcp.json"), &delivery.pi)?;
        if self.profile.omp() {
            let network = home.join("taucode/network.json");
            let mut origins: Vec<String> = serde_json::from_slice(&std::fs::read(&network).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            origins.extend(delivery.pi["mcpServers"].as_object().into_iter().flat_map(|servers| servers.values()).filter_map(|server| server["url"].as_str()).filter_map(|url| reqwest::Url::parse(url).ok()).map(|url| url.origin().ascii_serialization()));
            origins.sort(); origins.dedup();
            config::write_json(&network, &json!(origins))?;
        }
        config::write_json(&home.join("taucode/connectors.json"), &json!(connectors))?;
        env.extend(delivery.env);
        let mut secrets = delivery.secrets;
        for p in registry.iter().filter(|p| p.enabled) {
            if let Some(value) = env.get(&super::engines::key_env(&p.id)) {
                secrets.push(value.clone());
            }
        }
        config::write_policy(&home, registry, policy)?;
        config::write_thread_policy(&home, &id, registry, policy)?;
        config::write_access(&home, &id, None)?;
        if self.work {
            let file = home.join("taucode/threads").join(format!("{id}.json"));
            let mut access: Value = serde_json::from_slice(&std::fs::read(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            access["kind"] = json!("work");
            access["workspace"] = s.thread["cwd"].clone();
            access["readOnlyFolders"] = json!(self.read_only_folders.lock().await.get(&s.workspace).cloned().unwrap_or_default());
            if let Ok(root) = super::engine_runtime::source("work", None) { access["skillRoot"] = json!(root); }
            config::write_json(&file, &access)?;
        }
        env.insert("TAUCODE_SESSION_KEY".into(), id.clone());
        env.insert(
            "TAUCODE_WORKSPACE".into(),
            s.thread["cwd"].as_str().ok_or("Missing cwd")?.into(),
        );
        let node = super::engine_runtime::node_path();
        #[cfg(test)]
        let node = if self.fake { "python3".into() } else { node };
        #[cfg(test)]
        if !self.fake {
            super::engine_runtime::check_node(node.as_os_str()).await?;
        }
        let runtime = {
            #[cfg(test)]
            if self.fake { None } else { Some(super::engine_runtime::source(if self.profile.omp() { "omp" } else { "pi" }, None)?) }
            #[cfg(not(test))]
            if self.profile.custom() { None } else { Some(super::engine_runtime::prepare(self.data.as_ref().unwrap(), self.profile.id(), None).await?) }
        };
        let cli = std::env::var_os("TAUCODE_PI_CLI").map(PathBuf::from)
            .unwrap_or_else(|| runtime.as_ref().map(|dir| dir.join(if self.profile.omp() { "node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js" } else { "node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js" })).unwrap_or_default());
        #[cfg(test)]
        let cli = if self.fake {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_pi.py")
        } else {
            cli
        };
        if !self.profile.custom() && !cli.is_file() {
            return Err(
                "Pi runtime missing. Install engines/pi or set TAUCODE_PI_CLI to pi 1.4.0 cli.js"
                    .into(),
            );
        }
        if self.chat { s.thread["cwd"] = json!(self.chat_scratch(&id)?); }
        let mut command = if self.profile.custom() {
            super::engine_registry::validate(&self.profile.definition)?;
            let mut command = Command::new(super::engine_registry::executable(self.profile.definition.command.as_deref().ok_or("Missing engine command")?)?);
            command.args(&self.profile.definition.args);
            command
        } else if self.profile.omp() { Command::new(runtime.as_ref().ok_or("Missing omp runtime")?.join("node_modules/.bin/bun")) }
        else { Command::new(&node) };
        command.env_clear().envs(super::engine_registry::environment(std::env::vars(), &home));
        let guard = runtime.as_ref().map(|dir| dir.join("no-accounts.mjs")).unwrap_or_default();
        #[cfg(test)]
        if !self.fake && !self.profile.custom() {
            command.arg(if self.profile.omp() { "--preload" } else { "--import" }).arg(&guard).env("PI_OFFLINE", "1");
        }
        #[cfg(not(test))]
        if !self.profile.custom() { command.arg(if self.profile.omp() { "--preload" } else { "--import" }).arg(&guard); }
        #[cfg(test)]
        if self.fixture_transport { command.arg("--preload").arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/omp_fetch.mjs")); }
        if !self.profile.custom() { command.arg(&cli); }
        command
            .env("TAUCODE_PI_CLI", &cli)
            .env("TAUCODE_COMMAND_USAGE_DB", self.data.as_ref().ok_or("Missing app data")?.join("command-usage").join(format!("{}.sqlite", s.workspace)))
            .env_remove("TAUCODE_SUBAGENT_DEPTH")
            .env_remove("TAUCODE_TOOL_ACCESS")
            .arg("-e")
            .arg(home.join("extensions/taucode-gate.ts"));
        if self.chat {
            command.env("TAUCODE_CHAT", "1").args([
                "--no-tools", "--no-skills", "--no-prompt-templates", "--no-context-files",
                "--system-prompt", "You are a helpful conversational assistant. Answer clearly and honestly.",
            ]);
            if web && !connectors.is_empty() {
                command.env("TAUCODE_CHAT_WEB", "1").args(["-e", "builtin:mcp", "--tools", "mcp__taucode_browser__browser_navigate,mcp__taucode_browser__browser_snapshot,mcp__taucode_browser__browser_take_screenshot"]);
            } else { command.env_remove("TAUCODE_CHAT_WEB"); }
        } else if self.profile.omp() {
            command.args(["--no-pty", "--no-title", "--no-skills", "--no-rules"]);
            if features.subagents {
                let typebox = runtime.as_ref().ok_or("Missing omp runtime")?.join("node_modules/@oh-my-pi/omptype/dist/js/typebox.js");
                let source = include_str!("../../../../engines/pi/extensions/taucode-subagents.ts").replace("\"typebox\"", &serde_json::to_string(&typebox.to_string_lossy()).map_err(|e| e.to_string())?);
                std::fs::write(home.join("extensions/taucode-subagents.ts"), source).map_err(|e|e.to_string())?;
                command.arg("-e").arg(home.join("extensions/taucode-subagents.ts"));
            }
        } else {
            command.env_remove("TAUCODE_CHAT").env_remove("TAUCODE_CHAT_WEB");
            if features.subagents { command.arg("-e").arg(home.join("extensions/taucode-subagents.ts")); }
            if features.connectors { command.args(["-e", "builtin:mcp"]); }
        }
        command.env_remove("TAUCODE_WORK").env_remove("TAUCODE_WORK_PYTHON");
        if self.work {
            let source = super::engine_runtime::source("work", None)?;
            #[cfg(not(test))]
            let installed = super::engine_runtime::prepare(self.data.as_ref().unwrap(), "work", None).await.ok();
            #[cfg(test)]
            let installed: Option<PathBuf> = None;
            let python = installed.map(|p| p.join(if cfg!(windows) { ".venv/Scripts/python.exe" } else { ".venv/bin/python" }));
            command.arg("-e").arg(home.join("extensions/taucode-work.ts"));
            work_profile(&mut command, &source, python.as_deref());
        }
        if !self.profile.omp() { command.arg("--no-approve"); }
        command.args([
                "--no-extensions",
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
            .envs(env)
            .env("PATH", super::engine_runtime::node_path_env(&super::engine_runtime::node_path())?);
        let (_, history) = read_session(&s.file, registry)?;
        let mut translator = history;
        translator.diagnostic_codegraph = connectors.iter().any(|s|s.id=="codegraph" && s.enabled);
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
        translator.diagnostic_zero_cost = matches!(provider.auth, ProviderAuth::None) && matches!(provider.kind, ProviderKind::Ollama | ProviderKind::Lmstudio);
        translator.diagnostic_provider_kind = serde_json::to_value(provider.kind).unwrap_or_default().as_str().unwrap_or("unknown").into();
        translator.model = json!({"id":model,"provider":config::provider_id(provider,registry),"contextWindow":provider.models.iter().find(|m|m.id==model).and_then(|m|m.context_window)});
        let events = self.events.clone();
        let sink = self.sink.clone();
        let workspace = s.workspace.clone();
        let emit = Arc::new(move |e: Value| {
            let _ = events.send(e.clone());
            sink(&workspace, e);
        });
        let preview = self.previews.as_ref().map(|manager| {
            let mut env = super::engine_registry::environment(std::env::vars(), &home);
            env.extend(command.as_std().get_envs().filter_map(|(key, value)| {
                matches!(key.to_str(), Some("TAUCODE_WORK_PYTHON" | "PATH")).then(|| value.map(|v| (key.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()))).flatten()
            }));
            Arc::new(preview::Preview {
                manager: manager.clone(), data: self.data.clone().unwrap(), access: home.join("taucode/threads").join(format!("{id}.json")),
                workspace: s.workspace.clone(), thread: id.clone(), cwd: PathBuf::from(s.thread["cwd"].as_str().unwrap_or_default()),
                runtime: super::managed_tasks::TaskRuntime {
                    guard: Some((node.clone(), home.join("extensions/lib/commandGuard.ts"))),
                    env: Some(env.into_iter().collect()),
                }, secrets: secrets.clone(),
            })
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
        *p.preview.lock().await = preview;
        let mut starting = StartingProcess(Some(p.clone()));
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
        if require_extensions && p.extensions.lock().await.len() != if self.chat { 1 } else { 1 + usize::from(features.subagents) + usize::from(self.work) } {
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
        starting.0 = None;
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
            let mut cwd = params["cwd"]
                .as_str()
                .map(str::to_owned)
                .or(self.paths.lock().await.get(workspace).cloned())
                .or_else(|| self.chat.then(String::new))
                .ok_or("Missing pi workspace path")?;
            let dir = self.directory(workspace)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let id = uuid::Uuid::new_v4().to_string();
            if self.chat { cwd = self.chat_scratch(&id)?.to_string_lossy().into_owned(); }
            let file = dir.join(format!("{id}.jsonl"));
            // A valid pi session header fixes the ID before MCP children start, so
            // TAUCODE_SESSION_KEY always matches pi's sessionId, including on resume.
            let header = json!({"type":"session","version":3,"engine":self.profile.id(),"id":id,"timestamp":chrono::Utc::now().to_rfc3339(),"cwd":cwd,"modelProvider":provider.id,"model":model});
            std::fs::write(&file, format!("{header}\n")).map_err(|e| e.to_string())?;
            let mut thread = thread_value(&id, &cwd, &file);
            thread["engine"] = json!(self.profile.id());
            thread["modelProvider"] = json!(provider.id);
            thread["model"] = json!(model);
            if self.chat { thread["chatWeb"] = json!(params["web"] == true); }
            let mut session = Session {
                workspace: workspace.into(),
                thread: thread.clone(),
                file,
                process: None,
                configuration: String::new(),
            };
            if let Err(e) = if self.work { Ok(()) } else { self.spawn(&mut session, &registry, &policy).await.map(|_| ()) } {
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
        let team_home = self.data.as_ref().ok_or("Pi runtime is not configured")?
            .join(format!("{}/agent", self.profile.id())).join(workspace);
        if method == "thread/agentPolicy" {
            if self.chat || self.work { return Err("Conversation teams are available only in Code".into()); }
            match params["action"].as_str() {
                Some("get") => {},
                Some("set") => {
                    let team = serde_json::from_value::<crate::types::SubagentPolicy>(params["policy"].clone()).map_err(|e| format!("Invalid conversation team: {e}"))?;
                    let effective = config::apply_team(&policy, Some(team));
                    providers::validate(&registry, &effective)?;
                    config::write_team(&team_home, id, Some(&effective.subagents))?;
                },
                Some("reset") => config::write_team(&team_home, id, None)?,
                _ => return Err("Unknown conversation team action".into()),
            }
            let team = config::read_team(&team_home, id)?;
            return Ok(json!({"overridden":team.is_some(),"policy":config::apply_team(&policy, team)}));
        }
        if method == "thread/chatWeb/set" && self.chat {
            let web = params["web"].as_bool().ok_or("Missing Web state")?;
            if s.thread["chatWeb"] != web {
                if let Some(p) = &s.process {
                    if p.running.load(Ordering::SeqCst) || p.translator.lock().await.active { return Err("Stop the chat before changing Web access".into()); }
                    p.shutdown().await?;
                }
                s.thread["chatWeb"] = json!(web);
                s.configuration.clear();
            }
            return Ok(json!({}));
        }
        if method == "subagent/transcript" {
            let node = params["nodeId"].as_str().ok_or("Missing node ID")?;
            let transcript = if let Some(p) = &s.process {
                p.translator.lock().await.transcripts.get(node).cloned()
            } else { None };
            let transcript = transcript.or_else(|| read_session(&s.file, &registry).ok()
                .and_then(|(_, t)| t.transcripts.get(node).cloned()))
                .ok_or("Unknown subagent node")?;
            // Session files are raw. Apply the same credential redaction used by RPC.
            let mut secrets = Vec::new();
            for provider in registry.iter().filter(|p| p.enabled) {
                if let Some(token) = providers::token(provider, self.keys.as_ref()).await? { secrets.push(token); }
            }
            if let Some(data) = &self.data {
                let connectors = super::connectors::load(data, workspace, self.graph.as_ref())?;
                secrets.extend(super::connectors::delivery(&connectors, self.keys.as_ref())?.secrets);
            }
            return serde_json::from_str(&process::redact(transcript.to_string(), &secrets)).map_err(|e|e.to_string());
        }
        if method == "thread/fork" && self.chat {
            let (provider, model) = Self::model(&registry, &policy, params.get("model"), Some(&s.thread))?;
            if let Some(p) = &s.process {
                if p.running.load(Ordering::SeqCst) || p.translator.lock().await.active {
                    return Err("Finish or interrupt the chat before forking".into());
                }
                p.shutdown().await?;
            }
            let new_id = uuid::Uuid::new_v4().to_string();
            let file = self.directory(workspace)?.join(format!("{new_id}.jsonl"));
            fork_session(&s.file, &file, &new_id, &Value::Null, None)?;
            if let Some(item) = params["beforeUserItemId"].as_str() {
                if let Err(e) = rewind_session(&file, id, Some(item), None) {
                    let _ = std::fs::remove_file(&file); return Err(e);
                }
            }
            let (mut cloned, _) = read_session(&file, &registry)?;
            cloned["modelProvider"] = json!(provider.id);
            cloned["model"] = json!(model);
            cloned["forkedFromId"] = json!(id);
            cloned["cwd"] = json!(self.chat_scratch(&new_id)?);
            let bytes = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
            let (_, rest) = bytes.split_once('\n').ok_or("Invalid session")?;
            let header = json!({"type":"session","version":3,"engine":self.profile.id(),"id":new_id,"cwd":cloned["cwd"],"timestamp":chrono::Utc::now().to_rfc3339(),"modelProvider":provider.id,"model":model,"forkedFromId":id});
            let leaf = rest.lines().last().map(serde_json::from_str::<Value>).transpose().map_err(|e| e.to_string())?
                .map(|entry| entry["id"].clone()).unwrap_or(Value::Null);
            let change = json!({"type":"model_change","id":uuid::Uuid::new_v4().to_string(),"parentId":leaf,"timestamp":chrono::Utc::now().to_rfc3339(),"provider":config::provider_id(provider,&registry),"modelId":model});
            std::fs::write(&file, format!("{header}\n{rest}{change}\n")).map_err(|e| e.to_string())?;
            sessions.insert(new_id, Session { workspace: workspace.into(), thread: cloned.clone(), file, process: None, configuration: String::new() });
            return Ok(json!({"result":{"thread":cloned}}));
        }
        if method == "thread/fork" && params.get("sideOf").is_some() {
            let new_id = uuid::Uuid::new_v4().to_string();
            let file = self.directory(workspace)?.join(format!("{new_id}.jsonl"));
            fork_session(&s.file, &file, &new_id, &params["sideOf"], params["atItemId"].as_str())?;
            let (mut cloned, _) = read_session(&file, &registry)?;
            cloned["modelProvider"] = s.thread["modelProvider"].clone();
            cloned["model"] = s.thread["model"].clone();
            let home = &team_home;
            config::write_access(&home, &new_id, Some("read-only"))?;
            config::write_team(&home, &new_id, config::read_team(&home, id)?.as_ref())?;
            sessions.insert(new_id, Session { workspace: workspace.into(), thread: cloned.clone(), file, process: None, configuration: String::new() });
            return Ok(json!({"result":thread_response(cloned, false, "read-only")}));
        }
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
                .join(if self.chat { "chats/agent".into() } else { format!("{}/agent", self.profile.id()) })
                .join(if self.chat { id } else { workspace });
            config::write_access(&home, id, mode)?;
            if let Some(p) = &s.process {
                let access: Value = serde_json::from_slice(&std::fs::read(
                    home.join("taucode/threads").join(format!("{id}.json")),
                ).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                p.translator.lock().await.access_mode = access["accessMode"].as_str().unwrap_or("ask").into();
            }
            if method == "thread/accessMode/set" {
                return Ok(json!({"result":{}}));
            }
        }
        if method == "thread/resume" {
            let mut thread = s.thread.clone();
            let mode = if let Some(p) = s.process.as_ref().filter(|p| p.alive.load(Ordering::SeqCst)) {
                let t = p.translator.lock().await;
                thread["turns"] = json!(t.turns);
                t.access_mode.clone()
            } else {
                thread["turns"] = json!(read_session(&s.file, &registry)?.1.turns);
                let home = self.data.as_ref().ok_or("Pi runtime is not configured")?
                    .join(if self.chat { "chats/agent".into() } else { format!("{}/agent", self.profile.id()) })
                    .join(if self.chat { id } else { workspace });
                let access = match std::fs::read(home.join("taucode/threads").join(format!("{id}.json"))) {
                    Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())?,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
                    Err(e) => return Err(e.to_string()),
                };
                access["accessMode"].as_str().unwrap_or("ask").to_owned()
            };
            return Ok(json!({"result":thread_response(thread, true, &mode)}));
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
        let policy = if !self.chat && !self.work && !matches!(method, "turn/interrupt" | "turn/steer") {
            let team = config::read_team(&team_home, id)?;
            let overridden = team.is_some();
            let effective = config::apply_team(&policy, team);
            if overridden { providers::validate(&registry, &effective)?; }
            super::engines::feature_policy(effective, &super::engine_registry::features(self.data.as_deref()))
        } else { policy };
        let p = if matches!(method, "turn/interrupt" | "turn/steer") {
            match &s.process {
                Some(p) if p.alive.load(Ordering::SeqCst) => p.clone(),
                _ if method == "turn/interrupt" => return Ok(json!({"result":{}})),
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
            config::write_team(&team_home, &new_id, config::read_team(&team_home, id)?.as_ref())?;
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
                t.diagnostic_zero_cost = matches!(provider.auth, ProviderAuth::None) && matches!(provider.kind, ProviderKind::Ollama | ProviderKind::Lmstudio);
                t.diagnostic_provider_kind = serde_json::to_value(provider.kind).unwrap_or_default().as_str().unwrap_or("unknown").into();
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
            if let Some(cap) = provider.models.iter().find(|m| m.id == model).and_then(|m| m.reasoning.as_ref()) {
                if let Some(level) = super::reasoning::clamp(cap, params["effort"].as_str()) {
                    p.request(json!({"type":"set_thinking_level","level":level})).await?;
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
fn fork_session(source: &Path, target: &Path, id: &str, side_of: &Value, at_item: Option<&str>) -> Result<(), String> {
    let bytes = std::fs::read_to_string(source).map_err(|e|e.to_string())?;
    let mut entries = Vec::<Value>::new();
    for line in bytes.split_inclusive('\n').filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str(line) {
            Ok(entry) => entries.push(entry),
            Err(_) if !line.ends_with('\n') => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    let parent = entries.first().and_then(|e|e["id"].as_str()).ok_or("Missing session header")?.to_owned();
    if let Some(item) = at_item {
        let mut count = 0;
        let end = entries.iter().position(|entry| {
            if entry["type"] != "message" || entry["message"]["role"] != "assistant" { return false; }
            count += 1;
            let stamp = entry["message"]["timestamp"].as_u64().unwrap_or(count);
            item == format!("{parent}:message:{stamp}")
        }).ok_or("This message is not yet saved in the pi session")?;
        entries.truncate(end + 1);
    }
    entries[0]["id"] = json!(id);
    entries[0]["sideOf"] = side_of.clone();
    entries[0]["timestamp"] = json!(chrono::Utc::now().to_rfc3339());
    let mut output = entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    output.push('\n');
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|e|e.to_string())?;
    if let Err(e) = file.write_all(output.as_bytes()) {
        let _ = std::fs::remove_file(target);
        return Err(e.to_string());
    }
    Ok(())
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
    for key in ["engine", "modelProvider", "model", "forkedFromId"] {
        if !header[key].is_null() { thread[key] = header[key].clone(); }
    }
    if header.get("sideOf").is_some() {
        thread["sideOf"] = header["sideOf"].clone();
        thread["forkedFromId"] = header["sideOf"]["threadId"].clone();
    }
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
    let finished_calls: std::collections::HashSet<_> = branch.iter().filter_map(|entry| {
        if entry["type"] == "message" && entry["message"]["role"] == "toolResult"
            && agents::results(&entry["message"]["details"]).is_some() {
            entry["message"]["toolCallId"].as_str()
        } else if entry["customType"] == "taucode-subagent-result"
            && agents::results(&entry["data"]["result"]["details"]).is_some() {
            entry["data"]["toolCallId"].as_str()
        } else { None }
    }).collect();
    let mut messages = vec![];
    let recovery = recovery::Checkpoints::load(file);
    let recorded_calls: std::collections::HashSet<_> = branch.iter().filter(|entry|
        entry["type"] == "message" && entry["message"]["role"] == "toolResult"
    ).filter_map(|entry| entry["message"]["toolCallId"].as_str()).collect();
    for (index, entry) in branch.iter().enumerate() {
        match entry["type"].as_str() {
            Some("message") => {
                let mut m = entry["message"].clone();
                if m["role"] == "assistant" && matches!(m["stopReason"].as_str(), Some("aborted" | "error")) && m["content"].as_array().is_some_and(Vec::is_empty) {
                    if let Some(saved) = entry["parentId"].as_str().and_then(|id| recovery.message(id)).filter(|saved| saved["timestamp"] == m["timestamp"]) {
                        m["content"] = saved["content"].clone();
                    }
                }
                if m["role"] == "toolResult" && m["isError"] == true {
                    if let Some(saved) = recovery.tool(&json!({"id":m["toolCallId"],"name":m["toolName"]})) {
                        let partial = translate::text(&saved["result"]["content"]);
                        let durable = translate::text(&m["content"]);
                        if !partial.is_empty() && !durable.contains(&partial) {
                            // OMP can checkpoint a cancellation label followed by the
                            // same output it persists. Keep the label without repeating output.
                            let missing = partial.strip_suffix(&durable).unwrap_or(&partial).trim_end();
                            if !missing.is_empty() {
                                if let Some(content) = m["content"].as_array_mut() { content.insert(0, json!({"type":"text","text":missing})); }
                            }
                        }
                    }
                }
                if m["role"] == "user" && thread["preview"] == "" {
                    thread["preview"] = json!(translate::text(&m["content"]));
                }
                messages.push(m.clone());
                if m["role"] == "assistant" {
                    for call in m["content"].as_array().into_iter().flatten().filter(|p| p["type"] == "toolCall") {
                        let Some(id) = call["id"].as_str() else { continue; };
                        if !finished_calls.contains(id) {
                            if let Some(saved) = agents::checkpoint(file, id) { messages.push(saved); }
                        }
                        if !recorded_calls.contains(id) {
                            if let Some(saved) = recovery.tool(call) { messages.push(saved); }
                        }
                    }
                }
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
            Some("custom") if entry["customType"] == "taucode-subagent-result" => messages.push(json!({"role":"taucodeSubagentResult","toolCallId":entry["data"]["toolCallId"],"result":entry["data"]["result"]})),
            Some("custom") if entry["customType"] == "taucode-verification" => messages.push(json!({"role":"taucodeVerification","record":entry["data"]})),
            Some("custom") if entry["customType"] == "taucode-execution-limit" => messages.push(json!({"role":"taucodeExecutionLimit","record":entry["data"]})),
            Some("custom") if entry["customType"] == "session_exit" && entry["data"]["pendingToolCalls"].as_array().is_some_and(|calls| !calls.is_empty()) => messages.push(json!({"role":"taucodeInterrupted"})),
            _ => (),
        }
        // A saved assistant (including an alternate branch) or explicit rewind
        // supersedes its unfinished stream. A later user turn must retain it.
        if let Some(saved) = entry["id"].as_str().and_then(|id| recovery.message(id)) {
            let superseded = branch[index + 1..].iter().take_while(|next| next["message"]["role"] != "user")
                .any(|next| next["message"]["role"] == "assistant" || next["customType"] == "taucode-rewind");
            if !superseded { messages.push(saved); }
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

const WORK_PROMPT: &str = "You are doing knowledge work in the selected space. Use ask_user when you need clarification or a user decision; do not assume an answer. Plan first with a Markdown checklist (- [ ] step); update it with - [x] as you progress. Keep outputs in outputs/ unless told otherwise. In Ask mode, ask before deleting or overwriting user files outside outputs/. Full access authorizes workspace edits and shell commands without extra confirmation; read-only mode permits no writes. Never write outside this space; extra folders are read-only. Submit tools directly; taucode enforces their approval rules. Do not use ask_user to request tool permission. Shell commands must respect these same boundaries. Use preview_server for local preview servers, bound to 127.0.0.1 with a foreground command; do not detach servers with shell backgrounding, nohup, or daemon mode. Inspect its status/logs and verify readiness in the browser. Taucode keeps previews after completion and provides Stop/Restart controls. Use the bundled document skills and TAUCODE_WORK_PYTHON; if unavailable explain that document libraries are unavailable and offer Markdown/CSV instead. Summarize what was produced with relative file paths. Do not schedule recurring tasks.";
fn work_profile(command: &mut Command, source: &Path, python: Option<&Path>) {
    command.env("TAUCODE_WORK", "1")
        .args(["--no-skills", "--no-context-files", "--no-prompt-templates", "--tools", "read,write,edit,ls,find,grep,bash,subagent,ask_user,preview_server,mcp__*", "--append-system-prompt", WORK_PROMPT])
        .arg("--skill").arg(source.join("skills"));
    if let Some(python) = python { command.env("TAUCODE_WORK_PYTHON", python); }
}
