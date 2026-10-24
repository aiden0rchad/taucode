use crate::{remote_backend, state::AppState};
use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub(crate) async fn work_request(state: State<'_, AppState>, app: AppHandle, action: String, params: Value) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await {
        return remote_backend::call_remote(&*state, app, "work_request", json!({"action":action,"params":params})).await;
    }
    let settings = state.app_settings.lock().await.clone();
    state.work.request(&settings, &action, params).await
}
#[tauri::command]
pub(crate) async fn work_attachment(state: State<'_, AppState>, space_id: String, path: String) -> Result<String, String> {
    if remote_backend::is_remote_mode(&*state).await { return Err("Attach files on the host running this space".into()); }
    let settings = state.app_settings.lock().await.clone();
    state.work.attachment(&settings, &space_id, std::path::Path::new(&path)).await
}
#[tauri::command]
pub(crate) async fn work_output(state: State<'_, AppState>, app: AppHandle, task_id: String, path: String, action: String) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await { return Err("Open outputs on the host running this space".into()); }
    let settings = state.app_settings.lock().await.clone();
    let path = state.work.output(&settings, &task_id, &path).await?;
    match action.as_str() {
        "open" => app.opener().open_path(path.to_string_lossy(), None::<String>).map_err(|e| e.to_string()),
        "reveal" => app.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string()),
        "preview" => {
            let extension = path.extension().and_then(|p| p.to_str()).unwrap_or("").to_lowercase();
            if !["png","jpg","jpeg","gif","webp","pdf"].contains(&extension.as_str()) { return Err("Quick Look supports images and PDFs".into()); }
            #[cfg(target_os = "macos")]
            {
                tokio::process::Command::new("/usr/bin/qlmanage").arg("-p").arg(&path)
                    .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                    .spawn().map_err(|e| e.to_string())?;
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            app.opener().open_path(path.to_string_lossy(), None::<String>).map_err(|e| e.to_string())
        }
        _ => Err("Unknown output action".into()),
    }
}

