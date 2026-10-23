use crate::state::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State, WebviewWindow};
#[tauri::command]
pub(crate) async fn computer_request(
    params: Value,
    state: State<'_, AppState>,
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Value, String> {
    if params["action"] == "runLog" && crate::remote_backend::is_remote_mode(&state).await {
        return crate::remote_backend::call_remote(
            &state,
            app,
            "computer_request",
            json!({"params":params}),
        )
        .await;
    }
    if params["action"] == "approve" {
        if window.label() != "computer-hud" {
            return Err("Approve in the computer HUD".into());
        }
        state.computer.answer(
            params["id"].as_str().ok_or("Missing approval ID")?,
            params["accept"] == true,
        )?;
        return Ok(Value::Null);
    }
    if params["action"] == "permissions" {
        use tauri_plugin_opener::OpenerExt;
        let pane = match params["pane"].as_str() {
            Some("screen") => "Privacy_ScreenCapture",
            Some("accessibility") => "Privacy_Accessibility",
            _ => return Err("Unknown permission".into()),
        };
        app.opener()
            .open_url(
                format!("x-apple.systempreferences:com.apple.preference.security?{pane}"),
                None::<String>,
            )
            .map_err(|e| e.to_string())?;
        return Ok(json!({"ok":true}));
    }
    // Native control always belongs to the visible app on this Mac, with its HUD and kill switch.
    state.computer.request(app, params).await
}
pub(crate) fn install(app: &AppHandle) -> Result<(), String> {
    let computer = app.state::<AppState>().computer.clone();
    tauri::async_runtime::block_on(computer.serve(app.clone()))
}
pub(crate) fn shortcut(app: &AppHandle, event: tauri_plugin_global_shortcut::ShortcutEvent) {
    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
        if let Some(state) = app.try_state::<AppState>() {
            state.computer.stop();
        }
    }
}
