pub mod codegraph;
pub mod engine;

use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, broadcast};

use codegraph::CodegraphSetup;
use engine::{Engine, EngineConfig, EngineEvent};

struct CodegraphState(Option<CodegraphSetup>);

#[derive(Default)]
struct EngineState(Mutex<Option<Engine>>);

async fn current_engine(state: &State<'_, EngineState>) -> Result<Engine, String> {
    state
        .0
        .lock()
        .await
        .clone()
        .filter(Engine::is_running)
        .ok_or_else(|| "engine is not running".to_owned())
}

#[tauri::command]
async fn engine_start(
    app: AppHandle,
    state: State<'_, EngineState>,
    codegraph: State<'_, CodegraphState>,
) -> Result<Value, String> {
    let mut engine = state.0.lock().await;
    if engine.as_ref().is_none_or(|engine| !engine.is_running()) {
        let mut config = EngineConfig::default();
        if let Some(setup) = &codegraph.0 {
            config.args.extend(setup.codex_config_args());
        }
        let started = Engine::spawn_with_config(config)
            .await
            .map_err(|error| error.to_string())?;
        forward_events(app, started.subscribe(), codegraph.0.clone());
        *engine = Some(started);
    }
    Ok(json!({
        "running": true,
        "home": std::env::var("HOME").unwrap_or_default()
    }))
}

#[tauri::command]
async fn engine_request(
    method: String,
    params: Value,
    state: State<'_, EngineState>,
    codegraph: State<'_, CodegraphState>,
) -> Result<Value, String> {
    let rollback_thread = (method == "thread/rollback")
        .then(|| params["threadId"].as_str().map(str::to_owned))
        .flatten();
    let result = current_engine(&state)
        .await?
        .request(&method, params)
        .await
        .map_err(|error| error.to_string())?;
    if let (Some(setup), Some(thread)) = (&codegraph.0, rollback_thread) {
        setup.bump_epoch(&thread);
    }
    Ok(result)
}

#[tauri::command]
async fn thread_start(
    cwd: String,
    model: Option<String>,
    model_provider: Option<String>,
    state: State<'_, EngineState>,
) -> Result<Value, String> {
    let mut params = Map::from_iter([("cwd".to_owned(), Value::String(cwd))]);
    insert_nonempty(&mut params, "model", model);
    insert_nonempty(&mut params, "modelProvider", model_provider);
    current_engine(&state)
        .await?
        .request("thread/start", Value::Object(params))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn turn_start(
    thread_id: String,
    text: String,
    state: State<'_, EngineState>,
) -> Result<Value, String> {
    current_engine(&state)
        .await?
        .request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": text, "text_elements": [] }]
            }),
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn turn_interrupt(
    thread_id: String,
    turn_id: String,
    state: State<'_, EngineState>,
) -> Result<Value, String> {
    current_engine(&state)
        .await?
        .request(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn thread_fork(
    thread_id: String,
    model: Option<String>,
    model_provider: Option<String>,
    state: State<'_, EngineState>,
) -> Result<Value, String> {
    let mut params = Map::from_iter([("threadId".to_owned(), Value::String(thread_id))]);
    insert_nonempty(&mut params, "model", model);
    insert_nonempty(&mut params, "modelProvider", model_provider);
    current_engine(&state)
        .await?
        .request("thread/fork", Value::Object(params))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn server_request_respond(
    id: Value,
    result: Option<Value>,
    error: Option<Value>,
    state: State<'_, EngineState>,
) -> Result<Value, String> {
    current_engine(&state)
        .await?
        .respond_to_server_request(id, result, error)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Value::Null)
}

fn insert_nonempty(params: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        params.insert(key.to_owned(), Value::String(value));
    }
}

fn forward_events(
    app: AppHandle,
    mut events: broadcast::Receiver<EngineEvent>,
    codegraph: Option<CodegraphSetup>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(EngineEvent::Notification(message)) => {
                    if let Some(setup) = &codegraph {
                        setup.on_engine_message(&message);
                    }
                    let _ = app.emit("engine://notification", message);
                }
                Ok(EngineEvent::ServerRequest(message)) => {
                    let _ = app.emit("engine://server-request", message);
                }
                Ok(EngineEvent::Stderr(line)) => {
                    let _ = app.emit("engine://stderr", line);
                }
                Ok(EngineEvent::Exit { code }) => {
                    let _ = app.emit("engine://exit", json!({ "code": code }));
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    let _ = app.emit(
                        "engine://stderr",
                        format!("engine event stream dropped {count} messages"),
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(EngineState::default())
        .setup(|app| {
            app.manage(CodegraphState(codegraph::discover(
                app.path().app_data_dir()?,
            )));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_start,
            engine_request,
            thread_start,
            turn_start,
            turn_interrupt,
            thread_fork,
            server_request_respond
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
