mod client;
mod config;
mod permissions;
#[cfg(test)]
mod tests;
mod translate;

use super::{
    engines::EngineBackend,
    providers::{self, BoxFuture, KeyStore, Keychain},
};
use crate::{
    codegraph::CodegraphSetup,
    types::{AgentPolicy, McpServerEntry, ProviderEntry},
};
use client::Client;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
};
use tokio::{
    process::Command,
    sync::{broadcast, Mutex},
};
use translate::Translator;

type Sink = Arc<dyn Fn(&str, Value) + Send + Sync>;
struct Thread {
    workspace: String,
    metadata: Value,
    history: Translator,
    loaded: String,
    mode: String,
    capabilities: Value,
    replaying: bool,
    grants: HashSet<String>,
    file: PathBuf,
}
impl Thread {
    fn save(&mut self) -> Result<(), String> {
        self.metadata["turns"] = json!(self.history.turns);
        self.metadata["accessMode"] = json!(self.mode);
        self.metadata["updatedAt"] = json!(chrono::Utc::now().timestamp());
        let temp = self
            .file
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(
            &temp,
            serde_json::to_vec(&self.metadata).map_err(|e| e.to_string())?,
        )
        .and_then(|_| std::fs::rename(&temp, &self.file))
        .map_err(|e| e.to_string())
    }
}
struct Runtime {
    client: Arc<Client>,
    fingerprint: String,
    connectors: Vec<McpServerEntry>,
}
struct Dialog {
    workspace: String,
    thread: String,
    request_id: Value,
    params: Value,
}
pub(crate) struct ClaudeAcpEngine {
    data: Option<PathBuf>,
    pub paths: Mutex<HashMap<String, String>>,
    threads: Arc<Mutex<HashMap<String, Thread>>>,
    runtimes: Mutex<HashMap<String, Runtime>>,
    registry: Mutex<HashMap<String, (Vec<ProviderEntry>, AgentPolicy)>>,
    dialogs: Arc<Mutex<HashMap<String, Dialog>>>,
    operation: Mutex<()>,
    events: broadcast::Sender<Value>,
    sink: Sink,
    graph: Option<CodegraphSetup>,
    keys: Arc<dyn KeyStore>,
    #[cfg(test)]
    fake: bool,
}
impl Default for ClaudeAcpEngine {
    fn default() -> Self {
        Self {
            data: None,
            paths: Mutex::new(HashMap::new()),
            threads: Arc::new(Mutex::new(HashMap::new())),
            runtimes: Mutex::new(HashMap::new()),
            registry: Mutex::new(HashMap::new()),
            dialogs: Arc::new(Mutex::new(HashMap::new())),
            operation: Mutex::new(()),
            events: broadcast::channel(1024).0,
            sink: Arc::new(|_, _| {}),
            graph: None,
            keys: Arc::new(Keychain),
            #[cfg(test)]
            fake: false,
        }
    }
}
impl ClaudeAcpEngine {
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
    pub async fn configure(
        &self,
        workspace: &str,
        providers: Vec<ProviderEntry>,
        policy: AgentPolicy,
    ) {
        self.registry
            .lock()
            .await
            .insert(workspace.into(), (providers, policy));
    }
    pub async fn credentials_changed(&self) {
        for runtime in self.runtimes.lock().await.values_mut() {
            runtime.fingerprint.clear();
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
            .ok_or("Claude Code runtime is not configured")?
            .join("claude")
            .join(workspace))
    }
    async fn scan(&self, workspace: &str) -> Result<(), String> {
        if self.data.is_none() {
            return Ok(());
        }
        let dir = self.directory(workspace)?.join("threads");
        let files = match std::fs::read_dir(dir) {
            Ok(files) => files,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        let mut threads = self.threads.lock().await;
        for file in files {
            let file = file.map_err(|e| e.to_string())?.path();
            if file.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if file
                .file_stem()
                .and_then(|v| v.to_str())
                .and_then(|id| threads.get(id))
                .is_some_and(|t| t.workspace == workspace)
            {
                continue;
            }
            let metadata: Value =
                serde_json::from_slice(&std::fs::read(&file).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let id = metadata["id"]
                .as_str()
                .ok_or("Invalid Claude thread metadata")?
                .to_string();
            if metadata["engine"] != "claude" || !metadata["sessionId"].is_string() {
                return Err("Invalid Claude session mapping".into());
            }
            let cwd = metadata["cwd"]
                .as_str()
                .ok_or("Invalid Claude thread cwd")?
                .to_string();
            let turns = metadata["turns"].as_array().cloned().unwrap_or_default();
            let mode = metadata["accessMode"].as_str().unwrap_or("ask").to_string();
            threads.entry(id.clone()).or_insert(Thread {
                workspace: workspace.into(),
                metadata,
                history: Translator::new(id, cwd, turns),
                loaded: String::new(),
                mode,
                capabilities: Value::Null,
                replaying: false,
                grants: HashSet::new(),
                file,
            });
        }
        Ok(())
    }
    pub async fn owns(&self, workspace: &str, id: &str) -> bool {
        let _ = self.scan(workspace).await;
        self.threads
            .lock()
            .await
            .get(id)
            .is_some_and(|t| t.workspace == workspace)
    }
    fn model<'a>(
        registry: &'a [ProviderEntry],
        policy: &AgentPolicy,
        params: &Value,
        current: Option<&Value>,
    ) -> Result<(&'a ProviderEntry, String), String> {
        let explicit = params["model"].as_str().map(str::to_string);
        let selected = explicit
            .or_else(|| {
                current.and_then(|t| {
                    Some(format!(
                        "{}/{}",
                        t["modelProvider"].as_str()?,
                        t["model"].as_str()?
                    ))
                })
            })
            .or_else(|| {
                policy
                    .default_model
                    .as_ref()
                    .map(|m| format!("{}/{}", m.provider_id, m.model_id))
            })
            .or_else(|| {
                registry
                    .iter()
                    .filter(|p| p.enabled && providers::claude_usable(p))
                    .find_map(|p| {
                        p.models
                            .iter()
                            .find(|m| m.enabled)
                            .map(|m| format!("{}/{}", p.id, m.id))
                    })
            })
            .ok_or(config::PROVIDER_ERROR)?;
        let (provider, model) = selected
            .split_once('/')
            .ok_or("Select a registry model (provider/model)")?;
        let provider = registry
            .iter()
            .find(|p| p.enabled && p.id == provider)
            .ok_or("Enable this thread's provider in Settings")?;
        if !providers::claude_usable(provider) {
            return Err(config::PROVIDER_ERROR.into());
        }
        if !provider.models.iter().any(|m| m.enabled && m.id == model) {
            return Err("Enable this registry model in Settings".into());
        }
        if current.is_some_and(|t| t["modelProvider"] != provider.id) {
            return Err("Start a new Claude Code thread to switch providers".into());
        }
        Ok((provider, model.into()))
    }
    async fn spawn(
        &self,
        workspace: &str,
        cwd: &str,
        provider: &ProviderEntry,
        model: &str,
    ) -> Result<Arc<Client>, String> {
        let data = self
            .data
            .as_ref()
            .ok_or("Claude Code runtime is not configured")?;
        let connectors = super::connectors::load(data, workspace, self.graph.as_ref())?;
        let fingerprint =
            serde_json::to_string(&(provider, &connectors)).map_err(|e| e.to_string())?;
        let mut runtimes = self.runtimes.lock().await;
        if let Some(runtime) = runtimes.get(workspace) {
            if runtime.client.alive.load(Ordering::SeqCst) && runtime.fingerprint == fingerprint {
                return Ok(runtime.client.clone());
            }
            if runtime.client.running.load(Ordering::SeqCst) > 0 {
                return Err("Provider settings changed. Finish or interrupt running Claude Code turns, then retry.".into());
            }
            runtime.client.shutdown().await?;
        }
        // Remove the old runtime before starting a replacement, including on errors.
        runtimes.remove(workspace);
        let home = self.directory(workspace)?;
        config::private_directory(&home)?;
        config::private_directory(&home.join("threads"))?;
        let token = providers::token(provider, self.keys.as_ref())
            .await?
            .ok_or("Save an API key for this provider in Settings")?;
        let env = config::environment(std::env::vars(), &home, provider, &token, model)?;
        let (_, mut secrets) = config::mcp_servers(&connectors, self.keys.as_ref(), workspace)?;
        secrets.push(token);
        let node = std::env::var("TAUCODE_NODE").unwrap_or_else(|_| "node".into());
        #[cfg(test)]
        let (node, cli) = if self.fake {
            (
                "python3".into(),
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_acp.py"),
            )
        } else {
            (
                node,
                super::engine_runtime::source("claude-acp", None)?
                    .join("node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js"),
            )
        };
        #[cfg(not(test))]
        let cli = super::engine_runtime::prepare(data, "claude-acp", None)
            .await?
            .join("node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js");
        let mut command = Command::new(node);
        command
            .arg(cli)
            .arg("--hide-claude-auth")
            .env_clear()
            .envs(env)
            .current_dir(cwd);
        let threads = self.threads.clone();
        let dialogs = self.dialogs.clone();
        let sink = self.sink.clone();
        let events = self.events.clone();
        let ws = workspace.to_string();
        let gates = connectors.clone();
        let client = Client::spawn(command, Arc::new(move |message| {
            let (threads,dialogs,sink,events,workspace,gates) = (threads.clone(),dialogs.clone(),sink.clone(),events.clone(),ws.clone(),gates.clone());
            Box::pin(async move {
                let emit = |event: Value| { let _ = events.send(event.clone()); sink(&workspace,event); };
                let method = message["method"].as_str().unwrap_or("");
                if method == "codex/stderr" { emit(message); return None; }
                if method == "acp/disconnected" {
                    let mut dialogs = dialogs.lock().await;
                    dialogs.retain(|id,d| { if d.workspace != workspace { return true; } emit(json!({"method":"serverRequest/resolved","params":{"threadId":d.thread,"requestId":id}})); false });
                    return None;
                }
                let params = &message["params"];
                let mut threads = threads.lock().await;
                let thread = threads.values_mut().find(|t| t.workspace==workspace && t.metadata["sessionId"]==params["sessionId"]);
                let Some(thread) = thread else { return message.get("id").map(|_|json!({"outcome":{"outcome":"cancelled"}})); };
                if method == "session/update" {
                    // Live prompts already have a user item; load replay creates its own turns.
                    if params["update"]["sessionUpdate"] == "user_message_chunk" && !thread.replaying { return None; }
                    let updates = thread.history.update(&params["update"]);
                    if !thread.replaying { for event in updates { emit(event); } }
                    if let Err(e) = thread.save() { emit(thread.history.event("error",json!({"error":{"message":e}}))); }
                } else if method == "session/request_permission" {
                    use permissions::Decision;
                    let mut decision = permissions::decide(&thread.mode,&thread.history.cwd,&gates,&params["toolCall"]);
                    if decision == Decision::Ask && thread.grants.contains(&permissions::grant_key(&params["toolCall"])) { decision = Decision::Allow; }
                    if decision != Decision::Ask {
                        if decision == Decision::Reject { emit(thread.history.event("codex/stderr",json!({"message":"Claude Code tool denied by access mode or connector policy"}))); }
                        return Some(permissions::outcome(params,if decision==Decision::Allow{"accept"}else{"decline"}));
                    }
                    let id = format!("claude:{}",uuid::Uuid::new_v4());
                    dialogs.lock().await.insert(id.clone(),Dialog{workspace:workspace.clone(),thread:thread.history.thread.clone(),request_id:message["id"].clone(),params:params.clone()});
                    emit(permissions::approval(&id,&thread.history.thread,thread.history.turn_id(),&thread.history.cwd,params));
                } else if message.get("id").is_some() {
                    return Some(json!({"outcome":{"outcome":"cancelled"}}));
                }
                None
            })
        }),secrets).await?;
        let initialized = client.request("initialize",json!({"protocolVersion":1,"clientInfo":{"name":"taucode","version":env!("CARGO_PKG_VERSION")},"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"_meta":{"terminal_output_delta":true}}})).await;
        match initialized {
            Ok(capabilities) => *client.capabilities.lock().await = capabilities,
            Err(e) => {
                client.shutdown().await?;
                return Err(e);
            }
        }
        runtimes.insert(
            workspace.into(),
            Runtime {
                client: client.clone(),
                fingerprint,
                connectors,
            },
        );
        Ok(client)
    }
    async fn session_params(
        &self,
        workspace: &str,
        id: &str,
        cwd: &str,
        model: &str,
    ) -> Result<Value, String> {
        let runtimes = self.runtimes.lock().await;
        let runtime = runtimes
            .get(workspace)
            .ok_or("Claude Code is not running")?;
        let (servers, _) = config::mcp_servers(&runtime.connectors, self.keys.as_ref(), id)?;
        if servers
            .as_array()
            .into_iter()
            .flatten()
            .any(|s| s["type"] == "http")
            && runtime
                .client
                .capabilities
                .lock()
                .await
                .pointer("/agentCapabilities/mcpCapabilities/http")
                != Some(&json!(true))
        {
            return Err("This ACP agent does not support HTTP connectors".into());
        }
        Ok(config::session_params(cwd, servers, model))
    }
    async fn load(
        &self,
        workspace: &str,
        id: &str,
        client: &Arc<Client>,
        model: &str,
    ) -> Result<(), String> {
        let (session, cwd, loaded) = {
            let threads = self.threads.lock().await;
            let t = threads.get(id).ok_or("Unknown Claude thread")?;
            (
                t.metadata["sessionId"].clone(),
                t.history.cwd.clone(),
                t.loaded == client.id,
            )
        };
        let capabilities = engine_capabilities(&*client.capabilities.lock().await);
        self.threads.lock().await.get_mut(id).unwrap().metadata["capabilities"] = capabilities;
        if loaded {
            return Ok(());
        }
        let mut params = self.session_params(workspace, id, &cwd, model).await?;
        let load = client
            .capabilities
            .lock()
            .await
            .pointer("/agentCapabilities/loadSession")
            == Some(&json!(true));
        let old_turns = {
            let mut threads = self.threads.lock().await;
            let t = threads.get_mut(id).unwrap();
            t.replaying = true;
            if load {
                t.history.active = false;
                Some(std::mem::take(&mut t.history.turns))
            } else {
                None
            }
        };
        if load {
            params["sessionId"] = session;
        }
        let result = client
            .request(if load { "session/load" } else { "session/new" }, params)
            .await;
        let mut threads = self.threads.lock().await;
        let t = threads.get_mut(id).unwrap();
        t.replaying = false;
        match result {
            Ok(result) => {
                t.history.finish("end_turn", None);
                if t.history.turns.is_empty() {
                    if let Some(old) = old_turns {
                        t.history.turns = old;
                    }
                }
                if !load {
                    t.metadata["sessionId"] = result["sessionId"].clone();
                    t.metadata["resumeNote"] = json!(
                        "This ACP agent cannot load sessions. Started a fresh Claude Code session."
                    );
                    self.emit(
                        workspace,
                        t.history
                            .event("codex/stderr", json!({"message":t.metadata["resumeNote"]})),
                    );
                }
                t.capabilities = result;
                t.loaded = client.id.clone();
                t.save()?;
            }
            Err(e) => {
                if let Some(old) = old_turns {
                    t.history.turns = old;
                }
                t.history.active = false;
                t.save()?;
                return Err(e);
            }
        }
        Ok(())
    }
    async fn set_mode(&self, id: &str, client: &Client, mode: &str) -> Result<(), String> {
        let current = self.threads.lock().await.get(id).ok_or("Unknown Claude thread")?.mode.clone();
        let mode = if mode == "current" { &current } else { mode };
        if !matches!(mode, "ask" | "read-only" | "full-access") {
            return Err("Invalid access mode".into());
        }
        let (session, modes) = {
            let threads = self.threads.lock().await;
            let t = threads.get(id).ok_or("Unknown Claude thread")?;
            (
                t.metadata["sessionId"].clone(),
                t.capabilities["modes"]["availableModes"].clone(),
            )
        };
        let target = if mode == "read-only" {
            "plan"
        } else {
            "default"
        };
        if modes
            .as_array()
            .is_some_and(|m| m.iter().any(|m| m["id"] == target))
        {
            client
                .request(
                    "session/set_mode",
                    json!({"sessionId":session,"modeId":target}),
                )
                .await?;
        }
        let mut threads = self.threads.lock().await;
        let t = threads.get_mut(id).unwrap();
        if t.mode != mode {
            t.grants.clear();
        }
        t.mode = mode.into();
        t.save()
    }
    async fn dispatch(
        &self,
        workspace: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        // Interrupt and steering do not wait for the in-flight prompt response.
        if matches!(method, "turn/interrupt" | "turn/steer") {
            let id = params["threadId"].as_str().ok_or("Missing thread ID")?;
            let session = {
                let threads = self.threads.lock().await;
                let t = threads
                    .get(id)
                    .filter(|t| t.workspace == workspace)
                    .ok_or("Unknown Claude thread")?;
                t.metadata["sessionId"].clone()
            };
            let client = self
                .runtimes
                .lock()
                .await
                .get(workspace)
                .map(|r| r.client.clone())
                .ok_or("Claude Code is not running")?;
            let _client_operation = client.operation.lock().await;
            if method == "turn/interrupt" {
                client
                    .notify("session/cancel", json!({"sessionId":session}))
                    .await?;
                let mut dialogs = self.dialogs.lock().await;
                let ids: Vec<_> = dialogs
                    .iter()
                    .filter(|(_, d)| d.thread == id && d.workspace == workspace)
                    .map(|(id, _)| id.clone())
                    .collect();
                for request in ids {
                    let d = dialogs.remove(&request).unwrap();
                    client
                        .respond(d.request_id, permissions::outcome(&d.params, "cancel"))
                        .await?;
                    self.emit(workspace,json!({"method":"serverRequest/resolved","params":{"threadId":id,"requestId":request}}));
                }
            } else {
                if client
                    .capabilities
                    .lock()
                    .await
                    .pointer("/_meta/steering/supported")
                    != Some(&json!(true))
                {
                    return Err("Steering is unsupported for this engine".into());
                }
                let prompt = prompt_input(
                    &params["input"],
                    client
                        .capabilities
                        .lock()
                        .await
                        .pointer("/agentCapabilities/promptCapabilities/image")
                        == Some(&json!(true)),
                )?;
                client.request("_session/steering",json!({"sessionId":session,"prompt":prompt,"_meta":{"steering":{"idleBehavior":"promptRequired"}}})).await?;
            }
            return Ok(json!({"result":{}}));
        }
        let _operation = self.operation.lock().await;
        self.scan(workspace).await?;
        if method == "thread/list" {
            let mut data: Vec<_> = self
                .threads
                .lock()
                .await
                .values()
                .filter(|t| t.workspace == workspace && t.metadata["archived"] != true)
                .map(|t| {
                    let mut v = t.metadata.clone();
                    v["turns"] = json!([]);
                    v
                })
                .collect();
            data.sort_by_key(|t| std::cmp::Reverse(t["updatedAt"].as_i64().unwrap_or(0)));
            let (data, next) = page(data, &params)?;
            return Ok(json!({"result":{"data":data,"nextCursor":next}}));
        }
        let (registry, policy) = self
            .registry
            .lock()
            .await
            .get(workspace)
            .cloned()
            .unwrap_or_default();
        let id = params["threadId"].as_str();
        let current = if let Some(id) = id {
            Some(
                self.threads
                    .lock()
                    .await
                    .get(id)
                    .filter(|t| t.workspace == workspace)
                    .ok_or("Unknown Claude thread")?
                    .metadata
                    .clone(),
            )
        } else {
            None
        };
        if method == "model/list" {
            let (provider, _) = Self::model(&registry, &policy, &params, current.as_ref())?;
            return Ok(providers::catalog(std::slice::from_ref(provider)));
        }
        if method == "thread/read" {
            return Ok(json!({"result":{"thread":current}}));
        }
        if method == "subagent/transcript" {
            let node = params["nodeId"].as_str().ok_or("Missing node ID")?;
            let item = current.as_ref().unwrap()["turns"].as_array().into_iter().flatten()
                .flat_map(|turn| turn["items"].as_array().into_iter().flatten())
                .find(|item| item["agentsTree"].as_array().into_iter().flatten().any(|agent| agent["id"] == node))
                .ok_or("Node is not a child of this thread")?;
            let mut transcript = super::pi::agents::capped(item["agentUpdates"].as_array().into_iter().flatten().cloned());
            transcript["truncated"] = json!(true);
            transcript["note"] = json!("Claude Code exposes Task tool updates, not a separate subagent transcript.");
            return Ok(transcript);
        }
        if method == "thread/turns/list" {
            let (data, next) = page(
                current.as_ref().unwrap()["turns"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .rev()
                    .collect(),
                &params,
            )?;
            return Ok(json!({"result":{"data":data,"nextCursor":next,"backwardsCursor":null}}));
        }
        if matches!(method, "thread/compact/start" | "thread/rollback") {
            return Err("Unsupported for this engine".into());
        }
        if matches!(
            method,
            "thread/name/set" | "thread/archive" | "thread/unarchive"
        ) {
            let mut threads = self.threads.lock().await;
            let t = threads.get_mut(id.ok_or("Missing thread ID")?).unwrap();
            if method == "thread/name/set" {
                t.metadata["name"] = params["name"].clone();
            } else {
                t.metadata["archived"] = json!(method == "thread/archive");
            }
            t.save()?;
            return Ok(json!({"result":{}}));
        }
        let (provider, model) = Self::model(&registry, &policy, &params, current.as_ref())?;
        let cwd = current
            .as_ref()
            .and_then(|t| t["cwd"].as_str())
            .or_else(|| params["cwd"].as_str())
            .map(str::to_string)
            .or(self.paths.lock().await.get(workspace).cloned())
            .ok_or("Missing Claude workspace path")?;
        let client = self.spawn(workspace, &cwd, provider, &model).await?;
        let _client_operation = client.operation.lock().await;
        if method == "thread/start" {
            let id = format!("claude-{}", uuid::Uuid::new_v4());
            let result = client
                .request(
                    "session/new",
                    self.session_params(workspace, &id, &cwd, &model).await?,
                )
                .await?;
            if !result["sessionId"].is_string() {
                return Err("ACP did not return a session ID".into());
            }
            let now = chrono::Utc::now().timestamp();
            let metadata = json!({"id":id,"sessionId":result["sessionId"],"engine":"claude","cwd":cwd,"modelProvider":provider.id,"model":model,"preview":"","name":null,"createdAt":now,"updatedAt":now,"status":{"type":"idle"},"turns":[],"source":{"custom":"taucode-claude"},"capabilities":engine_capabilities(&*client.capabilities.lock().await)});
            let mut thread = Thread {
                workspace: workspace.into(),
                metadata,
                history: Translator::new(id.clone(), cwd, vec![]),
                loaded: client.id.clone(),
                mode: "ask".into(),
                capabilities: result,
                replaying: false,
                grants: HashSet::new(),
                file: self
                    .directory(workspace)?
                    .join("threads")
                    .join(format!("{id}.json")),
            };
            thread.save()?;
            let response = thread_response(&thread);
            self.emit(
                workspace,
                json!({"method":"thread/started","params":{"thread":thread.metadata}}),
            );
            self.threads.lock().await.insert(id, thread);
            return Ok(json!({"result":response}));
        }
        let id = id.ok_or("Missing thread ID")?;
        self.load(workspace, id, &client, &model).await?;
        let session = self.threads.lock().await[id].metadata["sessionId"].clone();
        if method == "thread/fork" {
            if engine_capabilities(&*client.capabilities.lock().await)["fork"] != true {
                return Err("Claude Code adapter doesn't support forking sessions".into());
            }
            if !params["turnId"].is_null() || !params["atItemId"].is_null() {
                return Err("Claude Code sessions can only fork at the latest point".into());
            }
            let mut fork = self.session_params(workspace, id, &cwd, &model).await?;
            fork["sessionId"] = session;
            let result = client.request("session/fork", fork).await?;
            let new_id = format!("claude-{}", uuid::Uuid::new_v4());
            let mut metadata = self.threads.lock().await[id].metadata.clone();
            metadata["id"] = json!(new_id);
            metadata["sessionId"] = result["sessionId"].clone();
            metadata["forkedFromId"] = json!(id);
            if let Some(side_of) = params.get("sideOf") {
                metadata["sideOf"] = side_of.clone();
            }
            let turns = metadata["turns"].as_array().cloned().unwrap_or_default();
            let mut thread = Thread {
                workspace: workspace.into(),
                metadata,
                history: Translator::new(new_id.clone(), cwd, turns),
                loaded: String::new(),
                mode: if params["sandbox"] == "read-only" { "read-only" } else { "ask" }.into(),
                capabilities: result,
                replaying: false,
                grants: HashSet::new(),
                file: self
                    .directory(workspace)?
                    .join("threads")
                    .join(format!("{new_id}.json")),
            };
            thread.save()?;
            let response = thread_response(&thread);
            self.threads.lock().await.insert(new_id, thread);
            return Ok(json!({"result":response}));
        }
        if method == "thread/resume" {
            return Ok(json!({"result":thread_response(&self.threads.lock().await[id])}));
        }
        let mode = params["accessMode"]
            .as_str()
            .or_else(|| match params["sandboxPolicy"]["type"].as_str() {
                Some("dangerFullAccess") => Some("full-access"),
                Some("readOnly") => Some("read-only"),
                Some("workspaceWrite") => Some("ask"),
                _ => None,
            })
            .map(str::to_string)
            .unwrap_or(self.threads.lock().await[id].mode.clone());
        if !matches!(method, "turn/start" | "thread/accessMode/set") {
            return Err(format!("Unsupported for this engine: {method}"));
        }
        if method == "turn/start" && self.threads.lock().await[id].history.active {
            return Err("A Claude Code turn is already running in this thread".into());
        }
        self.set_mode(id, &client, &mode).await?;
        if method == "thread/accessMode/set" {
            return Ok(json!({"result":{}}));
        }
        let config_id = self.threads.lock().await[id].capabilities["configOptions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|o| o["category"] == "model" || o["id"] == "model")
            .and_then(|o| o["id"].as_str())
            .map(str::to_string);
        if let Some(config_id) = config_id {
            client
                .request(
                    "session/set_config_option",
                    json!({"sessionId":session,"configId":config_id,"value":model}),
                )
                .await?;
        } else if self.threads.lock().await[id].metadata["model"] != model {
            return Err(
                "This ACP agent cannot change models; start a new thread with the selected model"
                    .into(),
            );
        }
        let prompt = prompt_input(
            &params["input"],
            client
                .capabilities
                .lock()
                .await
                .pointer("/agentCapabilities/promptCapabilities/image")
                == Some(&json!(true)),
        )?;
        let turn = {
            let mut threads = self.threads.lock().await;
            let t = threads.get_mut(id).unwrap();
            t.metadata["model"] = json!(model);
            if t.metadata["preview"] == "" {
                t.metadata["preview"] = json!(prompt
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
                    .chars()
                    .take(200)
                    .collect::<String>());
            }
            let updates = t.history.start(params["input"].clone());
            t.save()?;
            for event in updates {
                self.emit(workspace, event);
            }
            t.history.turns.last().unwrap().clone()
        };
        client.running.fetch_add(1, Ordering::SeqCst);
        let response = client
            .begin(
                "session/prompt",
                json!({"sessionId":session,"prompt":prompt}),
            )
            .await;
        let (threads, dialogs, sink, events, ws, id) = (
            self.threads.clone(),
            self.dialogs.clone(),
            self.sink.clone(),
            self.events.clone(),
            workspace.to_string(),
            id.to_string(),
        );
        let running = client.clone();
        tokio::spawn(async move {
            let result = match response {
                Ok((_, rx)) => rx
                    .await
                    .unwrap_or_else(|_| Err("ACP response channel closed".to_string().into())),
                Err(e) => Err(e.into()),
            };
            let emit = |event: Value| {
                let _ = events.send(event.clone());
                sink(&ws, event);
            };
            let mut threads = threads.lock().await;
            if let Some(t) = threads.get_mut(&id) {
                let updates = match result {
                    Ok(v) if v["stopReason"] == "error" => t.history.fail_prompt("ACP prompt failed".to_string().into()),
                    Ok(v) => t
                        .history
                        .finish(v["stopReason"].as_str().unwrap_or("end_turn"), None),
                    Err(e) => t.history.fail_prompt(e),
                };
                if let Err(e) = t.save() {
                    emit(t.history.event("error", json!({"error":{"message":e}})));
                }
                for event in updates {
                    emit(event);
                }
            }
            drop(threads);
            dialogs.lock().await.retain(|request,d|{if d.thread!=id{return true;}emit(json!({"method":"serverRequest/resolved","params":{"threadId":id,"requestId":request}}));false});
            running.running.fetch_sub(1, Ordering::SeqCst);
        });
        Ok(json!({"result":{"turn":turn}}))
    }
}
fn engine_capabilities(capabilities: &Value) -> Value {
    let fork = capabilities.pointer("/agentCapabilities/sessionCapabilities/fork")
        .is_some_and(|v| v.is_object() || v == &json!(true));
    json!({"fork":fork,"rewind":false,"steer":capabilities.pointer("/_meta/steering/supported") == Some(&json!(true))})
}
impl EngineBackend for ClaudeAcpEngine {
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
            let id = id.as_str().ok_or("Invalid Claude approval ID")?;
            let dialog = self
                .dialogs
                .lock()
                .await
                .remove(id)
                .ok_or("Claude approval expired")?;
            let (client, connectors) = self
                .runtimes
                .lock()
                .await
                .get(&dialog.workspace)
                .map(|r| (r.client.clone(), r.connectors.clone()))
                .ok_or("Claude Code is not running")?;
            let mut decision = result["decision"].as_str().unwrap_or("cancel");
            if let Some(thread) = self.threads.lock().await.get(&dialog.thread) {
                if permissions::decide(
                    &thread.mode,
                    &thread.history.cwd,
                    &connectors,
                    &dialog.params["toolCall"],
                ) == permissions::Decision::Reject
                {
                    decision = "decline";
                }
            } else {
                decision = "decline";
            }
            if decision == "acceptForSession" {
                if !permissions::tool_name(&dialog.params["toolCall"]).starts_with("mcp__") {
                    if let Some(thread) = self.threads.lock().await.get_mut(&dialog.thread) {
                        thread
                            .grants
                            .insert(permissions::grant_key(&dialog.params["toolCall"]));
                    }
                }
                decision = "accept";
            }
            client
                .respond(
                    dialog.request_id,
                    permissions::outcome(&dialog.params, decision),
                )
                .await?;
            self.emit(&dialog.workspace,json!({"method":"serverRequest/resolved","params":{"threadId":dialog.thread,"requestId":id}}));
            Ok(())
        })
    }
    fn events(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }
    fn shutdown(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            for (_, r) in self.runtimes.lock().await.drain() {
                r.client.shutdown().await?;
            }
            Ok(())
        })
    }
}
fn thread_response(thread: &Thread) -> Value {
    json!({"thread":thread.metadata,"model":thread.metadata["model"],"modelProvider":thread.metadata["modelProvider"],"cwd":thread.history.cwd,"approvalPolicy":if thread.mode=="full-access"{"never"}else{"on-request"},"sandbox":{"type":match thread.mode.as_str(){"read-only"=>"readOnly","full-access"=>"dangerFullAccess",_=>"workspaceWrite"}},"reasoningEffort":null,"initialTurnsPage":null,"turnsBackwardsCursor":null,"itemsBackwardsCursor":null})
}
fn page(data: Vec<Value>, params: &Value) -> Result<(Vec<Value>, Option<String>), String> {
    let start = params["cursor"]
        .as_str()
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| "Invalid history cursor")?
        .unwrap_or(0);
    let end = start
        .saturating_add(params["limit"].as_u64().unwrap_or(100).clamp(1, 1000) as usize)
        .min(data.len());
    Ok((
        data.get(start..end).unwrap_or(&[]).to_vec(),
        (end < data.len()).then(|| end.to_string()),
    ))
}
fn prompt_input(input: &Value, images: bool) -> Result<Value, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut prompt = vec![];
    for part in input.as_array().ok_or("ACP input must be an array")? {
        match part["type"].as_str() {
            Some("text") => prompt.push(
                json!({"type":"text","text":part["text"].as_str().ok_or("Invalid prompt text")?}),
            ),
            Some("image" | "localImage") if images => {
                let (mime, data) = if part["type"] == "image" {
                    let (mime, data) = part["url"]
                        .as_str()
                        .and_then(|v| v.strip_prefix("data:"))
                        .and_then(|v| v.split_once(";base64,"))
                        .ok_or("ACP images require a base64 data URL or localImage")?;
                    STANDARD.decode(data).map_err(|_| "Invalid image base64")?;
                    (mime.to_string(), data.to_string())
                } else {
                    let path = Path::new(part["path"].as_str().ok_or("Missing image path")?);
                    let mime = match path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "png" => "image/png",
                        "jpg" | "jpeg" => "image/jpeg",
                        "webp" => "image/webp",
                        "gif" => "image/gif",
                        _ => return Err("Unsupported image format".into()),
                    };
                    (
                        mime.into(),
                        STANDARD.encode(std::fs::read(path).map_err(|e| e.to_string())?),
                    )
                };
                prompt.push(json!({"type":"image","mimeType":mime,"data":data}));
            }
            Some("image" | "localImage") => {
                return Err("This ACP agent does not support images".into())
            }
            _ => return Err("ACP supports text and image input".into()),
        }
    }
    if prompt.is_empty() {
        return Err("A prompt is required".into());
    }
    Ok(json!(prompt))
}
