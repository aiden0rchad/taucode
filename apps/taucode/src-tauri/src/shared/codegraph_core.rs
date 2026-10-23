use crate::{
    backend::events::{AppServerEvent, EventSink},
    codegraph::{CodegraphSetup, CODEGRAPH_DIR},
    types::WorkspaceEntry,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

pub(crate) fn enabled(global: bool, workspace: Option<bool>) -> bool {
    global && workspace.unwrap_or(true)
}

pub(crate) async fn workspace_status<E: EventSink>(
    manager: &Arc<Codegraph>,
    workspaces: &Mutex<HashMap<String, WorkspaceEntry>>,
    settings: &Mutex<crate::types::AppSettings>,
    workspace_id: &str,
    rebuild: bool,
    sink: E,
) -> Result<CodegraphStatus, String> {
    let entry = workspaces
        .lock()
        .await
        .get(workspace_id)
        .cloned()
        .ok_or("Workspace not found")?;
    let global = settings.lock().await.codegraph_enabled;
    Ok(if rebuild {
        manager.start(entry, global, true, sink).await
    } else {
        manager.status(&entry, global).await
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct CodegraphStatus {
    pub state: String,
    pub detail: String,
}

impl CodegraphStatus {
    fn new(state: &str, detail: impl Into<String>) -> Self {
        Self {
            state: state.into(),
            detail: detail.into(),
        }
    }
}

pub(crate) struct Codegraph {
    pub setup: Option<CodegraphSetup>,
    statuses: Mutex<HashMap<String, CodegraphStatus>>,
}

impl Codegraph {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            setup: crate::codegraph::discover(data_dir),
            statuses: Mutex::new(HashMap::new()),
        })
    }

    pub async fn status(&self, entry: &WorkspaceEntry, global: bool) -> CodegraphStatus {
        if !enabled(global, entry.settings.codegraph_enabled) {
            return CodegraphStatus::new(
                "disabled",
                "CodeGraph is disabled. Changes apply on the next app-server spawn.",
            );
        }
        if self.setup.is_none() {
            return CodegraphStatus::new(
                "unavailable",
                "Install codegraph on PATH, then restart the app or daemon.",
            );
        }
        self.statuses
            .lock()
            .await
            .get(&entry.id)
            .cloned()
            .unwrap_or_else(|| {
                if Path::new(&entry.path).join(CODEGRAPH_DIR).is_dir() {
                    CodegraphStatus::new("ready", "Private index is ready.")
                } else {
                    CodegraphStatus::new(
                        "unavailable",
                        "Connect this workspace to create its private index.",
                    )
                }
            })
    }

    pub async fn start<E: EventSink>(
        self: &Arc<Self>,
        entry: WorkspaceEntry,
        global: bool,
        force: bool,
        sink: E,
    ) -> CodegraphStatus {
        if !enabled(global, entry.settings.codegraph_enabled) || self.setup.is_none() {
            let status = self.status(&entry, global).await;
            emit(&sink, &entry.id, &status);
            return status;
        }
        let mut statuses = self.statuses.lock().await;
        if let Some(status) = statuses
            .get(&entry.id)
            .filter(|status| status.state == "indexing")
        {
            return status.clone();
        }
        let status = CodegraphStatus::new("indexing", "Building private index…");
        statuses.insert(entry.id.clone(), status.clone());
        emit(&sink, &entry.id, &status);
        drop(statuses);
        let manager = self.clone();
        let program = self.setup.as_ref().unwrap().upstream[0].clone();
        tokio::spawn(async move {
            let path = PathBuf::from(&entry.path);
            let exclude_path = path.clone();
            let result = async {
                tokio::task::spawn_blocking(move || exclude_index(&exclude_path))
                    .await
                    .map_err(|e| e.to_string())??;
                if force || !path.join(CODEGRAPH_DIR).is_dir() {
                    let mut command = crate::shared::process_core::tokio_command(&program);
                    command
                        .arg("init")
                        .env("CODEGRAPH_DIR", CODEGRAPH_DIR)
                        .current_dir(&path)
                        .kill_on_drop(true);
                    if force {
                        command.arg("--force");
                    }
                    let output = tokio::time::timeout(Duration::from_secs(300), command.output())
                        .await
                        .map_err(|_| "Indexing timed out after 5 minutes.".to_string())?
                        .map_err(|e| e.to_string())?;
                    if !output.status.success() {
                        return Err(format!(
                            "codegraph init failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    if !path.join(CODEGRAPH_DIR).is_dir() {
                        return Err("codegraph init did not create the private index.".into());
                    }
                }
                Ok::<_, String>(())
            }
            .await;
            let status = match result {
                Ok(()) => CodegraphStatus::new("ready", "Private index is ready."),
                Err(error) => CodegraphStatus::new("failed", error),
            };
            manager
                .statuses
                .lock()
                .await
                .insert(entry.id.clone(), status.clone());
            emit(&sink, &entry.id, &status);
        });
        status
    }
}

fn emit<E: EventSink>(sink: &E, workspace_id: &str, status: &CodegraphStatus) {
    sink.emit_app_server_event(AppServerEvent {
        workspace_id: workspace_id.into(),
        message: serde_json::json!({"method": "codegraph/status", "params": status}),
    });
}

pub(crate) fn exclude_index(workspace: &Path) -> Result<(), String> {
    let repo = match git2::Repository::discover(workspace) {
        Ok(repo) => repo,
        Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    // Linked worktrees keep info/exclude in their common git directory.
    let git_dir = match std::fs::read_to_string(repo.path().join("commondir")) {
        Ok(common) => repo.path().join(common.trim()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => repo.path().to_owned(),
        Err(error) => return Err(error.to_string()),
    };
    let path = git_dir.join("info/exclude");
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    if contents
        .lines()
        .any(|line| line.trim() == ".codegraph-taucode/")
    {
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !contents.is_empty() && !contents.ends_with('\n') {
        writeln!(file).map_err(|e| e.to_string())?;
    }
    writeln!(file, ".codegraph-taucode/").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn initialization_is_background_single_flight_and_rebuilds() {
        use crate::backend::app_server::protocol_tests::RecordingSink;
        use std::os::unix::fs::PermissionsExt;
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let dir = std::env::temp_dir().join(format!("codegraph-init-{}", uuid::Uuid::new_v4()));
            git2::Repository::init(&dir).unwrap();
            let program = dir.join("fake-codegraph");
            std::fs::write(&program, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> calls\nwhile [ ! -f release ]; do sleep 0.01; done\nmkdir -p \"$CODEGRAPH_DIR\"\n").unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
            let manager = Arc::new(Codegraph {
                setup: Some(CodegraphSetup { proxy_exe: PathBuf::new(), upstream: vec![program.to_string_lossy().into()], state_dir: dir.clone() }),
                statuses: Mutex::new(HashMap::new()),
            });
            let entry = WorkspaceEntry {
                id: "test".into(), name: "test".into(), path: dir.to_string_lossy().into(),
                kind: crate::types::WorkspaceKind::Main, parent_id: None, worktree: None,
                settings: Default::default(),
            };
            let sink = RecordingSink::default();
            assert_eq!(manager.start(entry.clone(), true, false, sink.clone()).await.state, "indexing");
            assert_eq!(manager.start(entry.clone(), true, true, sink.clone()).await.state, "indexing");
            std::fs::write(dir.join("release"), "").unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while manager.status(&entry, true).await.state == "indexing" { tokio::time::sleep(Duration::from_millis(10)).await; }
            }).await.unwrap();
            assert_eq!(manager.status(&entry, true).await.state, "ready");
            assert_eq!(std::fs::read_to_string(dir.join("calls")).unwrap(), "init\n");
            manager.start(entry.clone(), true, true, sink.clone()).await;
            tokio::time::timeout(Duration::from_secs(5), async {
                while manager.status(&entry, true).await.state == "indexing" { tokio::time::sleep(Duration::from_millis(10)).await; }
            }).await.unwrap();
            assert_eq!(std::fs::read_to_string(dir.join("calls")).unwrap(), "init\ninit --force\n");
            let states: Vec<String> = sink.0.lock().unwrap().iter().map(|event| event.message["params"]["state"].as_str().unwrap().into()).collect();
            assert_eq!(states, ["indexing", "ready", "indexing", "ready"]);
            std::fs::remove_dir_all(dir).unwrap();
        });
    }
    #[test]
    fn enable_matrix() {
        for global in [false, true] {
            for workspace in [None, Some(false), Some(true)] {
                assert_eq!(
                    enabled(global, workspace),
                    global && workspace != Some(false)
                );
            }
        }
    }

    #[test]
    fn exclude_is_idempotent_and_preserves_gitignore() {
        let dir = std::env::temp_dir().join(format!("codegraph-exclude-{}", uuid::Uuid::new_v4()));
        let repo = git2::Repository::init(&dir).unwrap();
        let exclude = repo.path().join("info/exclude");
        std::fs::remove_file(&exclude).unwrap();
        std::fs::write(dir.join(".gitignore"), "keep\n").unwrap();
        exclude_index(&dir).unwrap();
        exclude_index(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(&exclude).unwrap(),
            ".codegraph-taucode/\n"
        );
        std::fs::write(&exclude, "existing").unwrap();
        exclude_index(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(exclude).unwrap(),
            "existing\n.codegraph-taucode/\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(".gitignore")).unwrap(),
            "keep\n"
        );
        drop(repo);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
