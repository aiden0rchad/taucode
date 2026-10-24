use crate::{remote_backend, state::AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn read_chat_attachment(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || crate::shared::chats::attachment(std::path::Path::new(&path)))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn chat_request(state: State<'_, AppState>, app: AppHandle, action: String, params: Value) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await {
        return remote_backend::call_remote(&*state, app, "chat_request", json!({"action":action,"params":params})).await;
    }
    let settings = state.app_settings.lock().await.clone();
    state.chats.request(&settings, &action, params).await
}

// Window mode is local UI state even when conversations run on a remote daemon.
#[tauri::command]
pub(crate) async fn set_window_mode(state: State<'_, AppState>, window: tauri::WebviewWindow, mode: String) -> Result<(), String> {
    if !["code", "chat", "work"].contains(&mode.as_str()) { return Err("Invalid mode".into()); }
    let mut settings = state.app_settings.lock().await;
    let mut updated = settings.clone();
    updated.window_modes.insert(window.label().into(), mode);
    crate::storage::write_settings(&state.settings_path, &updated)?;
    *settings = updated;
    Ok(())
}
