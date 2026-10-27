use crate::{remote_backend, state::AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn managed_tasks_request(
    workspace_id: String,
    thread_id: String,
    action: String,
    params: Value,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&state).await {
        return remote_backend::call_remote(&state, app, "managed_tasks_request", json!({
            "workspaceId":workspace_id,"threadId":thread_id,"action":action,"params":params
        })).await;
    }
    state.managed_tasks.request(&state.app_settings, &state.workspaces, &state.sessions, workspace_id, thread_id, action, params).await
}
