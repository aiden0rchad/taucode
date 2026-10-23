use crate::{remote_backend, state::AppState};
use serde_json::{json, Value};
use tauri::{ipc::Channel, AppHandle, Manager, State};

#[tauri::command]
pub(crate) async fn browser_request(
    params: Value,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    if remote_backend::is_remote_mode(&state).await {
        return remote_backend::call_remote(
            &state,
            app,
            "browser_request",
            json!({"params":params}),
        )
        .await;
    }
    state.browser.request(params).await
}

#[tauri::command]
pub(crate) async fn browser_subscribe(
    channel: Channel<Value>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let browser = state.browser.clone();
    let remote = remote_backend::is_remote_mode(&state).await;
    let id = uuid::Uuid::new_v4().to_string();
    let task = tokio::spawn(async move {
        let mut events = browser.state.subscribe();
        let mut last = Value::Null;
        loop {
            let state = app.state::<AppState>();
            let result = if remote {
                remote_backend::call_remote(
                    &state,
                    app.clone(),
                    "browser_request",
                    json!({"params":{"action":"poll"}}),
                )
                .await
            } else {
                browser.request(json!({"action":"poll"})).await
            };
            let value = match result {
                Ok(v) => v,
                Err(e) => json!({"error":e}),
            };
            if value != last {
                if channel.send(value.clone()).is_err() {
                    break;
                }
                last = value;
            }
            // Local frames wake immediately; a daemon owner is polled through the
            // existing authenticated RPC/socket, with no exposed CDP port.
            tokio::select! { _ = events.changed() => (), _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => () }
        }
    });
    state.browser_streams.lock().await.insert(id.clone(), task);
    Ok(id)
}

#[tauri::command]
pub(crate) async fn browser_unsubscribe(
    id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let mut streams = state.browser_streams.lock().await;
    if let Some(task) = streams.remove(&id) {
        task.abort();
    }
    if streams.is_empty() {
        browser_request(json!({"action":"detach"}), state.clone(), app).await?;
    }
    Ok(())
}
