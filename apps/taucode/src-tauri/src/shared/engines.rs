use super::providers::{self, BoxFuture, KeyStore};
use crate::{backend::app_server::CodexEngine, types::*};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex};

/// Engine adapters exchange canonical app-server envelopes. History is paged
/// through thread/turns/list; server requests retain their original IDs.
/// Capability matrix (frontend: threads/utils/engineCapabilities.ts):
/// Engine | fork latest | fork item | conversation rewind | side chats | tree | checkpoints | steer | compact
/// Pi     | yes         | yes       | yes                 | yes        | yes  | yes         | yes   | yes
/// Codex  | yes         | no        | rollback probe      | latest     | yes  | yes         | yes   | yes
/// Claude | advertised  | no        | no (latest fork only)| advertised | Task | yes         | advertised | no
#[allow(dead_code)]
pub(crate) trait EngineBackend: Send + Sync {
    fn list_models<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "model/list", params)
    }
    fn set_name<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/name/set", params)
    }
    fn fork<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/fork", params)
    }
    fn compact<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/compact/start", params)
    }
    fn interrupt<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "turn/interrupt", params)
    }
    fn steer<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "turn/steer", params)
    }
    fn send_turn<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "turn/start", params)
    }
    fn list_history<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/turns/list", params)
    }
    fn list_threads<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/list", params)
    }
    fn read_thread<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/read", params)
    }
    fn resume_thread<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/resume", params)
    }
    fn start_thread<'a>(
        &'a self,
        workspace: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        self.request(workspace, "thread/start", params)
    }
    fn request<'a>(
        &'a self,
        workspace: &'a str,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>>;
    fn respond_to_server_request(
        &self,
        id: Value,
        result: Value,
    ) -> BoxFuture<'_, Result<(), String>>;
    fn events(&self) -> broadcast::Receiver<Value>;
    fn shutdown(&self) -> BoxFuture<'_, Result<(), String>>;
}
impl EngineBackend for CodexEngine {
    fn request<'a>(
        &'a self,
        workspace: &'a str,
        method: &'a str,
        params: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(self.send_request_for_workspace(workspace, method, params))
    }
    fn respond_to_server_request(
        &self,
        id: Value,
        result: Value,
    ) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(self.send_response(id, result))
    }
    fn events(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }
    fn shutdown(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async {
            crate::shared::process_core::kill_child_process_tree(&mut *self.child.lock().await)
                .await;
            Ok(())
        })
    }
}
pub(crate) use super::pi::PiEngine;
pub(crate) struct WorkspaceSession {
    pub checkpoints: Option<Arc<super::checkpoints::Checkpoints>>,
    pub codex: Mutex<Option<Arc<CodexEngine>>>,
    configuration: Mutex<Option<String>>,
    reload_threads: Mutex<HashSet<String>>,
    #[cfg(test)]
    test_backend: Option<Arc<CodexEngine>>,
    factory:
        Option<Box<dyn Fn() -> BoxFuture<'static, Result<Arc<CodexEngine>, String>> + Send + Sync>>,
    pub codegraph: Option<crate::codegraph::CodegraphSetup>,
    pub codex_args: Option<String>,
    pub owner_workspace_id: String,
    pub pi: PiEngine,
    pub(super) extra_pi: Mutex<HashMap<String, Arc<PiEngine>>>,
    extra_acp: Mutex<HashMap<String, Arc<super::acp::ClaudeAcpEngine>>>,
    pub claude: super::acp::ClaudeAcpEngine,
    pub owners: Mutex<HashMap<String, EngineKind>>,
    pub thread_providers: Mutex<HashMap<String, String>>,
    pub providers: Vec<ProviderEntry>,
    pub policy: AgentPolicy,
}
#[cfg(test)]
impl std::ops::Deref for WorkspaceSession {
    type Target = CodexEngine;
    fn deref(&self) -> &CodexEngine {
        self.test_backend.as_ref().expect("fixture engine")
    }
}
impl WorkspaceSession {
    #[cfg(test)]
    pub fn new(
        codex: Arc<CodexEngine>,
        providers: Vec<ProviderEntry>,
        policy: AgentPolicy,
    ) -> Self {
        Self {
            codegraph: codex.codegraph.clone(),
            codex_args: codex.codex_args.clone(),
            owner_workspace_id: codex.owner_workspace_id.clone(),
            test_backend: Some(codex.clone()),
            codex: Mutex::new(Some(codex)),
            configuration: Mutex::new(None),
            reload_threads: Mutex::new(HashSet::new()),
            factory: None,
            checkpoints: None,
            pi: PiEngine::default(),
            extra_pi: Mutex::new(HashMap::new()),
            extra_acp: Mutex::new(HashMap::new()),
            claude: super::acp::ClaudeAcpEngine::default(),
            owners: Mutex::new(HashMap::new()),
            thread_providers: Mutex::new(HashMap::new()),
            providers,
            policy,
        }
    }
    pub fn lazy(
        factory: Box<
            dyn Fn() -> BoxFuture<'static, Result<Arc<CodexEngine>, String>> + Send + Sync,
        >,
        providers: Vec<ProviderEntry>,
        policy: AgentPolicy,
        codex_args: Option<String>,
        codegraph: Option<crate::codegraph::CodegraphSetup>,
        owner_workspace_id: String,
    ) -> Self {
        Self {
            codex: Mutex::new(None),
            configuration: Mutex::new(None),
            reload_threads: Mutex::new(HashSet::new()),
            #[cfg(test)]
            test_backend: None,
            factory: Some(factory),
            checkpoints: None,
            codegraph,
            codex_args,
            owner_workspace_id,
            pi: PiEngine::default(),
            extra_pi: Mutex::new(HashMap::new()),
            extra_acp: Mutex::new(HashMap::new()),
            claude: super::acp::ClaudeAcpEngine::default(),
            owners: Mutex::new(HashMap::new()),
            thread_providers: Mutex::new(HashMap::new()),
            providers,
            policy,
        }
    }
    fn current_configuration(&self) -> (Vec<ProviderEntry>, AgentPolicy) {
        self.configuration_for(&self.owner_workspace_id)
    }
    fn configuration_for(&self, workspace: &str) -> (Vec<ProviderEntry>, AgentPolicy) {
        if let Some(home) = crate::codex::home::resolve_default_codex_home() {
            if let Some(data) = home.parent().and_then(Path::parent) {
                if let Ok(settings) = crate::storage::read_settings(&data.join("settings.json")) {
                    let workspaces = crate::storage::read_workspaces(&data.join("workspaces.json"))
                        .unwrap_or_default();
                    let over = workspaces
                        .get(workspace)
                        .and_then(|w| w.settings.agent_policy.as_ref());
                    return (
                        settings.providers.clone(),
                        feature_policy(providers::effective_policy(&settings.agent_policy, over), &settings.features()),
                    );
                }
            }
        }
        (self.providers.clone(), self.policy.clone())
    }
    async fn prepare_engines(&self, workspace: &str, settings: &AppSettings) -> Result<(), String> {
        let path = self.pi.paths.lock().await.get(workspace).cloned();
        for definition in super::engine_registry::registry(settings).into_iter().filter(|e| !matches!(e.id.as_str(), "pi" | "claude" | "codex")) {
            match definition.protocol {
                EngineProtocol::PiRpc => {
                    let mut engines = self.extra_pi.lock().await;
                    let backend = engines.entry(definition.id.clone()).or_insert_with(|| Arc::new(self.pi.with_definition(definition)));
                    if let Some(path) = &path { backend.paths.lock().await.insert(workspace.into(), path.clone()); }
                },
                EngineProtocol::Acp => {
                    let mut engines = self.extra_acp.lock().await;
                    let backend = engines.entry(definition.id.clone()).or_insert_with(|| Arc::new(self.claude.with_definition(definition)));
                    if let Some(path) = &path { backend.paths.lock().await.insert(workspace.into(), path.clone()); }
                },
                _ => (),
            }
        }
        Ok(())
    }
    pub async fn codex_backend(&self) -> Result<Arc<CodexEngine>, String> {
        self.backend(false).await
    }
    pub async fn credentials_changed(&self) {
        // Reload child environment on the next request, after active turns finish.
        *self.configuration.lock().await = Some(String::new());
        self.pi.credentials_changed().await;
        self.claude.credentials_changed().await;
    }
    async fn backend(&self, refresh: bool) -> Result<Arc<CodexEngine>, String> {
        let mut backend = self.codex.lock().await;
        let mut fingerprint = self.configuration.lock().await;
        let connectors = crate::codex::home::resolve_default_codex_home()
            .and_then(|home| home.parent().and_then(Path::parent).map(Path::to_path_buf))
            .map(|data| {
                super::connectors::load(&data, &self.owner_workspace_id, self.codegraph.as_ref())
            })
            .transpose()?;
        let current = serde_json::to_string(&(self.current_configuration(), connectors))
            .map_err(|e| e.to_string())?;
        if refresh && self.factory.is_some() && fingerprint.as_ref().is_some_and(|v| *v != current)
        {
            if let Some(c) = backend.as_ref() {
                if !c.active_threads.lock().await.is_empty() {
                    return Err("Provider or policy settings changed. Finish or interrupt running turns, then retry.".into());
                }
                c.shutdown().await?;
            }
            *backend = None;
            *self.reload_threads.lock().await = self
                .owners
                .lock()
                .await
                .iter()
                .filter(|(_, engine)| **engine == "codex".to_owned())
                .map(|(id, _)| id.clone())
                .collect();
        }
        if backend.is_none() {
            *backend = Some((self.factory.as_ref().ok_or("Missing codex backend")?)().await?);
            *fingerprint = Some(current);
        }
        Ok(backend.as_ref().unwrap().clone())
    }
    pub async fn is_alive(&self) -> bool {
        match self.codex.lock().await.as_ref() {
            Some(c) => matches!(c.child.lock().await.try_wait(), Ok(None)),
            None => true,
        }
    }
    pub async fn shutdown(&self) {
        if let Some(c) = self.codex.lock().await.take() {
            let _ = c.shutdown().await;
        }
        for backend in self.extra_pi.lock().await.values() { let _ = backend.shutdown().await; }
        for backend in self.extra_acp.lock().await.values() { let _ = backend.shutdown().await; }
        let _ = self.pi.shutdown().await;
        let _ = self.claude.shutdown().await;
    }
    pub async fn register_workspace_with_path(&self, id: &str, path: Option<&str>) {
        if let Some(path) = path {
            if let Some(checkpoints) = &self.checkpoints { checkpoints.register(id, path); }
            self.pi.paths.lock().await.insert(id.into(), path.into());
            self.claude.paths.lock().await.insert(id.into(), path.into());
        }
        if let Some(c) = self.codex.lock().await.as_ref() {
            c.register_workspace_with_path(id, path).await;
        }
    }
    pub async fn unregister_workspace(&self, id: &str) {
        if let Some(c) = self.codex.lock().await.as_ref() {
            c.unregister_workspace(id).await;
        }
    }
    pub async fn send_request_for_workspace(&self, workspace: &str, method: &str, params: Value) -> Result<Value, String> {
        let thread = params["threadId"].as_str().unwrap_or("").to_owned();
        if method == "turn/start" && super::engine_registry::features(self.pi.data.as_deref()).checkpoints {
            if let Some(checkpoints) = &self.checkpoints {
                let engine = if self.claude.owns(workspace, &thread).await { "claude" } else if self.pi.owns(workspace, &thread).await { "pi" } else { "codex" };
                checkpoints.before(workspace, &thread, engine, &params).await?;
            }
        }
        let mut result = self.request_inner(workspace, method, params).await;
        if method == "thread/list" {
            if let Ok(response) = &mut result {
                if let Some(rows) = response.pointer_mut("/result/data").and_then(Value::as_array_mut) {
                    for (id, backend) in self.extra_pi.lock().await.iter() {
                        let page = backend.request(workspace, method, json!({})).await?;
                        for mut row in page["result"]["data"].as_array().into_iter().flatten().cloned() { row["engine"] = json!(id); rows.push(row); }
                    }
                    for (id, backend) in self.extra_acp.lock().await.iter() {
                        let page = backend.request(workspace, method, json!({})).await?;
                        for mut row in page["result"]["data"].as_array().into_iter().flatten().cloned() { row["engine"] = json!(id); rows.push(row); }
                    }
                    rows.sort_by_key(|t| std::cmp::Reverse(t["updatedAt"].as_i64().unwrap_or(0)));
                }
            }
        }
        if method == "turn/start" && (result.is_err() || result.as_ref().is_ok_and(|r| r.get("error").is_some())) {
            if let Some(checkpoints) = &self.checkpoints { checkpoints.complete(workspace, &thread); }
        }
        result
    }
    async fn request_inner(
        &self,
        workspace: &str,
        method: &str,
        mut params: Value,
    ) -> Result<Value, String> {
        let settings = super::engine_registry::settings(self.pi.data.as_deref());
        let features = settings.features();
        if (method == "thread/sidechat" && !features.side_chats) || (method.starts_with("subagent/") && !features.subagents) { return Err("This feature is disabled in Settings".into()); }
        let (providers, policy) = self.configuration_for(workspace);
        let policy = feature_policy(policy, &features);
        self.prepare_engines(workspace, &settings).await?;
        self.pi
            .configure(workspace, providers.clone(), policy.clone())
            .await;
        self.claude.configure(workspace, providers.clone(), policy.clone()).await;
        if method == "thread/sidechat" {
            return super::sidechats::create(self, workspace, params).await;
        }
        let claude_thread = if let Some(id) = params["threadId"].as_str() {
            self.claude.owns(workspace, id).await
        } else { false };
        if method == "model/list" {
            if claude_thread || params["engine"] == "claude" {
                return self.claude.request(workspace, method, params).await;
            }
            return Ok(providers::catalog(&providers));
        }
        let thread_id = params
            .get("threadId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let pi_thread = if let Some(id) = thread_id.as_ref() {
            self.pi.owns(workspace, id).await
        } else {
            false
        };
        if let Some(id) = thread_id.as_ref() {
            for (name, backend) in self.extra_pi.lock().await.iter() {
                if backend.owns(workspace, id).await { self.owners.lock().await.insert(id.clone(), name.clone()); break; }
            }
            for (name, backend) in self.extra_acp.lock().await.iter() {
                if backend.owns(workspace, id).await { self.owners.lock().await.insert(id.clone(), name.clone()); break; }
            }
        }
        let owners = self.owners.lock().await;
        let engine = if method == "thread/start" {
            params
                .get("engine")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| "Unknown engine")?
                .unwrap_or_else(|| policy.default_engine.clone())
        } else {
            thread_id
                .as_ref()
                .and_then(|id| owners.get(id).cloned())
                .unwrap_or(if claude_thread {
                    "claude".to_owned()
                } else if pi_thread {
                    "pi".to_owned()
                } else {
                    "codex".to_owned()
                })
        };
        drop(owners);
        if let Some(obj) = params.as_object_mut() {
            obj.remove("engine");
        }
        let (definition, notice) = super::engine_registry::resolve(&settings, &engine)?;
        let engine = definition.id.clone();
        let mutating = matches!(method, "turn/start" | "turn/steer" | "thread/start" | "thread/fork" | "thread/compact/start" | "thread/accessMode/set");
        if !definition.enabled && mutating { return Err(format!("Enable {} to continue", definition.name)); }
        let method = if !definition.enabled && method == "thread/resume" { "thread/read" } else { method };
        if definition.protocol != EngineProtocol::CodexAppServer && method != "thread/list" {
            let mut response = match definition.protocol {
                EngineProtocol::PiRpc if engine == "pi" => self.pi.request(workspace, method, params).await?,
                EngineProtocol::PiRpc => {
                    let backend = self.extra_pi.lock().await.get(&engine).cloned().ok_or("Missing pi-RPC engine")?;
                    backend.configure(workspace, providers.clone(), policy.clone()).await;
                    backend.request(workspace, method, params).await?
                },
                EngineProtocol::Acp if engine == "claude" => self.claude.request(workspace, method, params).await?,
                EngineProtocol::Acp => {
                    let backend = self.extra_acp.lock().await.get(&engine).cloned().ok_or("Missing ACP engine")?;
                    backend.configure(workspace, providers.clone(), policy.clone()).await;
                    backend.request(workspace, method, params).await?
                },
                _ => unreachable!(),
            };
            if let Some(thread) = response.pointer_mut("/result/thread") {
                thread["engine"] = json!(engine);
                thread["canAcceptDirectInput"] = json!(definition.enabled);
                if !definition.enabled { thread["engineNotice"] = json!(format!("Enable {} to continue", definition.name)); }
                if let Some(notice) = notice { thread["engineNotice"] = json!(notice); }
                if let Some(id) = thread["id"].as_str() { self.owners.lock().await.insert(id.into(), engine); }
            }
            if matches!(method, "thread/read" | "thread/resume" | "thread/fork" | "thread/start") { super::sidechats::enrich(self, workspace, &mut response)?; }
            return Ok(response);
        }
        if matches!(method, "turn/start" | "thread/accessMode/set") {
            let id = thread_id.as_deref().ok_or("Missing thread ID")?;
            let path = self.pi.data.as_ref().map(|_| super::sidechats::metadata_path(self, workspace, id)).transpose()?;
            let mut metadata = match &path {
                Some(path) => crate::storage::read_thread_metadata(path)?,
                None => json!({}),
            };
            if !metadata.is_object() { return Err("Invalid thread metadata".into()); }
            if params.get("accessMode").is_some_and(|v| !v.is_string())
                || method == "thread/accessMode/set" && params.get("accessMode").is_none()
            { return Err("Invalid access mode".into()); }
            let requested = params["accessMode"].as_str().or_else(|| match params["sandboxPolicy"]["type"].as_str() {
                Some("readOnly") => Some("read-only"),
                Some("workspaceWrite") => Some("ask"),
                Some("dangerFullAccess") => Some("full-access"),
                _ => None,
            }).unwrap_or("current");
            let mode = if requested == "current" {
                metadata.get("accessMode").map(|v| v.as_str().ok_or("Invalid saved access mode")).transpose()?
                    .unwrap_or(if metadata.get("sideOf").is_some() { "read-only" } else { "ask" })
            } else { requested }.to_owned();
            if !matches!(mode.as_str(), "read-only" | "ask" | "full-access") {
                return Err("Invalid access mode".into());
            }
            metadata["accessMode"] = json!(mode);
            if let Some(path) = path { crate::storage::write_thread_metadata(&path, &metadata)?; }
            else if method == "thread/accessMode/set" { return Err("Thread metadata storage unavailable".into()); }
            if method == "thread/accessMode/set" { return Ok(json!({"result":{}})); }
            params.as_object_mut().unwrap().remove("accessMode");
            params["approvalPolicy"] = json!(if mode == "ask" { "on-request" } else { "never" });
            params["sandboxPolicy"] = match mode.as_str() {
                "read-only" => json!({"type":"readOnly"}),
                "full-access" => json!({"type":"dangerFullAccess"}),
                _ if params["sandboxPolicy"]["type"] == "workspaceWrite" => params["sandboxPolicy"].clone(),
                _ => json!({"type":"workspaceWrite","writableRoots":params["cwd"].as_str().into_iter().collect::<Vec<_>>(),"networkAccess":true}),
            };
        }
        if method == "turn/start" && params.get("model").and_then(Value::as_str).is_none() {
            let id = thread_id.as_deref().ok_or("Missing thread ID")?;
            let known = self.thread_providers.lock().await.get(id).cloned();
            let current = if let Some(current) = known {
                current
            } else {
                let response = self
                    .backend(true)
                    .await?
                    .read_thread(workspace, json!({"threadId":id}))
                    .await?;
                response
                    .pointer("/result/thread/modelProvider")
                    .and_then(Value::as_str)
                    .ok_or("Cannot determine thread provider; resume the thread first")?
                    .to_owned()
            };
            let provider = providers
                .iter()
                .find(|p| p.id == current && p.enabled && providers::codex_usable(p))
                .ok_or("Enable this thread's provider in Settings before sending")?;
            let model = provider
                .models
                .iter()
                .find(|m| m.enabled)
                .ok_or("Enable a registry model before sending")?;
            params["model"] = json!(format!("{}/{}", provider.id, model.id));
        }
        if method == "thread/start" && params.get("model").is_none() {
            if let Some(m) = &policy.default_model {
                params["model"] = json!(format!("{}/{}", m.provider_id, m.model_id));
            }
        }
        if let Some(model) = params
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            if let Some((provider, model)) = model.split_once('/') {
                let p = providers
                    .iter()
                    .find(|p| p.id == provider && p.enabled && providers::codex_usable(p))
                    .ok_or("Model is unavailable for codex")?;
                if !p.models.iter().any(|m| m.id == model && m.enabled) {
                    return Err("Enable the registry model first".into());
                }
                if method == "turn/start" {
                    let id = thread_id.as_deref().ok_or("Missing thread ID")?;
                    let known = self.thread_providers.lock().await.get(id).cloned();
                    let current = if let Some(current) = known {
                        current
                    } else {
                        let response = self
                            .backend(true)
                            .await?
                            .read_thread(workspace, json!({"threadId":id}))
                            .await?;
                        response
                            .pointer("/result/thread/modelProvider")
                            .and_then(Value::as_str)
                            .ok_or("Cannot determine thread provider; resume the thread first")?
                            .to_owned()
                    };
                    if current != provider {
                        return Err("Fork this codex thread to switch providers".into());
                    }
                }
                if method == "turn/start" {
                    if let Some(cap) = p.models.iter().find(|m| m.id == model).and_then(|m| m.reasoning.as_ref()) {
                        let effort = super::reasoning::clamp(cap, params["effort"].as_str());
                        params["effort"] = json!(effort.as_deref().map(|v| if v == "off" { "none" } else { v }));
                        if params["collaborationMode"]["settings"].is_object() { params["collaborationMode"]["settings"]["reasoning_effort"] = params["effort"].clone(); }
                    }
                }
                params["model"] = json!(model);
                if method != "turn/start" {
                    params["modelProvider"] = json!(provider);
                }
            } else if !providers.is_empty() {
                return Err("Select a registry model (provider/model)".into());
            }
        }
        if method == "thread/start"
            && !providers.iter().any(|p| {
                p.enabled && providers::codex_usable(p) && p.models.iter().any(|m| m.enabled)
            })
        {
            return Err("Add and enable a codex-compatible provider and model in Settings".into());
        }
        if method == "thread/list"
            && !providers.iter().any(|p| {
                p.enabled && providers::codex_usable(p) && p.models.iter().any(|m| m.enabled)
            })
            && self.codex.lock().await.is_none()
        {
            let mut response = self.pi.request(workspace, method, params).await?;
            let claude = self.claude.request(workspace, method, json!({})).await?;
            if let Some(data) = response.pointer_mut("/result/data").and_then(Value::as_array_mut) {
                data.extend(claude["result"]["data"].as_array().into_iter().flatten().cloned());
                data.sort_by_key(|t| std::cmp::Reverse(t["updatedAt"].as_i64().unwrap_or(0)));
            }
            super::sidechats::enrich(self, workspace, &mut response)?;
            return Ok(response);
        }
        let backend = self
            .backend(!matches!(method, "turn/interrupt" | "turn/steer"))
            .await?;
        if method == "subagent/transcript" {
            let node = params["nodeId"].as_str().ok_or("Missing node ID")?;
            let parent = thread_id.as_deref().ok_or("Missing parent thread")?;
            let mut pending = vec![parent.to_owned()];
            let mut visited = HashSet::new();
            let mut found = None;
            while let Some(id) = pending.pop() {
                if !visited.insert(id.clone()) { continue; }
                let response = backend.read_thread(workspace, json!({"threadId":id,"includeTurns":true})).await?;
                if id == node && id != parent { found = Some(response); break; }
                pending.extend(collab_children(&response));
            }
            let response = found.ok_or("Node is not a child of this thread")?;
            let messages = response.pointer("/result/thread/turns").and_then(Value::as_array)
                .ok_or("Child thread unavailable")?.iter()
                .flat_map(|t| t["items"].as_array().into_iter().flatten()).cloned();
            return Ok(super::pi::agents::capped(messages));
        }
        // The engine protocol must not receive taucode-only fork metadata.
        if engine == "codex".to_owned() {
            if let Some(params) = params.as_object_mut() { params.remove("sideOf"); params.remove("atItemId"); }
        }
        if matches!(
            method,
            "turn/start" | "thread/compact/start" | "thread/name/set"
        ) {
            if let Some(id) = thread_id.as_ref() {
                let mut reload = self.reload_threads.lock().await;
                if reload.contains(id) {
                    backend
                        .resume_thread(workspace, json!({"threadId":id}))
                        .await?;
                    reload.remove(id);
                }
            }
        }
        let mut response = backend.request(workspace, method, params).await?;
        if method == "thread/resume" {
            if let Some(id) = thread_id.as_ref() {
                self.reload_threads.lock().await.remove(id);
            }
        }
        if let Some(thread) = response.pointer_mut("/result/thread") {
            thread["engine"] = json!("codex");
            if !definition.enabled {
                thread["canAcceptDirectInput"] = json!(false);
                thread["engineNotice"] = json!(format!("Enable {} to continue", definition.name));
            }
            if let Some(notice) = notice { thread["engineNotice"] = json!(notice); }
            if let (Some(id), Some(provider)) = (
                thread.get("id").and_then(Value::as_str),
                thread.get("modelProvider").and_then(Value::as_str),
            ) {
                self.thread_providers
                    .lock()
                    .await
                    .insert(id.into(), provider.into());
            }
            if let Some(id) = thread.get("id").and_then(Value::as_str) {
                self.owners
                    .lock()
                    .await
                    .insert(id.into(), "codex".to_owned());
            }
        }
        if method == "thread/list" {
            let pi = self.pi.request(workspace, method, json!({})).await?;
            let claude = self.claude.request(workspace, method, json!({})).await?;
            if let Some(data) = response
                .pointer_mut("/result/data")
                .and_then(Value::as_array_mut)
            {
                for t in data.iter_mut() {
                    t["engine"] = json!("codex");
                    if let Some(id) = t.get("id").and_then(Value::as_str) {
                        self.owners
                            .lock()
                            .await
                            .insert(id.into(), "codex".to_owned());
                    }
                }
                data.extend(
                    pi.pointer("/result/data")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                );
                data.extend(claude["result"]["data"].as_array().into_iter().flatten().cloned());
                data.sort_by_key(|t| {
                    std::cmp::Reverse(t.get("updatedAt").and_then(Value::as_i64).unwrap_or(0))
                });
            }
        }
        if matches!(method, "thread/list" | "thread/read" | "thread/resume" | "thread/fork" | "thread/start") { super::sidechats::enrich(self, workspace, &mut response)?; }
        Ok(response)
    }
    pub async fn send_response(&self, id: Value, result: Value) -> Result<(), String> {
        for backend in self.extra_pi.lock().await.values() {
            if backend.respond_to_server_request(id.clone(), result.clone()).await.is_ok() { return Ok(()); }
        }
        for backend in self.extra_acp.lock().await.values() {
            if backend.respond_to_server_request(id.clone(), result.clone()).await.is_ok() { return Ok(()); }
        }
        if id.as_str().is_some_and(|id| id.starts_with("claude:")) {
            return self.claude.respond_to_server_request(id, result).await;
        }
        if id.as_str().is_some_and(|id| id.starts_with("pi:")) {
            return self.pi.respond_to_server_request(id, result).await;
        }
        self.codex_backend()
            .await?
            .respond_to_server_request(id, result)
            .await
    }
}
fn collab_children(response: &Value) -> Vec<String> {
    response.pointer("/result/thread/turns").and_then(Value::as_array).into_iter().flatten()
        .flat_map(|turn| turn["items"].as_array().into_iter().flatten())
        .filter(|item| item["type"] == "collabAgentToolCall")
        .flat_map(|item| item["receiverThreadIds"].as_array().into_iter().flatten())
        .filter_map(Value::as_str).map(str::to_owned).collect()
}
pub(crate) fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("string serialization")
}
pub(crate) fn key_env(id: &str) -> String {
    format!(
        "TAUCODE_PROVIDER_{}_KEY",
        id.to_ascii_uppercase().replace('-', "_")
    )
}
pub(crate) fn codex_config(
    providers: &[ProviderEntry],
    policy: &AgentPolicy,
    home: &Path,
) -> Result<(String, Vec<(PathBuf, String)>), String> {
    providers::validate(providers, policy)?;
    let mut config = String::from("cli_auth_credentials_store = \"ephemeral\"\nmcp_oauth_credentials_store = \"file\"\ncheck_for_update_on_startup = false\n[analytics]\nenabled = false\n[feedback]\nenabled = false\n[apps._default]\nenabled = false\n[plugins._default]\nenabled = false\n[features]\napps = false\nplugins = false\n");
    config.push_str(&format!("multi_agent = {}\n", policy.subagents.enabled));
    let mut files = vec![];
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(policy)
        .map_err(|e| e.to_string())?
        .hash(&mut hash);
    let role_dir = home.join("roles").join(format!("{:016x}", hash.finish()));
    for p in providers
        .iter()
        .filter(|p| p.enabled && providers::codex_usable(p))
    {
        config.push_str(&format!("\n[model_providers.{}]\nname = {}\nbase_url = {}\nwire_api = \"responses\"\nsupports_websockets = false\nrequires_openai_auth = false\n", quote(&p.id), quote(&p.name), quote(providers::base_url(p))));
        match &p.auth {
            ProviderAuth::ApiKey => config.push_str(&format!("env_key = {}\n", quote(&key_env(&p.id)))),
            ProviderAuth::Env { var } => config.push_str(&format!("env_key = {}\n", quote(var))),
            ProviderAuth::Command { .. } => config.push_str(&format!("env_key = {}\n", quote(&key_env(&p.id)))),
            ProviderAuth::None => (),
        }
        if !p.extra_headers.is_empty() {
            config.push_str(&format!(
                "[model_providers.{}.http_headers]\n",
                quote(&p.id)
            ));
            for (k, v) in &p.extra_headers {
                config.push_str(&format!("{} = {}\n", quote(k), quote(v)));
            }
        }
    }
    let usable_model = |m: &ModelRef| {
        providers
            .iter()
            .any(|p| p.id == m.provider_id && p.enabled && providers::codex_usable(p))
    };
    let default = policy
        .default_model
        .as_ref()
        .filter(|m| usable_model(m))
        .cloned()
        .or_else(|| {
            providers
                .iter()
                .find(|p| {
                    p.enabled && providers::codex_usable(p) && p.models.iter().any(|m| m.enabled)
                })
                .and_then(|p| {
                    p.models.iter().find(|m| m.enabled).map(|m| ModelRef {
                        provider_id: p.id.clone(),
                        model_id: m.id.clone(),
                    })
                })
        });
    if let Some(m) = default {
        config = format!(
            "model_provider = {}\nmodel = {}\n{config}",
            quote(&m.provider_id),
            quote(&m.model_id)
        );
    }
    let s = &policy.subagents;
    config.push_str(&format!(
        "\n[agents]\nmax_threads = {}\nmax_depth = {}\n",
        if s.enabled { s.max_concurrent } else { 1 },
        s.max_depth
    ));
    if let Some(m) = s.default_model.as_ref().filter(|m| usable_model(m)) {
        config.push_str(&format!(
            "default_subagent_model = {}\n",
            quote(&m.model_id)
        ));
    }
    for role in &s.roles {
        let path = role_dir.join(format!("{}.toml", role.name));
        let mut content = format!(
            "developer_instructions = {}\nsandbox_mode = {}\n",
            quote(&role.instructions),
            quote(if role.tool_access == ToolAccess::ReadOnly {
                "read-only"
            } else {
                "workspace-write"
            })
        );
        if let Some(m) = &role.model {
            if !usable_model(m) {
                continue;
            }
            content.push_str(&format!(
                "model = {}\nmodel_provider = {}\n",
                quote(&m.model_id),
                quote(&m.provider_id)
            ));
        }
        config.push_str(&format!(
            "[agents.{}]\ndescription = {}\nconfig_file = {}\n",
            quote(&role.name),
            quote(&role.description),
            quote(&path.to_string_lossy())
        ));
        files.push((path, content));
    }
    Ok((config, files))
}
pub(crate) fn prepare_codex(
    home: &Path,
    providers: &[ProviderEntry],
    policy: &AgentPolicy,
    keys: &dyn KeyStore,
) -> Result<Vec<(String, String)>, String> {
    let (config, files) = codex_config(providers, policy, home)?;
    if home.join(".credentials.json").exists() {
        return Err("Remove .credentials.json from taucode's isolated Codex home; MCP OAuth sign-in is not supported".into());
    }
    std::fs::create_dir_all(home.join("roles")).map_err(|e| e.to_string())?;
    // This directory is exclusively taucode-owned. Never import user credentials.
    let auth = home.join("auth.json");
    if auth.exists() {
        std::fs::remove_file(auth).map_err(|e| e.to_string())?;
    }
    write_config_file(&home.join("config.toml"), &config)?;
    for (path, content) in files {
        std::fs::create_dir_all(path.parent().ok_or("Missing role directory")?)
            .map_err(|e| e.to_string())?;
        write_config_file(&path, &content)?;
    }
    let mut env = vec![];
    for p in providers
        .iter()
        .filter(|p| p.enabled && providers::codex_usable(p))
    {
        match &p.auth {
            ProviderAuth::ApiKey => {
                if let Some(key) = keys.get(&p.id)? {
                    env.push((key_env(&p.id), key));
                }
            }
            ProviderAuth::Env { var } => {
                if let Ok(key) = std::env::var(var) {
                    env.push((var.clone(), key));
                }
            }
            _ => (),
        }
    }
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn codex_side_chat_access_survives_current_omission_overrides_and_restart() {
        use crate::shared::codex_core::{send_user_message_core, set_thread_access_mode_core};
        use tokio::io::{AsyncBufReadExt, BufReader};
        let data = std::env::temp_dir().join(format!("codex-access-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data).unwrap();
        let provider: ProviderEntry = serde_json::from_value(json!({"id":"test","name":"Test","kind":"openai","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"model","enabled":true}]})).unwrap();
        let backend = make_session().codex_backend().await.unwrap();
        let session = || {
            let mut session = WorkspaceSession::new(backend.clone(), vec![provider.clone()], AgentPolicy::default());
            session.pi = PiEngine::new(data.clone(), "fixture".into(), data.to_string_lossy().into(), None, Arc::new(|_, _| {}));
            Arc::new(session)
        };
        let stdout = backend.child.lock().await.stdout.take().unwrap();
        let responder = backend.clone();
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                let request: Value = serde_json::from_str(&line).unwrap();
                let result = match request["method"].as_str().unwrap() {
                    "thread/fork" => {
                        assert_eq!(request["params"]["sandbox"], "read-only");
                        json!({"thread":{"id":"side","modelProvider":"test"}})
                    }
                    "thread/read" => json!({"thread":{"id":request["params"]["threadId"],"modelProvider":"test"}}),
                    "turn/start" => {
                        assert!(request["params"].get("accessMode").is_none());
                        json!({"seen":request["params"]})
                    }
                    method => panic!("Unexpected Codex request: {method}"),
                };
                responder.pending.lock().await.remove(&request["id"].as_u64().unwrap()).unwrap()
                    .send(json!({"result":result})).unwrap();
            }
        });
        let first = session();
        first.send_request_for_workspace("fixture", "thread/accessMode/set", json!({"threadId":"parent","accessMode":"full-access"})).await.unwrap();
        first.send_request_for_workspace("fixture", "thread/sidechat", json!({"threadId":"parent"})).await.unwrap();
        let path = super::super::sidechats::metadata_path(&first, "fixture", "side").unwrap();
        assert_eq!(crate::storage::read_thread_metadata(&path).unwrap()["accessMode"], "read-only");
        let workspaces = Mutex::new(HashMap::from([("fixture".into(), serde_json::from_value(json!({"id":"fixture","name":"Fixture","path":data,"settings":{}})).unwrap())]));
        let sessions = Mutex::new(HashMap::from([("fixture".into(), session())]));
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            set_thread_access_mode_core(&sessions, "fixture".into(), "side".into(), "current".into()).await.unwrap();
            for (requested, expected) in [(Some("current"), "readOnly"), (None, "readOnly"), (Some("ask"), "workspaceWrite"), (Some("current"), "workspaceWrite"), (Some("full-access"), "dangerFullAccess"), (None, "dangerFullAccess")] {
                let response = send_user_message_core(&sessions, &workspaces, "fixture".into(), "side".into(), "hello".into(), None, None, None, requested.map(str::to_owned), None, None).await.unwrap();
                let params = &response["result"]["seen"];
                assert_eq!(params["sandboxPolicy"]["type"], expected);
                assert_eq!(params["approvalPolicy"], if expected == "workspaceWrite" { "on-request" } else { "never" });
                if expected == "workspaceWrite" { assert_eq!(params["sandboxPolicy"]["writableRoots"], json!([data])); }
            }
            for (mode, expected) in [("read-only", "readOnly"), ("ask", "workspaceWrite"), ("full-access", "dangerFullAccess")] {
                set_thread_access_mode_core(&sessions, "fixture".into(), "side".into(), mode.into()).await.unwrap();
                sessions.lock().await.insert("fixture".into(), session());
                let response = send_user_message_core(&sessions, &workspaces, "fixture".into(), "side".into(), "hello".into(), None, None, None, Some("current".into()), None, None).await.unwrap();
                assert_eq!(response["result"]["seen"]["sandboxPolicy"]["type"], expected);
                assert_eq!(crate::storage::read_thread_metadata(&path).unwrap()["accessMode"], mode);
            }
            set_thread_access_mode_core(&sessions, "fixture".into(), "side".into(), "read-only".into()).await.unwrap();
            let response = first.send_request_for_workspace("fixture", "turn/start", json!({"threadId":"side","accessMode":"current","sandboxPolicy":{"type":"dangerFullAccess"}})).await.unwrap();
            assert_eq!(response["result"]["seen"]["sandboxPolicy"]["type"], "readOnly");
            let response = first.send_request_for_workspace("fixture", "turn/start", json!({"threadId":"side"})).await.unwrap();
            assert_eq!(response["result"]["seen"]["sandboxPolicy"]["type"], "readOnly");
            assert!(send_user_message_core(&sessions, &workspaces, "fixture".into(), "side".into(), "hello".into(), None, None, None, Some("invalid".into()), None, None).await.is_err());
        }).await.unwrap();
        let saved = crate::storage::read_thread_metadata(&path).unwrap();
        assert_eq!(saved["accessMode"], "read-only");
        assert_eq!(saved["sideOf"]["threadId"], "parent");
        assert_eq!(crate::storage::read_thread_metadata(&super::super::sidechats::metadata_path(&first, "fixture", "parent").unwrap()).unwrap()["accessMode"], "full-access");
        backend.child.lock().await.kill().await.unwrap();
        server.await.unwrap();
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn native_collab_nodes_map_to_real_child_threads() {
        let response = json!({"result":{"thread":{"turns":[{"items":[
            {"type":"collabAgentToolCall","receiverThreadIds":["child-a","child-b"]},
            {"type":"agentMessage","receiverThreadIds":["not-a-child"]}
        ]}]}}});
        assert_eq!(collab_children(&response), vec!["child-a", "child-b"]);
    }
    use crate::backend::app_server::protocol_tests::make_session;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn key_replacement_preserves_turns_and_restores_threads() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let session = WorkspaceSession::lazy(
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { make_session().codex_backend().await })
            }),
            vec![],
            AgentPolicy::default(),
            None,
            None,
            "workspace".into(),
        );
        let first = session.backend(true).await.unwrap();
        session
            .owners
            .lock()
            .await
            .insert("running".into(), "codex".to_owned());
        first.active_threads.lock().await.insert("running".into());
        session.credentials_changed().await;
        assert!(session.backend(true).await.is_err());
        assert!(Arc::ptr_eq(&first, &session.backend(false).await.unwrap()));
        first.active_threads.lock().await.clear();
        let next = session.backend(true).await.unwrap();
        assert!(!Arc::ptr_eq(&first, &next));
        let stdout = next.child.lock().await.stdout.take().unwrap();
        let responder = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            for method in ["thread/resume", "thread/compact/start"] {
                let line = lines.next_line().await.unwrap().unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["method"], method);
                assert_eq!(request["params"]["threadId"], "running");
                next.pending
                    .lock()
                    .await
                    .remove(&request["id"].as_u64().unwrap())
                    .unwrap()
                    .send(json!({"result":{}}))
                    .unwrap();
            }
        });
        session
            .send_request_for_workspace(
                "workspace",
                "thread/compact/start",
                json!({"threadId":"running"}),
            )
            .await
            .unwrap();
        responder.await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        session.shutdown().await;
    }
    #[tokio::test]
    async fn custom_protocol_routing_and_disabled_history() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let mut settings = AppSettings::default();
        let mut custom = super::super::engine_registry::resolve(&settings, "pi").unwrap().0;
        custom.id = "fork".into(); custom.name = "Fork".into(); custom.built_in = false;
        custom.command = Some("python3".into());
        settings.engines.push(custom.clone());
        std::fs::write(dir.join("settings.json"), serde_json::to_vec(&settings).unwrap()).unwrap();
        let mut session = WorkspaceSession::lazy(Box::new(|| Box::pin(async { panic!("Must not start Codex") })), vec![], AgentPolicy::default(), None, None, "workspace".into());
        session.pi = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
        let error = session.send_request_for_workspace("workspace", "thread/start", json!({"engine":"fork"})).await.unwrap_err();
        assert_eq!(error, "Add and enable a provider and model in Settings");
        assert!(session.extra_pi.lock().await.contains_key("fork"));
        let history = dir.join("fork/sessions/workspace");
        std::fs::create_dir_all(&history).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let header = json!({"type":"session","version":3,"engine":"fork","id":id,"timestamp":"2026-01-01T00:00:00Z","cwd":dir,"modelProvider":"test","model":"stub"});
        std::fs::write(history.join(format!("{id}.jsonl")), format!("{header}\n")).unwrap();
        settings.engines[0].enabled = false;
        std::fs::write(dir.join("settings.json"), serde_json::to_vec(&settings).unwrap()).unwrap();
        let read = session.send_request_for_workspace("workspace", "thread/resume", json!({"threadId":id})).await.unwrap();
        assert_eq!(read["result"]["thread"]["engine"], "fork");
        assert_eq!(read["result"]["thread"]["canAcceptDirectInput"], false);
        assert_eq!(read["result"]["thread"]["engineNotice"], "Enable Fork to continue");
        assert_eq!(session.send_request_for_workspace("workspace", "turn/start", json!({"threadId":id})).await.unwrap_err(), "Enable Fork to continue");
        session.shutdown().await;
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn pi_routes_by_thread_owner_without_starting_codex() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let session = WorkspaceSession::lazy(
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Err("must not start".into()) })
            }),
            vec![],
            AgentPolicy::default(),
            None,
            None,
            "workspace".into(),
        );
        assert_eq!(
            session
                .send_request_for_workspace("workspace", "thread/start", json!({}))
                .await
                .unwrap_err(),
            "Add and enable a provider and model in Settings"
        );
        session
            .owners
            .lock()
            .await
            .insert("pi-thread".into(), "pi".to_owned());
        assert_eq!(
            session
                .send_request_for_workspace(
                    "workspace",
                    "turn/interrupt",
                    json!({"threadId":"pi-thread"})
                )
                .await
                .unwrap_err(),
            "Unknown pi thread"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(session
            .send_request_for_workspace("workspace", "thread/list", json!({}))
            .await
            .unwrap()["result"]["data"]
            .as_array()
            .unwrap()
            .is_empty());
        session.shutdown().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn list_tags_and_sorts_threads_and_records_ownership() {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let session = make_session();
        let stdout = session.child.lock().await.stdout.take().unwrap();
        let backend = session.codex_backend().await.unwrap();
        let responder = tokio::spawn(async move {
            let line = BufReader::new(stdout)
                .lines()
                .next_line()
                .await
                .unwrap()
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["method"], "thread/list");
            backend.pending.lock().await.remove(&request["id"].as_u64().unwrap()).unwrap().send(json!({"result":{"data":[{"id":"old","updatedAt":1},{"id":"new","updatedAt":9}]}})).unwrap();
        });
        let response = session
            .send_request_for_workspace("fixture", "thread/list", json!({}))
            .await
            .unwrap();
        responder.await.unwrap();
        assert_eq!(response["result"]["data"][0]["id"], "new");
        assert_eq!(response["result"]["data"][0]["engine"], "codex");
        assert_eq!(
            session.owners.lock().await.get("new"),
            Some(&"codex".to_owned())
        );
        session.shutdown().await;
    }
}

pub(crate) fn codex_config_args(
    providers: &[ProviderEntry],
    policy: &AgentPolicy,
    home: &Path,
) -> Result<Vec<String>, String> {
    let (config, _) = codex_config(providers, policy, home)?;
    let doc = config
        .parse::<toml_edit::Document>()
        .map_err(|e| e.to_string())?;
    let mut args = vec![];
    for (key, item) in doc.iter() {
        if key == "features" {
            for (feature, value) in item.as_table().ok_or("Invalid feature config")?.iter() {
                args.extend(["-c".into(), format!("features.{feature}={value}")]);
            }
        } else {
            let value = item
                .clone()
                .into_value()
                .map_err(|_| "Invalid generated config")?;
            args.extend(["-c".into(), format!("{key}={value}")]);
        }
    }
    Ok(args)
}

fn write_config_file(path: &Path, content: &str) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = std::fs::write(&temp, content).and_then(|_| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result.map_err(|e| e.to_string())
}

pub(crate) fn feature_policy(mut policy: AgentPolicy, features: &Features) -> AgentPolicy {
    if !features.subagents { policy.subagents.enabled = false; policy.subagents.max_concurrent = 0; }
    policy
}
