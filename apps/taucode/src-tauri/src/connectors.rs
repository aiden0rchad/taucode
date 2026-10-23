use crate::{
    remote_backend,
    shared::{connectors, providers::Keychain},
    state::AppState,
};
use serde_json::Value;
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn connector_request(
    params: Value,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&state).await {
        return remote_backend::call_remote(
            &state,
            app,
            "connector_request",
            serde_json::json!({"params":params}),
        )
        .await;
    }
    let changed = matches!(
        params["action"].as_str(),
        Some("setSecret" | "deleteSecret")
    );
    let result = connectors::request(
        params,
        &state.app_settings,
        &state.workspaces,
        state.codegraph.setup.as_ref(),
        &Keychain,
        state
            .settings_path
            .parent()
            .ok_or("Missing app data directory")?,
    )
    .await?;
    if changed {
        for session in state.sessions.lock().await.values() {
            session.credentials_changed().await;
        }
    }
    Ok(result)
}
