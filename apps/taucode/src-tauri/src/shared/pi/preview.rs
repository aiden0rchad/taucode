use crate::shared::managed_tasks::{ManagedTasks, TaskRuntime};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

pub(super) struct Preview {
    pub manager: Arc<ManagedTasks>,
    pub data: PathBuf,
    pub access: PathBuf,
    pub workspace: String,
    pub thread: String,
    pub cwd: PathBuf,
    pub runtime: TaskRuntime,
    pub secrets: Vec<String>,
}
impl Preview {
    pub async fn request(&self, params: Value) -> Result<Value, String> {
        let action = params["action"].as_str().ok_or("Missing preview action")?;
        if !matches!(action, "start" | "list" | "stop") {
            return Err("Unknown preview action".into());
        }
        if action == "start" {
            let features = crate::shared::engine_registry::features(Some(&self.data));
            if !features.work_mode || !features.terminal {
                return Err("Enable Work and Terminal in Settings → Features".into());
            }
            let access: Value =
                serde_json::from_slice(&std::fs::read(&self.access).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if access["kind"] != "work"
                || !matches!(access["accessMode"].as_str(), Some("ask" | "full-access"))
            {
                return Err("Preview commands require a Work task with write access".into());
            }
        }
        // Scope comes from the host's session; tool arguments cannot supply another task/cwd.
        self.manager
            .request_owned(
                &self.workspace,
                &self.thread,
                &self.cwd,
                action,
                params.clone(),
                self.runtime.clone(),
            )
            .await
    }
    pub async fn stop(&self) -> Result<(), String> {
        self.manager
            .stop_session(&self.workspace, &self.thread)
            .await
    }
}
