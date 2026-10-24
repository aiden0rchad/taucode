use crate::{remote_backend, state::AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn import_request(
    state: State<'_, AppState>,
    app: AppHandle,
    action: String,
    params: Value,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await {
        // File picker paths refer to this computer, never the remote host.
        if action == "export" {
            return Err("Import export files from a local backend window".into());
        }
        return remote_backend::call_remote(
            &*state,
            app,
            "import_request",
            json!({"action":action,"params":params}),
        )
        .await;
    }
    state
        .imports
        .request(&action, params, &state.workspaces, &state.app_settings)
        .await
}
