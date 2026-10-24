use super::*;

pub(super) async fn dispatch_rpc_request(
    state: &DaemonState,
    method: &str,
    params: &Value,
    client_version: &str,
) -> Result<Value, String> {
    if method == "import_request" {
        let action = params["action"].as_str().ok_or("Missing import action")?;
        if action == "export" { return Err("Export file selection is local-only".into()); }
        return state.imports.request(action, params["params"].clone(), &state.workspaces, &state.app_settings).await;
    }
    if method == "chat_request" {
        let settings = state.app_settings.lock().await.clone();
        return state.chats.request(&settings, params["action"].as_str().ok_or("Missing chat action")?, params["params"].clone()).await;
    }
    if let Some(result) = daemon::try_handle(state, method, params).await {
        return result;
    }

    if let Some(result) = workspace::try_handle(state, method, params, client_version).await {
        return result;
    }

    if let Some(result) = codex::try_handle(state, method, params).await {
        return result;
    }

    if let Some(result) = git::try_handle(state, method, params).await {
        return result;
    }

    if let Some(result) = prompts::try_handle(state, method, params).await {
        return result;
    }

    Err(format!("unknown method: {method}"))
}
