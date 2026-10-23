use std::{sync::Arc, time::Duration};

use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::*,
    service::{NotificationContext, RunningService},
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use tokio::{process::Command, sync::Notify, time::timeout};

struct Client {
    changes: Arc<Notify>,
    processes: tempfile::TempDir,
}
impl ClientHandler for Client {
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.changes.notify_one();
    }
}

async fn start(state: Option<&std::path::Path>) -> RunningService<RoleClient, Client> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tau-mcp-proxy"));
    cmd.arg("--")
        .arg(env!("CARGO_BIN_EXE_tau-mcp-fake-upstream"));
    cmd.env_remove("TAUCODE_STATE_DIR");
    let processes = tempfile::tempdir().unwrap();
    cmd.env("TAUCODE_FAKE_LIFECYCLE_DIR", processes.path());
    if let Some(state) = state {
        cmd.env("TAUCODE_STATE_DIR", state);
    }
    timeout(
        Duration::from_secs(10),
        Client {
            changes: Arc::new(Notify::new()),
            processes,
        }
        .serve(TokioChildProcess::new(cmd).unwrap()),
    )
    .await
    .unwrap()
    .unwrap()
}

fn request(tool: &str, meta: Option<Value>) -> CallToolRequestParams {
    let mut request = CallToolRequestParams::new(tool.to_owned());
    request.meta = meta.map(|meta| meta.as_object().unwrap().clone().into());
    request
}

async fn who(client: &RunningService<RoleClient, Client>, key: Option<&str>) -> Value {
    let result = timeout(
        Duration::from_secs(10),
        client.call_tool(request("whoami", key.map(|key| json!({"threadId":key})))),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.meta.as_ref().unwrap().get("upstream"),
        Some(&json!(true))
    );
    let text = serde_json::to_value(&result.content).unwrap();
    let value: Value = serde_json::from_str(text[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(result.structured_content.as_ref(), Some(&value));
    value
}

async fn assert_stopped(processes: &std::path::Path, session: &Value) {
    timeout(Duration::from_secs(5), async {
        while processes.join(session.to_string()).exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("upstream child was not terminated");
}

#[tokio::test]
async fn sessions_epochs_metadata_and_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let client = start(Some(dir.path())).await;
    let info = client.peer_info().unwrap();
    assert_eq!(
        info.instructions.as_deref(),
        Some("fake upstream instructions")
    );
    assert_eq!(info.protocol_version, ProtocolVersion::V_2025_11_25);
    assert_eq!(
        info.server_info.as_ref().unwrap().name,
        "tau-mcp-fake-upstream"
    );
    assert_eq!(
        info.capabilities.tools.as_ref().unwrap().list_changed,
        Some(true)
    );
    let list = client.list_tools(None).await.unwrap();
    assert_eq!(
        list.tools
            .iter()
            .map(|t| t.name.as_ref())
            .collect::<Vec<_>>(),
        ["whoami", "crash"]
    );
    let mut next_page = PaginatedRequestParams::default();
    next_page.cursor = list.next_cursor;
    let page = client.list_tools(Some(next_page)).await.unwrap();
    assert_eq!(page.tools.len(), 1);
    assert_eq!(page.tools[0].name, "crash");

    let default = who(&client, None).await;
    let a = who(&client, Some("A")).await;
    let a2 = who(&client, Some("A")).await;
    let b = who(&client, Some("B")).await;
    assert_eq!(a["session"], a2["session"]);
    assert_eq!(a2["calls"], 2);
    assert_ne!(a["session"], b["session"]);
    assert_ne!(a["session"], default["session"]);
    assert_eq!(
        who(&client, Some("default")).await["session"],
        default["session"]
    );

    tau_mcp_proxy::epoch::bump_epoch(dir.path(), "A");
    let reset = who(&client, Some("A")).await;
    assert_ne!(reset["session"], a["session"]);
    assert_stopped(client.service().processes.path(), &a["session"]).await;
    assert_eq!(reset["calls"], 1);
    assert_eq!(who(&client, Some("B")).await["session"], b["session"]);
    let mut meta =
        json!({"threadId":"A", "nested":{"x":[1,true]}, "progressToken":"token", "custom":"value"});
    let handle = client
        .send_request_with_option(
            ClientRequest::CallToolRequest(CallToolRequest::new(request(
                "whoami",
                Some(meta.clone()),
            ))),
            Default::default(),
        )
        .await
        .unwrap();
    // The test client's SDK also assigns a progress token. This is the metadata
    // actually sent to the proxy, which must survive the second SDK unchanged.
    meta["progressToken"] = serde_json::to_value(&handle.progress_token).unwrap();
    let ServerResult::CallToolResult(result) = handle.await_response().await.unwrap() else {
        panic!("expected tool result");
    };
    assert_eq!(result.structured_content.unwrap()["meta"], meta);

    assert!(
        timeout(
            Duration::from_secs(10),
            client.call_tool(request("crash", Some(json!({"threadId":"A"}))))
        )
        .await
        .unwrap()
        .is_err()
    );
    let recovered = who(&client, Some("A")).await;
    assert_ne!(recovered["session"], reset["session"]);
    assert_eq!(recovered["calls"], 1);
    // Ordinary MCP errors preserve a healthy session.
    assert!(
        client
            .call_tool(request("unknown", Some(json!({"threadId":"A"}))))
            .await
            .is_err()
    );
    assert_eq!(
        who(&client, Some("A")).await["session"],
        recovered["session"]
    );
    let processes = client.service().processes.path().to_owned();
    let mut client = client;
    client.close().await.unwrap();
    for session in [&default["session"], &b["session"], &recovered["session"]] {
        assert_stopped(&processes, session).await;
    }
}

#[tokio::test]
async fn concurrent_keys_and_default_notifications_without_state_dir() {
    let client = start(None).await;
    let (a, b) = tokio::join!(who(&client, Some("A")), who(&client, Some("A")));
    assert_eq!(a["session"], b["session"]);
    let mut counts = [a["calls"].as_u64().unwrap(), b["calls"].as_u64().unwrap()];
    counts.sort();
    assert_eq!(counts, [1, 2]);
    let slow = request("whoami", Some(json!({"threadId":"slow"})))
        .with_arguments(json!({"sleep_ms":1500}).as_object().unwrap().clone());
    let (slow, fast) = tokio::join!(client.call_tool(slow), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        timeout(Duration::from_secs(1), who(&client, Some("fast")))
            .await
            .unwrap()
    });
    assert_ne!(
        slow.unwrap().structured_content.unwrap()["session"],
        fast["session"]
    );
    let notify =
        request("whoami", None).with_arguments(json!({"notify":true}).as_object().unwrap().clone());
    client.call_tool(notify).await.unwrap();
    timeout(Duration::from_secs(2), client.service().changes.notified())
        .await
        .unwrap();
    assert_eq!(who(&client, Some("A")).await["session"], a["session"]);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn evicts_least_recently_used_thread() {
    let client = start(None).await;
    let oldest = who(&client, Some("oldest")).await;
    let warm = who(&client, Some("warm")).await;
    for index in 0..30 {
        who(&client, Some(&format!("thread-{index}"))).await;
    }
    assert_eq!(who(&client, Some("warm")).await["session"], warm["session"]);
    who(&client, Some("new")).await;
    assert_stopped(client.service().processes.path(), &oldest["session"]).await;
    assert_eq!(who(&client, Some("warm")).await["session"], warm["session"]);
    assert_ne!(
        who(&client, Some("oldest")).await["session"],
        oldest["session"]
    );
    client.cancel().await.unwrap();
}
