use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use crate::backend::events::{AppServerEvent, EventSink};
use crate::codex::args::parse_codex_args;
use crate::shared::process_core::{kill_child_process_tree, tokio_command};
use crate::types::WorkspaceEntry;

#[cfg(target_os = "windows")]
use crate::shared::process_core::{build_cmd_c_command, resolve_windows_executable};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

fn extract_thread_id(value: &Value) -> Option<String> {
    fn extract_from_container(container: Option<&Value>) -> Option<String> {
        let container = container?;
        container
            .get("threadId")
            .or_else(|| container.get("thread_id"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                container
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
    }

    extract_from_container(value.get("params"))
        .or_else(|| extract_from_container(value.get("result")))
}

fn push_thread_id(out: &mut Vec<String>, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };
    if let Some(thread_id) = value.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push(thread_id.to_string());
        return;
    }
    if let Some(values) = value.as_array() {
        for entry in values {
            push_thread_id(out, Some(entry));
        }
    }
}

fn extract_related_thread_ids(value: &Value) -> Vec<String> {
    fn collect_agent_thread_ids(value: Option<&Value>, out: &mut Vec<String>) {
        let Some(value) = value else {
            return;
        };
        if let Some(values) = value.as_array() {
            for entry in values {
                collect_agent_thread_ids(Some(entry), out);
            }
            return;
        }
        let Some(record) = value.as_object() else {
            return;
        };
        push_thread_id(
            out,
            record.get("threadId").or_else(|| record.get("thread_id")),
        );
        push_thread_id(out, record.get("id"));
        push_thread_id(
            out,
            record.get("thread").and_then(|thread| {
                thread
                    .get("id")
                    .or_else(|| thread.get("threadId"))
                    .or_else(|| thread.get("thread_id"))
            }),
        );
    }

    fn collect_from_container(container: Option<&Value>, out: &mut Vec<String>) {
        let Some(container) = container.and_then(|value| value.as_object()) else {
            return;
        };
        push_thread_id(
            out,
            container
                .get("threadId")
                .or_else(|| container.get("thread_id")),
        );
        push_thread_id(out, container.get("agentThreadId"));
        push_thread_id(
            out,
            container.get("thread").and_then(|thread| thread.get("id")),
        );
        push_thread_id(
            out,
            container
                .get("params")
                .and_then(|params| params.get("threadId").or_else(|| params.get("thread_id"))),
        );
        push_thread_id(
            out,
            container
                .get("result")
                .and_then(|result| result.get("threadId").or_else(|| result.get("thread_id"))),
        );
        push_thread_id(
            out,
            container
                .get("newThreadId")
                .or_else(|| container.get("new_thread_id")),
        );
        push_thread_id(
            out,
            container
                .get("receiverThreadId")
                .or_else(|| container.get("receiver_thread_id")),
        );
        push_thread_id(
            out,
            container
                .get("receiverThreadIds")
                .or_else(|| container.get("receiver_thread_ids")),
        );
        collect_agent_thread_ids(
            container
                .get("receiverAgents")
                .or_else(|| container.get("receiver_agents")),
            out,
        );
        collect_agent_thread_ids(
            container
                .get("receiverAgent")
                .or_else(|| container.get("receiver_agent")),
            out,
        );
        collect_agent_thread_ids(
            container
                .get("agentStatuses")
                .or_else(|| container.get("agent_statuses")),
            out,
        );
        if let Some(status_map) = container
            .get("agentsStates")
            .or_else(|| container.get("statuses"))
            .and_then(|value| value.as_object())
        {
            out.extend(
                status_map
                    .keys()
                    .map(|key| key.trim().to_string())
                    .filter(|key| !key.is_empty()),
            );
        }
        if let Some(item) = container.get("item") {
            collect_from_container(Some(item), out);
        }
    }

    let mut out = Vec::new();
    collect_from_container(value.get("params"), &mut out);
    collect_from_container(value.get("result"), &mut out);
    collect_from_container(Some(value), &mut out);

    let mut seen = HashSet::new();
    out.into_iter()
        .filter(|thread_id| seen.insert(thread_id.clone()))
        .collect()
}

fn normalize_root_path(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/');
    if normalized.is_empty() {
        return String::new();
    }
    let lower = normalized.to_ascii_lowercase();
    let normalized = if lower.starts_with("//?/unc/") {
        format!("//{}", &normalized[8..])
    } else if lower.starts_with("//?/") || lower.starts_with("//./") {
        normalized[4..].to_string()
    } else {
        normalized.to_string()
    };
    if normalized.is_empty() {
        return String::new();
    }

    let bytes = normalized.as_bytes();
    let is_drive_path =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/';
    if is_drive_path || normalized.starts_with("//") {
        normalized.to_ascii_lowercase()
    } else {
        normalized.to_string()
    }
}

#[derive(Debug, Clone)]
struct ThreadListEntry {
    thread_id: String,
    cwd: Option<String>,
    is_memory_consolidation: bool,
}

fn extract_thread_entries_from_thread_list_result(value: &Value) -> Vec<ThreadListEntry> {
    fn collect_entries(input: &Value, out: &mut Vec<ThreadListEntry>) {
        if let Some(values) = input.as_array() {
            for value in values {
                collect_entries(value, out);
            }
            return;
        }
        let Some(object) = input.as_object() else {
            return;
        };

        let cwd = object
            .get("cwd")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| {
                object
                    .get("thread")
                    .and_then(|thread| thread.get("cwd"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            });

        let thread_id = object
            .get("threadId")
            .or_else(|| object.get("thread_id"))
            .or_else(|| object.get("id"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| {
                object
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            });
        if let Some(thread_id) = thread_id {
            let source = object
                .get("source")
                .or_else(|| object.get("thread").and_then(|thread| thread.get("source")));
            let is_memory_consolidation = source
                .and_then(source_subagent_kind)
                .is_some_and(|kind| kind == "memory_consolidation");
            out.push(ThreadListEntry {
                thread_id,
                cwd,
                is_memory_consolidation,
            });
        }

        for key in ["threads", "items", "results", "data"] {
            if let Some(values) = object.get(key).and_then(|value| value.as_array()) {
                for value in values {
                    collect_entries(value, out);
                }
            }
        }
    }

    let mut out = Vec::new();
    if let Some(result) = value.get("result") {
        collect_entries(result, &mut out);
    }
    out
}

fn resolve_workspace_for_cwd(
    cwd: &str,
    workspace_roots: &HashMap<String, String>,
) -> Option<String> {
    let normalized_cwd = normalize_root_path(cwd);
    if normalized_cwd.is_empty() {
        return None;
    }
    workspace_roots
        .iter()
        .filter_map(|(workspace_id, root)| {
            if root.is_empty() {
                return None;
            }
            let is_exact_match = root == &normalized_cwd;
            let is_nested_match = normalized_cwd.len() > root.len()
                && normalized_cwd.starts_with(root)
                && normalized_cwd.as_bytes().get(root.len()) == Some(&b'/');
            if is_exact_match || is_nested_match {
                Some((workspace_id, root.len()))
            } else {
                None
            }
        })
        .max_by_key(|(_, root_len)| *root_len)
        .map(|(workspace_id, _)| workspace_id.clone())
}

fn normalize_subagent_kind(value: &str) -> String {
    let mut normalized = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    if let Some(stripped) = normalized.strip_prefix("subagent_") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("sub_agent_") {
        normalized = stripped.to_string();
    }
    normalized
}

fn source_subagent_kind(source: &Value) -> Option<String> {
    if let Some(raw) = source.as_str() {
        let normalized = normalize_subagent_kind(raw);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }
    let source_obj = source.as_object()?;
    let sub_agent = source_obj
        .get("subAgent")
        .or_else(|| source_obj.get("sub_agent"))
        .or_else(|| source_obj.get("subagent"))?;

    if let Some(raw) = sub_agent.as_str() {
        let normalized = normalize_subagent_kind(raw);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }
    let sub_agent_obj = sub_agent.as_object()?;
    if let Some(explicit) = sub_agent_obj
        .get("kind")
        .or_else(|| sub_agent_obj.get("type"))
        .or_else(|| sub_agent_obj.get("name"))
        .or_else(|| sub_agent_obj.get("id"))
        .and_then(Value::as_str)
    {
        let normalized = normalize_subagent_kind(explicit);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }

    let candidate_keys: Vec<&String> = sub_agent_obj
        .keys()
        .filter(|key| key.as_str() != "thread_spawn" && key.as_str() != "threadSpawn")
        .collect();
    if candidate_keys.len() != 1 {
        return None;
    }
    let normalized = normalize_subagent_kind(candidate_keys[0]);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn thread_started_is_memory_consolidation(value: &Value) -> bool {
    value
        .get("params")
        .and_then(|params| {
            params
                .get("thread")
                .and_then(|thread| thread.get("source"))
                .or_else(|| params.get("source"))
        })
        .and_then(source_subagent_kind)
        .is_some_and(|kind| kind == "memory_consolidation")
}

fn should_suppress_hidden_thread_event(
    method_name: Option<&str>,
    has_result_or_error: bool,
) -> bool {
    !has_result_or_error
        && !matches!(
            method_name,
            Some("thread/archived") | Some("codex/backgroundThread")
        )
}

#[derive(Clone)]
pub(crate) struct RequestContext {
    workspace_id: String,
    method: String,
}

fn build_initialize_params(client_version: &str) -> Value {
    json!({
        "clientInfo": {
            "name": "codex_monitor",
            "title": "taucode",
            "version": client_version
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) use crate::shared::engines::WorkspaceSession;

pub(crate) struct CodexEngine {
    pub(crate) events: tokio::sync::broadcast::Sender<Value>,
    pub(crate) active_threads: Mutex<HashSet<String>>,
    pub(crate) codegraph: Option<crate::codegraph::CodegraphSetup>,
    #[cfg(test)]
    pub(crate) codex_args: Option<String>,
    pub(crate) child: Mutex<Child>,
    pub(crate) stdin: Mutex<ChildStdin>,
    pub(crate) pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    pub(crate) request_context: Mutex<HashMap<u64, RequestContext>>,
    pub(crate) thread_workspace: Mutex<HashMap<String, String>>,
    pub(crate) hidden_thread_ids: Mutex<HashSet<String>>,
    pub(crate) next_id: AtomicU64,
    /// Callbacks for background threads - events for these threadIds are sent through the channel
    pub(crate) background_thread_callbacks: Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>,
    pub(crate) owner_workspace_id: String,
    pub(crate) workspace_ids: Mutex<HashSet<String>>,
    pub(crate) workspace_roots: Mutex<HashMap<String, String>>,
}

impl CodexEngine {
    pub(crate) async fn register_workspace(&self, workspace_id: &str) {
        self.register_workspace_with_path(workspace_id, None).await;
    }

    pub(crate) async fn register_workspace_with_path(
        &self,
        workspace_id: &str,
        workspace_path: Option<&str>,
    ) {
        self.workspace_ids
            .lock()
            .await
            .insert(workspace_id.to_string());
        if let Some(path) = workspace_path {
            let normalized = normalize_root_path(path);
            if !normalized.is_empty() {
                self.workspace_roots
                    .lock()
                    .await
                    .insert(workspace_id.to_string(), normalized);
            }
        }
    }

    pub(crate) async fn unregister_workspace(&self, workspace_id: &str) {
        self.workspace_ids.lock().await.remove(workspace_id);
        self.workspace_roots.lock().await.remove(workspace_id);
    }

    async fn write_message(&self, value: Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().await;
        let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())
    }

    pub(crate) async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.send_request_for_workspace(self.owner_workspace_id.as_str(), method, params)
            .await
    }

    pub(crate) async fn send_request_for_workspace(
        &self,
        workspace_id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.register_workspace(workspace_id).await;
        self.pending.lock().await.insert(id, tx);
        self.request_context.lock().await.insert(
            id,
            RequestContext {
                workspace_id: workspace_id.to_string(),
                method: method.to_string(),
            },
        );
        if let Some(thread_id) = extract_thread_id(&json!({ "params": params.clone() })) {
            self.thread_workspace
                .lock()
                .await
                .insert(thread_id, workspace_id.to_string());
        }
        if let Err(error) = self
            .write_message(json!({ "id": id, "method": method, "params": params }))
            .await
        {
            self.pending.lock().await.remove(&id);
            self.request_context.lock().await.remove(&id);
            return Err(error);
        }
        match timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err("request canceled".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                self.request_context.lock().await.remove(&id);
                Err(format!(
                    "request timed out after {} seconds",
                    REQUEST_TIMEOUT.as_secs()
                ))
            }
        }
    }

    pub(crate) async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), String> {
        let value = if let Some(params) = params {
            json!({ "method": method, "params": params })
        } else {
            json!({ "method": method })
        };
        self.write_message(value).await
    }

    pub(crate) async fn send_response(&self, id: Value, result: Value) -> Result<(), String> {
        self.write_message(json!({ "id": id, "result": result }))
            .await
    }
}

pub(crate) fn build_codex_path_env(codex_bin: Option<&str>) -> Option<String> {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();

    let mut extras: Vec<PathBuf> = Vec::new();

    #[cfg(not(target_os = "windows"))]
    {
        extras.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
            ]
            .into_iter()
            .map(PathBuf::from),
        );

        if let Ok(home) = env::var("HOME") {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".local/bin"));
            extras.push(home_path.join(".local/share/mise/shims"));
            extras.push(home_path.join(".cargo/bin"));
            extras.push(home_path.join(".bun/bin"));
            let nvm_root = home_path.join(".nvm/versions/node");
            if let Ok(entries) = std::fs::read_dir(nvm_root) {
                for entry in entries.flatten() {
                    let bin_path = entry.path().join("bin");
                    if bin_path.is_dir() {
                        extras.push(bin_path);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            extras.push(Path::new(&appdata).join("npm"));
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            extras.push(
                Path::new(&local_app_data)
                    .join("Microsoft")
                    .join("WindowsApps"),
            );
        }
        if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".cargo").join("bin"));
            extras.push(home_path.join("scoop").join("shims"));
        }
        if let Ok(program_data) = env::var("PROGRAMDATA") {
            extras.push(Path::new(&program_data).join("chocolatey").join("bin"));
        }
    }

    if let Some(bin_path) = codex_bin.filter(|value| !value.trim().is_empty()) {
        if let Some(parent) = Path::new(bin_path).parent() {
            extras.push(parent.to_path_buf());
        }
    }

    for extra in extras {
        if !paths.iter().any(|path| path == &extra) {
            paths.push(extra);
        }
    }

    if paths.is_empty() {
        return None;
    }

    env::join_paths(paths)
        .ok()
        .map(|joined| joined.to_string_lossy().to_string())
}

pub(crate) fn build_codex_command_with_bin(
    codex_bin: Option<String>,
    codex_args: Option<&str>,
    args: Vec<String>,
) -> Result<Command, String> {
    let bin = codex_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".into());

    let path_env = build_codex_path_env(codex_bin.as_deref());
    let mut command_args = parse_codex_args(codex_args)?;
    command_args.extend(args);

    #[cfg(target_os = "windows")]
    let mut command = {
        let bin_trimmed = bin.trim();
        let resolved = resolve_windows_executable(bin_trimmed, path_env.as_deref());
        let resolved_path = resolved
            .as_deref()
            .unwrap_or_else(|| Path::new(bin_trimmed));
        let ext = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("cmd") | Some("bat")) {
            let mut command = tokio_command("cmd");
            let command_line = build_cmd_c_command(resolved_path, &command_args)?;
            command.arg("/D");
            command.arg("/S");
            command.arg("/C");
            command.raw_arg(command_line);
            command
        } else {
            let mut command = tokio_command(resolved_path);
            command.args(command_args);
            command
        }
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = tokio_command(bin.trim());
        command.args(command_args);
        command
    };

    if let Some(path_env) = path_env {
        command.env("PATH", path_env);
    }
    Ok(command)
}

pub(crate) async fn check_codex_installation(
    codex_bin: Option<String>,
) -> Result<Option<String>, String> {
    let mut command = build_codex_command_with_bin(codex_bin, None, vec!["--version".to_string()])?;
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = match timeout(Duration::from_secs(5), command.output()).await {
        Ok(result) => result.map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "Codex CLI not found. Install Codex and ensure `codex` is on your PATH.".to_string()
            } else {
                e.to_string()
            }
        })?,
        Err(_) => {
            return Err(
                "Timed out while checking Codex CLI. Make sure `codex --version` runs in Terminal."
                    .to_string(),
            );
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        if detail.is_empty() {
            return Err(
                "Codex CLI failed to start. Try running `codex --version` in Terminal.".to_string(),
            );
        }
        return Err(format!(
            "Codex CLI failed to start: {detail}. Try running `codex --version` in Terminal."
        ));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if version.is_empty() {
        None
    } else {
        Some(version)
    })
}

pub(crate) async fn route_app_server_message<E: EventSink>(
    session: &CodexEngine,
    fallback_workspace_id: &str,
    event_sink: &E,
    value: Value,
) {
    if value
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|m| m.starts_with("account/") || m.starts_with("app/"))
    {
        return;
    }
    if let Some(id) = extract_thread_id(&value) {
        match value.get("method").and_then(Value::as_str) {
            Some("turn/started") => {
                session.active_threads.lock().await.insert(id);
            }
            Some("turn/completed") => {
                session.active_threads.lock().await.remove(&id);
            }
            _ => (),
        }
    }
    let _ = session.events.send(value.clone());
    if let Some(setup) = &session.codegraph {
        setup.on_engine_message(&value);
    }
    let maybe_id = value.get("id").and_then(|id| id.as_u64());
    let has_method = value.get("method").is_some();
    let has_result_or_error = value.get("result").is_some() || value.get("error").is_some();
    let method_name = value.get("method").and_then(|method| method.as_str());

    // Check if this event is for a background thread
    let thread_id = extract_thread_id(&value);
    let mut request_workspace: Option<String> = None;
    let mut request_method: Option<String> = None;
    if let Some(id) = maybe_id {
        if has_result_or_error {
            if let Some(context) = session.request_context.lock().await.remove(&id) {
                request_workspace = Some(context.workspace_id);
                request_method = Some(context.method);
            }
        }
    }

    if let Some(ref workspace_id) = request_workspace {
        let related_thread_ids = extract_related_thread_ids(&value);
        if !related_thread_ids.is_empty() {
            let mut thread_workspace = session.thread_workspace.lock().await;
            for tid in related_thread_ids {
                thread_workspace.insert(tid, workspace_id.clone());
            }
        } else if let Some(ref tid) = thread_id {
            session
                .thread_workspace
                .lock()
                .await
                .insert(tid.clone(), workspace_id.clone());
        }
    }
    if matches!(request_method.as_deref(), Some("thread/list")) {
        let thread_entries = extract_thread_entries_from_thread_list_result(&value);
        if !thread_entries.is_empty() {
            let workspace_roots = session.workspace_roots.lock().await.clone();
            let mut hidden_thread_ids = Vec::new();
            let mut thread_workspace = session.thread_workspace.lock().await;
            for entry in thread_entries {
                if entry.is_memory_consolidation {
                    thread_workspace.remove(&entry.thread_id);
                    hidden_thread_ids.push(entry.thread_id);
                    continue;
                }
                let mapped_workspace = entry
                    .cwd
                    .as_deref()
                    .and_then(|cwd| resolve_workspace_for_cwd(cwd, &workspace_roots));
                if let Some(workspace_id) = mapped_workspace {
                    thread_workspace.insert(entry.thread_id, workspace_id);
                }
            }
            drop(thread_workspace);
            if !hidden_thread_ids.is_empty() {
                let mut hidden = session.hidden_thread_ids.lock().await;
                for thread_id in hidden_thread_ids {
                    hidden.insert(thread_id);
                }
            }
        }
    }

    let mapped_thread_workspace = if let Some(ref tid) = thread_id {
        session.thread_workspace.lock().await.get(tid).cloned()
    } else {
        None
    };

    let routed_workspace_id = mapped_thread_workspace
        .or_else(|| request_workspace.clone())
        .unwrap_or_else(|| fallback_workspace_id.to_string());

    if let Some(ref tid) = thread_id {
        if method_name == Some("codex/backgroundThread") {
            let action = value
                .get("params")
                .and_then(|params| params.get("action"))
                .and_then(Value::as_str)
                .unwrap_or("hide");
            if action.eq_ignore_ascii_case("hide") {
                session.hidden_thread_ids.lock().await.insert(tid.clone());
            }
        } else if method_name == Some("thread/started")
            && thread_started_is_memory_consolidation(&value)
        {
            session.hidden_thread_ids.lock().await.insert(tid.clone());
            let payload = AppServerEvent {
                workspace_id: routed_workspace_id.clone(),
                message: json!({
                    "method": "codex/backgroundThread",
                    "params": {
                        "threadId": tid,
                        "action": "hide"
                    }
                }),
            };
            event_sink.emit_app_server_event(payload);
            return;
        }

        let should_suppress_hidden_thread = {
            let hidden = session.hidden_thread_ids.lock().await;
            hidden.contains(tid)
        };
        if should_suppress_hidden_thread
            && should_suppress_hidden_thread_event(method_name, has_result_or_error)
        {
            return;
        }
    }

    if matches!(method_name, Some("item/started") | Some("item/completed")) {
        let related_thread_ids = extract_related_thread_ids(&value);
        if !related_thread_ids.is_empty() {
            let mut thread_workspace = session.thread_workspace.lock().await;
            for related_id in related_thread_ids {
                thread_workspace
                    .entry(related_id)
                    .or_insert_with(|| routed_workspace_id.clone());
            }
        }
    }

    if method_name == Some("thread/archived") {
        if let Some(ref tid) = thread_id {
            session.thread_workspace.lock().await.remove(tid);
            session.hidden_thread_ids.lock().await.remove(tid);
        }
    }

    if let Some(id) = maybe_id {
        if has_result_or_error {
            if let Some(tx) = session.pending.lock().await.remove(&id) {
                let _ = tx.send(value);
            }
        } else if has_method {
            // Check for background thread callback
            let mut sent_to_background = false;
            if let Some(ref tid) = thread_id {
                let callbacks = session.background_thread_callbacks.lock().await;
                if let Some(tx) = callbacks.get(tid) {
                    let _ = tx.send(value.clone());
                    sent_to_background = true;
                }
            }
            // Don't emit to frontend if this is a background thread event
            if !sent_to_background {
                let payload = AppServerEvent {
                    workspace_id: routed_workspace_id.clone(),
                    message: value,
                };
                event_sink.emit_app_server_event(payload);
            }
        } else if let Some(tx) = session.pending.lock().await.remove(&id) {
            let _ = tx.send(value);
        }
    } else if has_method {
        // Check for background thread callback
        let mut sent_to_background = false;
        if let Some(ref tid) = thread_id {
            let callbacks = session.background_thread_callbacks.lock().await;
            if let Some(tx) = callbacks.get(tid) {
                let _ = tx.send(value.clone());
                sent_to_background = true;
            }
        }
        // Don't emit to frontend if this is a background thread event
        if !sent_to_background {
            let payload = AppServerEvent {
                workspace_id: routed_workspace_id,
                message: value,
            };
            event_sink.emit_app_server_event(payload);
        }
    }
}

pub(crate) async fn spawn_workspace_session<E: EventSink>(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    codex_args: Option<String>,
    codex_home: Option<PathBuf>,
    client_version: String,
    event_sink: E,
    codegraph: Option<crate::codegraph::CodegraphSetup>,
    settings: crate::types::AppSettings,
) -> Result<Arc<WorkspaceSession>, String> {
    let policy = crate::shared::providers::effective_policy(
        &settings.agent_policy,
        entry.settings.agent_policy.as_ref(),
    );
    let policy = crate::shared::engines::feature_policy(policy, &settings.features());
    crate::shared::providers::validate(&settings.providers, &policy)?;
    let workspace_id = entry.id.clone();
    let providers = settings.providers.clone();
    let args = codex_args.clone();
    let graph = codegraph.clone();
    let pi_cwd = entry.path.clone();
    let pi_home = codex_home
        .clone()
        .or_else(crate::codex::home::resolve_default_codex_home);
    let pi_data = pi_home
        .as_deref()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or("Missing taucode app data directory for pi")?;
    let emit_sink = event_sink.clone();
    let checkpoints = crate::shared::checkpoints::Checkpoints::new(pi_data.clone(), workspace_id.clone(), pi_cwd.clone(), Arc::new(move |event| emit_sink.emit_app_server_event(event)));
    let event_sink = crate::shared::checkpoints::CheckpointSink { inner: event_sink, checkpoints: checkpoints.clone() };
    let sink = event_sink.clone();
    let factory = Box::new(move || {
        let (mut entry, bin, args, home, version, sink, graph, mut settings) = (
            entry.clone(),
            default_codex_bin.clone(),
            codex_args.clone(),
            codex_home.clone(),
            client_version.clone(),
            event_sink.clone(),
            codegraph.clone(),
            settings.clone(),
        );
        if let Some(data) = home
            .as_deref()
            .and_then(Path::parent)
            .and_then(Path::parent)
        {
            if let Ok(latest) = crate::storage::read_settings(&data.join("settings.json")) {
                settings = latest;
            }
            if let Ok(workspaces) = crate::storage::read_workspaces(&data.join("workspaces.json")) {
                if let Some(latest) = workspaces.get(&entry.id) {
                    entry.settings = latest.settings.clone();
                }
            }
        }
        Box::pin(spawn_codex_engine(
            entry, bin, args, home, version, sink, graph, settings,
        )) as crate::shared::providers::BoxFuture<'static, Result<Arc<CodexEngine>, String>>
    });
    let mut session = WorkspaceSession::lazy(
        factory,
        providers,
        policy,
        args,
        graph,
        workspace_id.clone(),
    );
    session.checkpoints = Some(checkpoints);
    let pi_sink = sink.clone();
    let claude_sink = sink.clone();
    session.claude = crate::shared::acp::ClaudeAcpEngine::new(
        pi_data.clone(), workspace_id.clone(), pi_cwd.clone(), session.codegraph.clone(),
        Arc::new(move |workspace, message| {
            claude_sink.emit_app_server_event(AppServerEvent { workspace_id: workspace.into(), message })
        }),
    );
    session.pi = crate::shared::pi::PiEngine::new(
        pi_data,
        workspace_id.clone(),
        pi_cwd,
        session.codegraph.clone(),
        Arc::new(move |workspace, message| {
            pi_sink.emit_app_server_event(AppServerEvent {
                workspace_id: workspace.into(),
                message,
            })
        }),
    );
    sink.emit_app_server_event(AppServerEvent {
        workspace_id: workspace_id.clone(),
        message: json!({"method":"codex/connected","params":{"workspaceId":workspace_id}}),
    });
    Ok(Arc::new(session))
}

async fn spawn_codex_engine<E: EventSink>(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    codex_args: Option<String>,
    codex_home: Option<PathBuf>,
    client_version: String,
    event_sink: E,
    codegraph: Option<crate::codegraph::CodegraphSetup>,
    settings: crate::types::AppSettings,
) -> Result<Arc<CodexEngine>, String> {
    if !settings.providers.iter().any(|p| {
        p.enabled && crate::shared::providers::codex_usable(p) && p.models.iter().any(|m| m.enabled)
    }) {
        return Err("Add and enable a codex-compatible provider and model in Settings".into());
    }
    let policy = crate::shared::providers::effective_policy(
        &settings.agent_policy,
        entry.settings.agent_policy.as_ref(),
    );
    let policy = crate::shared::engines::feature_policy(policy, &settings.features());
    let home = codex_home.ok_or("Managed CODEX_HOME unavailable")?;
    for arg in parse_codex_args(codex_args.as_deref())? {
        if [
            "forced_login_method",
            "chatgpt_base_url",
            "credentials_store",
            "model_provider",
            "mcp_servers",
        ]
        .iter()
        .any(|key| arg.contains(key))
            || arg == "--model"
            || arg == "-m"
            || arg.starts_with("--model=")
            || arg.contains("model=")
        {
            return Err("Configure providers and models in taucode Settings; account and provider overrides in Codex arguments are disabled".into());
        }
    }
    let mut provider_env = crate::shared::engines::prepare_codex(
        &home,
        &settings.providers,
        &policy,
        &crate::shared::providers::Keychain,
    )?;
    for p in settings.providers.iter().filter(|p| p.enabled && crate::shared::providers::codex_usable(p)) {
        if matches!(p.auth, crate::types::ProviderAuth::Command { .. }) {
            if let Some(key) = crate::shared::providers::token(p, &crate::shared::providers::Keychain).await? {
                provider_env.push((crate::shared::engines::key_env(&p.id), key));
            }
        }
    }
    let codex_bin = default_codex_bin;
    let _ = check_codex_installation(codex_bin.clone()).await?;

    let mut command = build_codex_command_with_bin(
        codex_bin,
        codex_args.as_deref(),
        vec!["app-server".to_string()],
    )?;
    command.args(crate::shared::engines::codex_config_args(
        &settings.providers,
        &policy,
        &home,
    )?);
    let mut connectors =
        crate::shared::connectors::effective(&settings, &entry.settings, codegraph.as_ref())?;
    crate::shared::connectors::configure_browser(
        &mut connectors,
        home.parent()
            .and_then(Path::parent)
            .ok_or("Missing browser app data directory")?,
    )?;
    let delivery =
        crate::shared::connectors::delivery(&connectors, &crate::shared::providers::Keychain)?;
    command.args(delivery.codex);
    command.current_dir(&entry.path);
    let mut secrets: Vec<String> = provider_env
        .iter()
        .map(|(_, value)| value.clone())
        .collect();
    secrets.extend(delivery.secrets);
    provider_env.extend(delivery.env);
    command
        .env_clear()
        .envs(crate::shared::engine_registry::environment(std::env::vars(), &home))
        .env("CODEX_HOME", &home)
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_APP_SERVER_TEST_USER_CONFIG_FILE")
        .envs(provider_env);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let diagnostic_start=taucode_diag_mcp::writer::enabled().then(std::time::Instant::now);
    let mut child = command.spawn().map_err(|e| {taucode_diag_mcp::writer::record(||taucode_diag_mcp::core::event("error","engine","engine.spawn_failed","Codex spawn failed"));e.to_string()})?;
    taucode_diag_mcp::observe::spawned("codex",diagnostic_start);
    let diagnostic_pid=child.id();
    taucode_diag_mcp::observe::child(diagnostic_pid,"codex");
    let stdin = child.stdin.take().ok_or("missing stdin")?;
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;

    let session = Arc::new(CodexEngine {
        events: tokio::sync::broadcast::channel(256).0,
        active_threads: Mutex::new(HashSet::new()),
        #[cfg(test)]
        codex_args,
        codegraph,
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        request_context: Mutex::new(HashMap::new()),
        thread_workspace: Mutex::new(HashMap::new()),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: entry.id.clone(),
        workspace_ids: Mutex::new(HashSet::from([entry.id.clone()])),
        workspace_roots: Mutex::new(HashMap::from([(
            entry.id.clone(),
            normalize_root_path(&entry.path),
        )])),
    });

    let session_clone = Arc::clone(&session);
    let fallback_workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    let output_secrets = secrets.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = redact_credentials(line, &output_secrets);
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    taucode_diag_mcp::writer::record(||taucode_diag_mcp::core::event("error","engine","engine.protocol_decode","Codex protocol decode failed"));
                    let payload = AppServerEvent {
                        workspace_id: fallback_workspace_id.clone(),
                        message: json!({
                            "method": "codex/parseError",
                            "params": { "error": err.to_string(), "raw": line },
                        }),
                    };
                    event_sink_clone.emit_app_server_event(payload);
                    continue;
                }
            };

            route_app_server_message(
                &session_clone,
                &fallback_workspace_id,
                &event_sink_clone,
                value,
            )
            .await;
        }

        for thread in session_clone.active_threads.lock().await.drain() {
            let workspace = session_clone.thread_workspace.lock().await.get(&thread).cloned().unwrap_or_else(|| fallback_workspace_id.clone());
            event_sink_clone.emit_app_server_event(AppServerEvent { workspace_id: workspace, message: json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"status":"failed","error":{"message":"Engine process exited"}}}}) });
        }
        // Ensure pending foreground requests cannot accumulate after process output ends.
        if taucode_diag_mcp::writer::enabled(){if let Ok(Some(status))=session_clone.child.lock().await.try_wait(){taucode_diag_mcp::observe::exited(diagnostic_pid,status);}}
        session_clone.pending.lock().await.clear();
        session_clone.request_context.lock().await.clear();
    });

    let workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            taucode_diag_mcp::observe::stderr(diagnostic_pid,&line);
            let line = redact_credentials(line, &secrets);
            if line.trim().is_empty() {
                continue;
            }
            let payload = AppServerEvent {
                workspace_id: workspace_id.clone(),
                message: json!({
                    "method": "codex/stderr",
                    "params": { "message": line },
                }),
            };
            event_sink_clone.emit_app_server_event(payload);
        }
    });

    let init_params = build_initialize_params(&client_version);
    let init_result = timeout(
        Duration::from_secs(15),
        session.send_request("initialize", init_params),
    )
    .await;
    let init_response = match init_result {
        Ok(response) => response,
        Err(_) => {
            let mut child = session.child.lock().await;
            kill_child_process_tree(&mut child).await;
            return Err(
                "Codex app-server did not respond to initialize. Check that `codex app-server` works in Terminal."
                    .to_string(),
            );
        }
    };
    init_response?;
    session.send_notification("initialized", None).await?;

    Ok(session)
}

fn redact_credentials(mut text: String, secrets: &[String]) -> String {
    for secret in secrets.iter().filter(|value| !value.is_empty()) {
        let escaped = serde_json::to_string(secret).expect("string serialization");
        text = text.replace(&escaped[1..escaped.len() - 1], "[REDACTED]");
        text = text.replace(secret, "[REDACTED]");
    }
    text
}

#[cfg(test)]
#[path = "app_server_protocol_tests.rs"]
pub(crate) mod protocol_tests;

#[cfg(test)]
mod tests {
    use super::{
        build_initialize_params, extract_related_thread_ids,
        extract_thread_entries_from_thread_list_result, extract_thread_id, normalize_root_path,
        resolve_workspace_for_cwd, should_suppress_hidden_thread_event, source_subagent_kind,
        thread_started_is_memory_consolidation,
    };
    use serde_json::json;
    #[test]
    fn credentials_are_redacted_from_engine_output() {
        let secret = "private-\"token".to_string();
        let raw = format!("error: {secret}");
        assert_eq!(
            super::redact_credentials(raw, &[secret.clone()]),
            "error: [REDACTED]"
        );
        let message = json!({"error": secret}).to_string();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&super::redact_credentials(
                message,
                &[secret]
            ))
            .unwrap(),
            json!({"error": "[REDACTED]"})
        );
    }
    use std::collections::HashMap;

    #[test]
    fn extract_thread_id_reads_camel_case() {
        let value = json!({ "params": { "threadId": "thread-123" } });
        assert_eq!(extract_thread_id(&value), Some("thread-123".to_string()));
    }

    #[test]
    fn extract_thread_id_reads_snake_case() {
        let value = json!({ "params": { "thread_id": "thread-456" } });
        assert_eq!(extract_thread_id(&value), Some("thread-456".to_string()));
    }

    #[test]
    fn extract_thread_id_reads_hook_notification_thread_id() {
        let value = json!({
            "method": "hook/started",
            "params": {
                "threadId": "thread-hook-1",
                "run": { "id": "hook-1" }
            }
        });
        assert_eq!(extract_thread_id(&value), Some("thread-hook-1".to_string()));
    }

    #[test]
    fn extract_thread_id_returns_none_when_missing() {
        let value = json!({ "params": {} });
        assert_eq!(extract_thread_id(&value), None);
    }

    #[test]
    fn build_initialize_params_enables_experimental_api() {
        let params = build_initialize_params("1.2.3");
        assert_eq!(
            params
                .get("capabilities")
                .and_then(|caps| caps.get("experimentalApi"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn extract_thread_entries_reads_result_data_items() {
        let value = json!({
            "result": {
                "data": [
                    { "id": "thread-a", "cwd": "/tmp/a" },
                    {
                        "threadId": "thread-b",
                        "cwd": "/tmp/b",
                        "source": { "subAgent": "memory_consolidation" }
                    }
                ]
            }
        });
        let entries = extract_thread_entries_from_thread_list_result(&value);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].thread_id, "thread-a");
        assert_eq!(entries[0].cwd.as_deref(), Some("/tmp/a"));
        assert!(!entries[0].is_memory_consolidation);
        assert_eq!(entries[1].thread_id, "thread-b");
        assert_eq!(entries[1].cwd.as_deref(), Some("/tmp/b"));
        assert!(entries[1].is_memory_consolidation);
    }

    #[test]
    fn extract_related_thread_ids_reads_spawn_hints_from_item_payloads() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "mcpToolCall",
                    "new_thread_id": "thread-child"
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child".to_string()));
    }

    #[test]
    fn extract_related_thread_ids_reads_receiver_agent_references() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "collabToolCall",
                    "receiver_agents": [
                        { "thread_id": "thread-child-a" },
                        { "thread": { "id": "thread-child-b" } }
                    ],
                    "statuses": {
                        "thread-child-c": { "status": "running" }
                    }
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child-a".to_string()));
        assert!(ids.contains(&"thread-child-b".to_string()));
        assert!(ids.contains(&"thread-child-c".to_string()));
    }

    #[test]
    fn extract_related_thread_ids_reads_singular_receiver_agent_reference() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "mcpToolCall",
                    "receiver_agent": { "thread_id": "thread-child-single" }
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child-single".to_string()));
    }

    #[test]
    fn resolve_workspace_for_cwd_normalizes_windows_paths() {
        let mut roots = HashMap::new();
        roots.insert("ws-1".to_string(), normalize_root_path("C:\\Dev\\Codex"));
        assert_eq!(
            resolve_workspace_for_cwd("c:/dev/codex", &roots),
            Some("ws-1".to_string())
        );
    }

    #[test]
    fn resolve_workspace_for_cwd_normalizes_windows_namespace_paths() {
        let mut roots = HashMap::new();
        roots.insert("ws-1".to_string(), normalize_root_path("C:\\Dev\\Codex"));
        assert_eq!(
            resolve_workspace_for_cwd("\\\\?\\C:\\Dev\\Codex", &roots),
            Some("ws-1".to_string())
        );
    }

    #[test]
    fn normalize_root_path_normalizes_windows_namespace_unc_paths() {
        assert_eq!(
            normalize_root_path("\\\\?\\UNC\\SERVER\\Share\\Repo\\"),
            "//server/share/repo"
        );
    }

    #[test]
    fn resolve_workspace_for_cwd_matches_nested_paths() {
        let mut roots = HashMap::new();
        roots.insert("ws-1".to_string(), normalize_root_path("/tmp/codex"));
        assert_eq!(
            resolve_workspace_for_cwd("/tmp/codex/subdir/project", &roots),
            Some("ws-1".to_string())
        );
    }

    #[test]
    fn resolve_workspace_for_cwd_prefers_longest_matching_root() {
        let mut roots = HashMap::new();
        roots.insert("ws-parent".to_string(), normalize_root_path("/tmp/codex"));
        roots.insert(
            "ws-child".to_string(),
            normalize_root_path("/tmp/codex/subdir"),
        );
        assert_eq!(
            resolve_workspace_for_cwd("/tmp/codex/subdir/project", &roots),
            Some("ws-child".to_string())
        );
    }

    #[test]
    fn source_subagent_kind_reads_string_variants() {
        assert_eq!(
            source_subagent_kind(&json!("subagent-memory-consolidation")),
            Some("memory_consolidation".to_string())
        );
        assert_eq!(
            source_subagent_kind(&json!("sub_agent_memory_consolidation")),
            Some("memory_consolidation".to_string())
        );
    }

    #[test]
    fn source_subagent_kind_reads_nested_subagent_object_keys() {
        let source = json!({
            "subAgent": {
                "memory_consolidation": {
                    "thread_spawn": { "parent_thread_id": "thread-parent" }
                }
            }
        });
        assert_eq!(
            source_subagent_kind(&source),
            Some("memory_consolidation".to_string())
        );
    }

    #[test]
    fn thread_started_memory_consolidation_detects_thread_source() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "source": {
                        "subagent": "memory_consolidation"
                    }
                }
            }
        });
        assert!(thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn thread_started_memory_consolidation_detects_params_source_fallback() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "threadId": "thread-1",
                "source": {
                    "subAgent": "memory_consolidation"
                }
            }
        });
        assert!(thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn thread_started_memory_consolidation_rejects_non_memory_subagent() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "source": {
                        "subAgent": "review"
                    }
                }
            }
        });
        assert!(!thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn hidden_thread_suppression_allows_rpc_responses() {
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/archived"),
            true
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/updated"),
            true
        ));
        assert!(!should_suppress_hidden_thread_event(None, true));
    }

    #[test]
    fn hidden_thread_suppression_still_blocks_non_exempt_notifications() {
        assert!(should_suppress_hidden_thread_event(
            Some("thread/updated"),
            false
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/archived"),
            false
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("codex/backgroundThread"),
            false
        ));
    }
}
