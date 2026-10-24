mod repository;
#[cfg(test)]
mod tests;
use crate::backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput};
use repository::Repository;
pub(crate) fn validate_space(data: &std::path::Path, workspace: &str, root: &std::path::Path) -> Result<(), String> {
    Repository::new(data, workspace, root).map(|_| ())
}
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Active {
    metadata: Value,
    checkpoint: Option<String>,
    user_bound: bool,
}
#[derive(Clone)]
struct Undo {
    checkpoint: String,
    paths: Vec<String>,
    leaf: Option<Value>,
    engine: String,
}
#[derive(Default)]
struct State {
    rewinding: bool,
    active: HashMap<(String, String), Active>,
    undo: HashMap<(String, String), Undo>,
}
struct RewindGuard<'a>(&'a Mutex<State>);
impl Drop for RewindGuard<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap().rewinding = false;
    }
}
pub struct Checkpoints {
    data: PathBuf,
    paths: Mutex<HashMap<String, String>>,
    state: Mutex<State>,
    operation: tokio::sync::Mutex<()>,
    emit: Arc<dyn Fn(AppServerEvent) + Send + Sync>,
}
impl Checkpoints {
    pub async fn outputs(&self, workspace: &str, thread: &str) -> Result<Vec<String>, String> {
        let _operation = self.operation.lock().await;
        let repo = self.repo(workspace)?;
        let thread = thread.to_owned();
        tokio::task::spawn_blocking(move || {
            let rows = repo.list(&thread)?;
            let before = rows.iter().rev().find(|r| r.metadata["kind"] == "before" && r.commit.is_some()).ok_or("No before checkpoint available")?;
            Ok(repo.diff(&thread, &before.id)?.into_iter().filter(|c| c.status != "deleted since").map(|c| c.path).collect())
        }).await.map_err(|e| e.to_string())?
    }
    pub fn new(
        data: PathBuf,
        workspace: String,
        path: String,
        emit: Arc<dyn Fn(AppServerEvent) + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            data,
            paths: Mutex::new(HashMap::from([(workspace, path)])),
            state: Mutex::new(State::default()),
            operation: tokio::sync::Mutex::new(()),
            emit,
        })
    }
    pub fn register(&self, id: &str, path: &str) {
        self.paths.lock().unwrap().insert(id.into(), path.into());
    }
    fn repo(&self, workspace: &str) -> Result<Repository, String> {
        let paths = self.paths.lock().unwrap();
        Repository::new(
            &self.data,
            workspace,
            Path::new(paths.get(workspace).ok_or("Unknown workspace")?),
        )
    }
    fn notify(&self, workspace: &str, thread: &str, warning: Option<&str>) {
        (self.emit)(AppServerEvent {
            workspace_id: workspace.into(),
            message: json!({"method":"checkpoint/updated","params":{"threadId":thread,"warning":warning}}),
        });
    }
    pub async fn before(
        self: &Arc<Self>,
        workspace: &str,
        thread: &str,
        engine: &str,
        params: &Value,
    ) -> Result<(), String> {
        let key = (workspace.to_owned(), thread.to_owned());
        let checkpoint_id = uuid::Uuid::new_v4().to_string();
        let metadata = json!({"checkpointId":checkpoint_id,"threadId":thread,"turnId":uuid::Uuid::new_v4().to_string(),"userItemId":uuid::Uuid::new_v4().to_string(),"kind":"before","timestamp":chrono::Utc::now().to_rfc3339(),"engine":engine,"preview":params["input"].as_array().map(|a| a.iter().filter_map(|v| v["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default()});
        {
            let mut state = self.state.lock().unwrap();
            if state.rewinding {
                return Err("Wait for rewind to finish before starting a turn".into());
            }
            if state.active.contains_key(&key) {
                return Err("Thread is running".into());
            }
            state.undo.remove(&key);
            state.active.insert(
                key.clone(),
                Active {
                    metadata: metadata.clone(),
                    checkpoint: Some(checkpoint_id),
                    user_bound: false,
                },
            );
        }
        (self.emit)(AppServerEvent {
            workspace_id: workspace.into(),
            message: json!({"method":"checkpoint/turnStarting","params":{"threadId":thread}}),
        });
        let this = self.clone();
        let started = Instant::now();
        let mut task = tokio::spawn(async move {
            let _operation = this.operation.lock().await;
            let repo = this.repo(&key.0)?;
            let copy = repo.clone();
            let row = tokio::task::spawn_blocking(move || copy.snapshot(metadata))
                .await
                .map_err(|e| e.to_string())??;
            if started.elapsed() >= Duration::from_secs(2) {
                repo.unavailable(&key.1, &row.id)?;
                return Err("checkpoint unavailable: snapshot exceeded 2 s".into());
            }
            if let Some(active) = this.state.lock().unwrap().active.get_mut(&key) {
                active.checkpoint = Some(row.id.clone());
            }
            this.notify(&key.0, &key.1, row.warning.as_deref());
            Ok::<_, String>(())
        });
        let warning = match tokio::time::timeout(Duration::from_secs(2), &mut task).await {
            Ok(Ok(Ok(()))) => None,
            Ok(Ok(Err(e))) => Some(e),
            Ok(Err(e)) => Some(e.to_string()),
            Err(_) => Some("checkpoint unavailable: snapshot exceeded 2 s".into()),
        };
        if let Some(warning) = warning {
            self.notify(workspace, thread, Some(&warning));
        }
        Ok(())
    }
    pub fn observe(self: &Arc<Self>, event: &AppServerEvent) -> Option<tokio::task::JoinHandle<()>> {
        let p = &event.message["params"];
        let Some(thread) = p["threadId"].as_str() else {
            return None;
        };
        let key = (event.workspace_id.clone(), thread.into());
        let method = event.message["method"].as_str().unwrap_or("");
        {
            let mut state = self.state.lock().unwrap();
            if let Some(active) = state.active.get_mut(&key) {
                if let Some(turn) = p["turnId"].as_str().or(p["turn"]["id"].as_str()) {
                    active.metadata["turnId"] = turn.into();
                }
                if p["item"]["type"] == "userMessage" && !active.user_bound {
                    active.metadata["userItemId"] = p["item"]["id"].clone();
                    active.user_bound = true;
                }
            }
        }
        if method == "turn/completed" {
            return self.complete(&key.0, &key.1);
        }
        None
    }
    pub fn complete(self: &Arc<Self>, workspace: &str, thread: &str) -> Option<tokio::task::JoinHandle<()>> {
        let key = (workspace.to_owned(), thread.to_owned());
        let active = self.state.lock().unwrap().active.remove(&key);
        let Some(mut active) = active else {
            return None;
        };
        let this = self.clone();
        Some(tokio::spawn(async move {
            let _operation = this.operation.lock().await;
            let result = async {
                let repo = this.repo(&key.0)?;
                active
                    .metadata
                    .as_object_mut()
                    .unwrap()
                    .remove("checkpointId");
                active.metadata["kind"] = json!("after");
                active.metadata["timestamp"] = json!(chrono::Utc::now().to_rfc3339());
                tokio::task::spawn_blocking(move || {
                    if let Some(id) = active.checkpoint {
                        repo.bind(
                            active.metadata["threadId"].as_str().unwrap(),
                            &id,
                            active.metadata["turnId"].as_str().unwrap_or(""),
                            active.metadata["userItemId"].as_str().unwrap_or(""),
                        )?;
                    }
                    repo.snapshot(active.metadata)
                })
                .await
                .map_err(|e| e.to_string())?
            }
            .await;
            let warning = match result {
                Ok(row) => row.warning,
                Err(e) => Some(e),
            };
            this.notify(&key.0, &key.1, warning.as_deref());
        }))
    }
    pub async fn request(
        &self,
        session: &super::engines::WorkspaceSession,
        workspace: &str,
        params: Value,
    ) -> Result<Value, String> {
        let thread = params["threadId"].as_str().ok_or("Missing threadId")?;
        let key = (workspace.to_owned(), thread.to_owned());
        let _operation = self.operation.lock().await;
        let repo = self.repo(workspace)?;
        let action = params["action"]
            .as_str()
            .ok_or("Missing checkpoint action")?;
        if action == "list" {
            let thread = thread.to_owned();
            let rows = tokio::task::spawn_blocking(move || repo.list(&thread))
                .await
                .map_err(|e| e.to_string())??;
            return Ok(
                json!({"checkpoints":rows,"undoAvailable":self.state.lock().unwrap().undo.contains_key(&key)}),
            );
        }
        {
            let mut state = self.state.lock().unwrap();
            if !state.active.is_empty() {
                return Err("Finish or interrupt running turns before rewinding".into());
            }
            state.rewinding = true;
        }
        let _rewind = RewindGuard(&self.state);
        if let Some(backend) = session.codex.lock().await.as_ref() {
            if backend.active_threads.lock().await.contains(thread) {
                return Err("Finish or interrupt the turn before rewinding".into());
            }
        }
        if session.pi.is_running(thread).await {
            return Err("Finish or interrupt the turn before rewinding".into());
        }
        let pi = session.pi.owns(workspace, thread).await;
        let claude = session.claude.owns(workspace, thread).await;
        let engine = if claude { "claude" } else if pi { "pi" } else { "codex" };
        if action == "capabilities" {
            let supported = if pi {
                true
            } else if claude {
                false
            } else {
                let backend = session.codex_backend().await?;
                // A zero-turn rollback is a non-mutating feature probe. Supported
                // servers either accept it or reject the count as invalid params.
                match backend
                    .send_request_for_workspace(
                        workspace,
                        "thread/rollback",
                        rollback_params(thread, 0),
                    )
                    .await
                {
                    Ok(v) => !unsupported(&v.to_string()),
                    Err(e) => !unsupported(&e),
                }
            };
            return Ok(json!({"conversation":supported,"engine":engine}));
        }
        if action == "diff" {
            let thread = thread.to_owned();
            let id = params["checkpointId"]
                .as_str()
                .ok_or("Missing checkpointId")?
                .to_owned();
            let changes = tokio::task::spawn_blocking(move || repo.diff(&thread, &id))
                .await
                .map_err(|e| e.to_string())??;
            return Ok(json!({"changes":changes}));
        }
        if action == "undo" {
            let undo = self
                .state
                .lock()
                .unwrap()
                .undo
                .get(&key)
                .cloned()
                .ok_or("Undo is no longer available")?;
            let copy = repo.clone();
            let t = thread.to_owned();
            let u = undo.clone();
            tokio::task::spawn_blocking(move || {
                copy.restore(&t, &u.checkpoint, &u.paths, safety_metadata(&t, &u.engine))
            })
            .await
            .map_err(|e| e.to_string())??;
            if let Some(leaf) = undo.leaf {
                session
                    .pi
                    .rewind(workspace, thread, None, Some(leaf))
                    .await?;
            }
            self.state.lock().unwrap().undo.remove(&key);
            return Ok(json!({"rewound":false,"conversationChanged":pi,"undoAvailable":false}));
        }
        if action != "rewind" {
            return Err("Unknown checkpoint action".into());
        }
        let mode = params["mode"].as_str().ok_or("Missing rewind mode")?;
        if !["both", "files", "conversation"].contains(&mode) {
            return Err("Invalid rewind mode".into());
        }
        let id = params["checkpointId"]
            .as_str()
            .ok_or("Missing checkpointId")?
            .to_owned();
        let paths: Vec<String> =
            serde_json::from_value(params["paths"].clone()).map_err(|e| e.to_string())?;
        let copy = repo.clone();
        let t = thread.to_owned();
        let rows = tokio::task::spawn_blocking(move || copy.list(&t))
            .await
            .map_err(|e| e.to_string())??;
        let target = rows
            .iter()
            .find(|r| r.id == id && r.commit.is_some())
            .ok_or("Checkpoint unavailable")?;
        let conversation = mode != "files";
        if conversation && claude {
            return Err("Conversation rewind isn't supported for Claude Code threads yet".into());
        }
        if conversation && target.metadata["kind"] != "before" {
            return Err("Conversation rewind requires a before checkpoint".into());
        }
        let mut rollback = None;
        if conversation && !pi {
            let backend = session.codex_backend().await?;
            let response = backend
                .send_request_for_workspace(
                    workspace,
                    "thread/read",
                    json!({"threadId":thread,"includeTurns":true}),
                )
                .await?;
            let turns = response
                .pointer("/result/thread/turns")
                .and_then(Value::as_array)
                .ok_or("Cannot read codex history")?;
            let index = turns
                .iter()
                .position(|turn| turn["id"] == target.metadata["turnId"])
                .ok_or("Message is not on the active conversation")?;
            rollback = Some(rollback_params(thread, turns.len() - index));
        }
        let selected = if mode == "conversation" {
            vec![]
        } else {
            paths
        };
        let copy = repo.clone();
        let t = thread.to_owned();
        let selected_copy = selected.clone();
        let safety = tokio::task::spawn_blocking(move || {
            copy.restore(&t, &id, &selected_copy, safety_metadata(&t, engine))
        })
        .await
        .map_err(|e| e.to_string())??;
        self.state.lock().unwrap().undo.insert(
            key.clone(),
            Undo {
                checkpoint: safety.id,
                paths: selected,
                leaf: None,
                engine: engine.into(),
            },
        );
        let mut error = None;
        if conversation {
            if pi {
                match session
                    .pi
                    .rewind(
                        workspace,
                        thread,
                        target.metadata["userItemId"].as_str(),
                        None,
                    )
                    .await
                {
                    Ok(leaf) => {
                        if let Some(undo) = self.state.lock().unwrap().undo.get_mut(&key) {
                            undo.leaf = Some(leaf);
                        }
                    }
                    Err(e) => error = Some(e),
                }
            } else {
                match session
                    .codex_backend()
                    .await?
                    .send_request_for_workspace(workspace, "thread/rollback", rollback.unwrap())
                    .await
                {
                    Ok(v) if v.get("error").is_some() => error = Some(v["error"].to_string()),
                    Err(e) => error = Some(e),
                    _ => (),
                }
            }
        }
        Ok(
            json!({"rewound":true,"undoAvailable":true,"conversationChanged":conversation && error.is_none(),"text":target.metadata["preview"],"error":error}),
        )
    }
}
fn safety_metadata(thread: &str, engine: &str) -> Value {
    json!({"threadId":thread,"turnId":"","userItemId":"","kind":"pre-rewind","timestamp":chrono::Utc::now().to_rfc3339(),"engine":engine})
}
fn unsupported(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("-32601")
        || e.contains("method not found")
        || e.contains("unknown method")
        || e.contains("unsupported")
}
fn rollback_params(thread: &str, count: usize) -> Value {
    json!({"threadId":thread,"numTurns":count})
}
#[derive(Clone)]
pub struct CheckpointSink<E: EventSink> {
    pub inner: E,
    pub checkpoints: Arc<Checkpoints>,
}
impl<E: EventSink> EventSink for CheckpointSink<E> {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        self.checkpoints.observe(&event);
        self.inner.emit_app_server_event(event);
    }
    fn emit_terminal_output(&self, event: TerminalOutput) {
        self.inner.emit_terminal_output(event);
    }
    fn emit_terminal_exit(&self, event: TerminalExit) {
        self.inner.emit_terminal_exit(event);
    }
}
pub async fn request(
    sessions: &tokio::sync::Mutex<HashMap<String, Arc<super::engines::WorkspaceSession>>>,
    workspace: String,
    params: Value,
) -> Result<Value, String> {
    let session = sessions
        .lock()
        .await
        .get(&workspace)
        .cloned()
        .ok_or("Workspace is not connected")?;
    session
        .checkpoints
        .as_ref()
        .ok_or("Checkpoints unavailable")?
        .request(&session, &workspace, params)
        .await
}
