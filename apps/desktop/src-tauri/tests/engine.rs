use std::{ffi::OsString, path::PathBuf, time::Duration};

use desktop_lib::engine::{Engine, EngineConfig, EngineEvent};
use serde_json::{Value, json};
use tokio::time::timeout;

fn config() -> EngineConfig {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fake_app_server.py");
    EngineConfig {
        program: OsString::from("python3"),
        args: vec![fixture.into_os_string()],
        request_timeout: Duration::from_secs(3),
        server_request_timeout: Duration::from_secs(1),
    }
}

async fn start() -> Engine {
    Engine::spawn_with_config(config())
        .await
        .expect("fake app-server should start")
}

#[tokio::test]
async fn handshake_is_initialize_then_initialized() {
    let engine = start().await;
    let result = engine
        .request("test/handshake", json!({}))
        .await
        .unwrap();
    assert_eq!(result["order"], json!(["initialize", "initialized"]));
    assert!(result["initialized"].as_bool().unwrap());
    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_requests_are_routed_by_id() {
    let engine = start().await;
    let (first, second) = tokio::join!(
        engine.request("test/concurrent", json!({ "value": "first" })),
        engine.request("test/concurrent", json!({ "value": "second" }))
    );
    assert_eq!(first.unwrap()["value"], "first");
    assert_eq!(second.unwrap()["value"], "second");
    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn notifications_reach_subscribers_in_order() {
    let engine = start().await;
    let mut events = engine.subscribe();
    engine
        .request("test/notifications", json!({}))
        .await
        .unwrap();

    for expected in 0..3 {
        let event = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        let EngineEvent::Notification(message) = event else {
            panic!("expected notification, got {event:?}");
        };
        assert_eq!(message["params"]["sequence"], expected);
    }
    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn server_request_response_preserves_id() {
    let engine = start().await;
    let mut events = engine.subscribe();
    let request_engine = engine.clone();
    let request = tokio::spawn(async move {
        request_engine
            .request("test/server-request", json!({}))
            .await
    });

    let event = timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    let EngineEvent::ServerRequest(message) = event else {
        panic!("expected server request, got {event:?}");
    };
    assert_eq!(message["id"], 700);
    engine
        .respond_to_server_request(message["id"].clone(), Some(json!({ "decision": "accept" })), None)
        .await
        .unwrap();

    let result = request.await.unwrap().unwrap();
    assert_eq!(result["response"]["id"], 700);
    assert_eq!(result["response"]["result"]["decision"], "accept");
    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn rpc_errors_surface_as_errors() {
    let engine = start().await;
    let error = engine
        .request("test/error", json!({}))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("RPC error -32000: boom"), "{error}");
    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn exit_fails_pending_requests_and_emits_status() {
    let engine = start().await;
    let mut events = engine.subscribe();
    let error = timeout(
        Duration::from_secs(1),
        engine.request("test/pending-exit", Value::Null),
    )
    .await
    .unwrap()
    .unwrap_err()
    .to_string();
    assert!(error.contains("exited with"), "{error}");

    loop {
        let event = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        if let EngineEvent::Exit { code } = event {
            assert_eq!(code, Some(23));
            break;
        }
    }
}
