use super::*;
use crate::backend::events::{TerminalExit, TerminalOutput};
use std::sync::Mutex as StdMutex;

#[derive(Clone, Default)]
pub(crate) struct RecordingSink(pub(crate) Arc<StdMutex<Vec<AppServerEvent>>>);

impl EventSink for RecordingSink {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        self.0.lock().unwrap().push(event);
    }
    fn emit_terminal_output(&self, _: TerminalOutput) {}
    fn emit_terminal_exit(&self, _: TerminalExit) {}
}

pub(crate) fn make_session() -> Arc<WorkspaceSession> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "more"]);
        command
    } else {
        Command::new("cat")
    };
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    Arc::new(WorkspaceSession {
        codex_args: None,
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        request_context: Mutex::new(HashMap::new()),
        thread_workspace: Mutex::new(HashMap::new()),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: "fixture".into(),
        workspace_ids: Mutex::new(HashSet::from(["fixture".into()])),
        workspace_roots: Mutex::new(HashMap::new()),
    })
}

#[test]
fn recognizes_0164_subagent_thread_references() {
    assert_eq!(
        extract_related_thread_ids(&json!({"params": {"threadId": "parent", "item": {
            "type": "subAgentActivity", "agentThreadId": "child"
        }}})),
        vec!["parent", "child"]
    );
    assert_eq!(
        extract_related_thread_ids(&json!({"params": {"item": {
            "type": "collabAgentToolCall", "agentsStates": {"child": {"status": "completed"}}
        }}})),
        vec!["child"]
    );
}

#[test]
fn replays_every_0164_server_message_through_production_router() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixtures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-0.164");
        let mut fixture_count = 0;
        for file in std::fs::read_dir(fixtures).unwrap() {
            let path = file.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            fixture_count += 1;
            let session = make_session();
            let sink = RecordingSink::default();
            let mut server_count = 0;
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                let row: Value = serde_json::from_str(line).unwrap();
                let msg = row["msg"].clone();
                if row["dir"] == "c2s" {
                    if let (Some(id), Some(method)) = (msg["id"].as_u64(), msg["method"].as_str()) {
                        session.request_context.lock().await.insert(
                            id,
                            RequestContext {
                                workspace_id: "fixture".into(),
                                method: method.into(),
                            },
                        );
                    }
                    continue;
                }
                server_count += 1;
                let is_response = msg.get("result").is_some() || msg.get("error").is_some();
                let receiver = if is_response {
                    let (tx, rx) = oneshot::channel();
                    session
                        .pending
                        .lock()
                        .await
                        .insert(msg["id"].as_u64().unwrap(), tx);
                    Some(rx)
                } else {
                    None
                };
                let before = sink.0.lock().unwrap().len();
                route_app_server_message(&session, "fixture", &sink, msg.clone()).await;
                if let Some(rx) = receiver {
                    assert_eq!(rx.await.unwrap(), msg, "{} response", path.display());
                    assert_eq!(sink.0.lock().unwrap().len(), before);
                } else {
                    let events = sink.0.lock().unwrap();
                    assert_eq!(events.len(), before + 1, "{}: {msg}", path.display());
                    assert_eq!(events.last().unwrap().message, msg);
                    assert_eq!(events.last().unwrap().workspace_id, "fixture");
                }
            }
            assert!(server_count > 0);
            session.child.lock().await.kill().await.unwrap();
        }
        assert_eq!(fixture_count, 4);
    });
}

#[test]
fn routes_string_request_ids_and_resolutions_to_thread_workspace() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
    let session = make_session();
    session.thread_workspace.lock().await.insert("thread".into(), "other".into());
    let sink = RecordingSink::default();
    for msg in [
        json!({"id":"question", "method":"item/tool/requestUserInput", "params":{"threadId":"thread"}}),
        json!({"method":"serverRequest/resolved", "params":{"threadId":"thread", "requestId":"question"}}),
    ] {
        route_app_server_message(&session, "fixture", &sink, msg.clone()).await;
        let events = sink.0.lock().unwrap();
        assert_eq!(events.last().unwrap().message, msg);
        assert_eq!(events.last().unwrap().workspace_id, "other");
    }
    session.child.lock().await.kill().await.unwrap();
    });
}
