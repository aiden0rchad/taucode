use crate::{
    remote_backend,
    shared::providers::{self, KeyStore},
    state::AppState,
};
use serde_json::{json, Value};
use tauri::{AppHandle, State};
#[tauri::command]
pub(crate) async fn engine_runtime(action: String, engine: Option<String>, state: State<'_, AppState>, app: AppHandle) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await {
        return remote_backend::call_remote(&*state, app, "engine_runtime", json!({"action":action,"engine":engine})).await;
    }
    let data = state.settings_path.parent().ok_or("Missing app data directory")?;
    match action.as_str() {
        "prepare" => { crate::shared::engine_runtime::prepare(data, engine.as_deref().ok_or("Missing engine")?, None).await?; }
        "cancel" => crate::shared::engine_runtime::cancel(engine.as_deref().ok_or("Missing engine")?),
        "status" => (),
        _ => return Err("Unknown runtime action".into()),
    }
    Ok(crate::shared::engine_runtime::status(data).await)
}
#[tauri::command]
pub(crate) async fn provider_test_and_fetch(
    id: String,
    model_id: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await {
        return remote_backend::call_remote(
            &*state,
            app,
            "provider_test_and_fetch",
            json!({"id":id,"modelId":model_id}),
        )
        .await;
    }
    providers::test_and_fetch(id, model_id, &state.app_settings, &state.settings_path).await
}
#[tauri::command]
pub(crate) async fn provider_set_api_key(
    id: String,
    key: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        let result = remote_backend::call_remote(
            &*state,
            app,
            "provider_set_api_key",
            json!({"id":id,"key":key}),
        )
        .await?;
        return serde_json::from_value(result).map_err(|_| "Invalid provider response".into());
    }
    if !state
        .app_settings
        .lock()
        .await
        .providers
        .iter()
        .any(|p| p.id == id)
    {
        return Err("Provider not found".into());
    }
    providers::Keychain.set(&id, &key)?;
    for session in state.sessions.lock().await.values() {
        session.credentials_changed().await;
    }
    Ok(())
}
#[tauri::command]
pub(crate) async fn provider_has_api_key(
    id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<bool, String> {
    if remote_backend::is_remote_mode(&*state).await {
        let result =
            remote_backend::call_remote(&*state, app, "provider_has_api_key", json!({"id":id}))
                .await?;
        return serde_json::from_value(result).map_err(|_| "Invalid provider response".into());
    }
    if !state
        .app_settings
        .lock()
        .await
        .providers
        .iter()
        .any(|p| p.id == id)
    {
        return Err("Provider not found".into());
    }
    Ok(providers::Keychain.get(&id)?.is_some())
}
#[tauri::command]
pub(crate) async fn provider_delete_api_key(
    id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(&*state).await {
        let result =
            remote_backend::call_remote(&*state, app, "provider_delete_api_key", json!({"id":id}))
                .await?;
        return serde_json::from_value(result).map_err(|_| "Invalid provider response".into());
    }
    if !state
        .app_settings
        .lock()
        .await
        .providers
        .iter()
        .any(|p| p.id == id)
    {
        return Err("Provider not found".into());
    }
    providers::Keychain.delete(&id)?;
    for session in state.sessions.lock().await.values() {
        session.credentials_changed().await;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn engine_test(definition: crate::types::EngineDefinition, state: State<'_, AppState>, app: AppHandle) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&*state).await { return remote_backend::call_remote(&*state, app, "engine_test", json!({"definition":definition})).await; }
    crate::shared::engine_registry::test(state.settings_path.parent().ok_or("Missing app data directory")?, &definition).await
}

// App-only: TCC and locally installed tools must never be queried on a remote daemon.
#[tauri::command]
pub(crate) fn setup_status() -> Value {
    json!({"permissions": crate::shared::computer::permissions(),
        "codegraph": crate::codegraph::discover(std::env::temp_dir()).is_some()})
}
