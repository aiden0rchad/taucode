use crate::{remote_backend, shared::checkpoints, state::AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn checkpoint_request(
    workspace_id: String,
    params: Value,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&state).await {
        return remote_backend::call_remote(
            &state,
            app,
            "checkpoint_request",
            json!({"workspaceId":workspace_id,"params":params}),
        )
        .await;
    }
    checkpoints::request(&state.sessions, workspace_id, params).await
}
