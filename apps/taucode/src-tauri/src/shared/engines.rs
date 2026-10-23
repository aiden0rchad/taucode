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
pub(crate) struct PiEngine {
    events: broadcast::Sender<Value>,
}
impl Default for PiEngine {
    fn default() -> Self {
        Self {
            events: broadcast::channel(256).0,
        }
    }
}
impl EngineBackend for PiEngine {
    fn request<'a>(
        &'a self,
        _: &'a str,
        method: &'a str,
        _: Value,
    ) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            if method == "thread/list" {
                Ok(json!({"result":{"data":[],"nextCursor":null}}))
            } else {
                Err("pi engine not yet available".into())
            }
        })
    }
    fn respond_to_server_request(&self, _: Value, _: Value) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async { Err("pi engine not yet available".into()) })
    }
    fn events(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }
    fn shutdown(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
}
pub(crate) struct WorkspaceSession {
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
            pi: PiEngine::default(),
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
            codegraph,
            codex_args,
            owner_workspace_id,
            pi: PiEngine::default(),
            owners: Mutex::new(HashMap::new()),
            thread_providers: Mutex::new(HashMap::new()),
            providers,
            policy,
        }
    }
    fn current_configuration(&self) -> (Vec<ProviderEntry>, AgentPolicy) {
        if let Some(home) = crate::codex::home::resolve_default_codex_home() {
            if let Some(data) = home.parent().and_then(Path::parent) {
                if let Ok(settings) = crate::storage::read_settings(&data.join("settings.json")) {
                    let workspaces = crate::storage::read_workspaces(&data.join("workspaces.json"))
                        .unwrap_or_default();
                    let over = workspaces
                        .get(&self.owner_workspace_id)
                        .and_then(|w| w.settings.agent_policy.as_ref());
                    return (
                        settings.providers,
                        providers::effective_policy(&settings.agent_policy, over),
                    );
                }
            }
        }
        (self.providers.clone(), self.policy.clone())
    }
    pub async fn codex_backend(&self) -> Result<Arc<CodexEngine>, String> {
        self.backend(false).await
    }
    pub async fn credentials_changed(&self) {
        // Reload child environment on the next request, after active turns finish.
        *self.configuration.lock().await = Some(String::new());
    }
    async fn backend(&self, refresh: bool) -> Result<Arc<CodexEngine>, String> {
        let mut backend = self.codex.lock().await;
        let mut fingerprint = self.configuration.lock().await;
        let current =
            serde_json::to_string(&self.current_configuration()).map_err(|e| e.to_string())?;
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
                .filter(|(_, engine)| **engine == EngineKind::Codex)
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
        let _ = self.pi.shutdown().await;
    }
    pub async fn register_workspace_with_path(&self, id: &str, path: Option<&str>) {
        if let Some(c) = self.codex.lock().await.as_ref() {
            c.register_workspace_with_path(id, path).await;
        }
    }
    pub async fn unregister_workspace(&self, id: &str) {
        if let Some(c) = self.codex.lock().await.as_ref() {
            c.unregister_workspace(id).await;
        }
    }
    pub async fn send_request_for_workspace(
        &self,
        workspace: &str,
        method: &str,
        mut params: Value,
    ) -> Result<Value, String> {
        let (providers, policy) = self.current_configuration();
        if method == "model/list" {
            return Ok(providers::catalog(&providers));
        }
        let thread_id = params
            .get("threadId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let owners = self.owners.lock().await;
        let engine = if method == "thread/start" {
            params
                .get("engine")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| "Unknown engine")?
                .unwrap_or(policy.default_engine)
        } else {
            thread_id
                .as_ref()
                .and_then(|id| owners.get(id).copied())
                .unwrap_or(EngineKind::Codex)
        };
        drop(owners);
        if let Some(obj) = params.as_object_mut() {
            obj.remove("engine");
        }
        if engine == EngineKind::Pi {
            return self.pi.request(workspace, method, params).await;
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
            return self.pi.request(workspace, method, params).await;
        }
        let backend = self
            .backend(!matches!(method, "turn/interrupt" | "turn/steer"))
            .await?;
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
                    .insert(id.into(), EngineKind::Codex);
            }
        }
        if method == "thread/list" {
            let pi = self.pi.request(workspace, method, json!({})).await?;
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
                            .insert(id.into(), EngineKind::Codex);
                    }
                }
                data.extend(
                    pi.pointer("/result/data")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                );
                data.sort_by_key(|t| {
                    std::cmp::Reverse(t.get("updatedAt").and_then(Value::as_i64).unwrap_or(0))
                });
            }
        }
        Ok(response)
    }
    pub async fn send_response(&self, id: Value, result: Value) -> Result<(), String> {
        self.codex_backend()
            .await?
            .respond_to_server_request(id, result)
            .await
    }
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
    let mut config = String::from("cli_auth_credentials_store = \"ephemeral\"\ncheck_for_update_on_startup = false\n[analytics]\nenabled = false\n[feedback]\nenabled = false\n[apps._default]\nenabled = false\n[plugins._default]\nenabled = false\n[features]\napps = false\nplugins = false\n");
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
            ProviderAuth::Command { command, args } => config.push_str(&format!("auth = {{ command = {}, args = [{}], timeout_ms = 5000, refresh_interval_ms = 300000 }}\n", quote(command), args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(", "))),
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
            .insert("running".into(), EngineKind::Codex);
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
            "pi engine not yet available"
        );
        session
            .owners
            .lock()
            .await
            .insert("pi-thread".into(), EngineKind::Pi);
        assert_eq!(
            session
                .send_request_for_workspace(
                    "workspace",
                    "turn/interrupt",
                    json!({"threadId":"pi-thread"})
                )
                .await
                .unwrap_err(),
            "pi engine not yet available"
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
            Some(&EngineKind::Codex)
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
