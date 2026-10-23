use crate::{
    event_sink::TauriEventSink,
    remote_backend,
    shared::codegraph_core::{self, CodegraphStatus},
    state::AppState,
};
use tauri::{AppHandle, State};

async fn status(
    workspace_id: String,
    rebuild: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<CodegraphStatus, String> {
    if remote_backend::is_remote_mode(&state).await {
        let method = if rebuild {
            "codegraph_rebuild"
        } else {
            "codegraph_status"
        };
        let response = remote_backend::call_remote(
            &state,
            app,
            method,
            serde_json::json!({"workspaceId": workspace_id}),
        )
        .await?;
        return serde_json::from_value(response).map_err(|e| e.to_string());
    }
    codegraph_core::workspace_status(
        &state.codegraph,
        &state.workspaces,
        &state.app_settings,
        &workspace_id,
        rebuild,
        TauriEventSink::new(app),
    )
    .await
}

#[tauri::command]
pub(crate) async fn codegraph_status(
    workspace_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<CodegraphStatus, String> {
    status(workspace_id, false, state, app).await
}

#[tauri::command]
pub(crate) async fn codegraph_rebuild(
    workspace_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<CodegraphStatus, String> {
    status(workspace_id, true, state, app).await
}
