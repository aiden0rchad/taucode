use super::*;

#[test]
fn unknown_engine_migrates_to_default() {
    assert_eq!(
        serde_json::from_value::<crate::types::EngineKind>(json!("future")).unwrap(),
        "future".to_owned()
    );
}

struct Keys;
impl KeyStore for Keys {
    fn get(&self, _: &str) -> Result<Option<String>, String> {
        Ok(Some("fixture-api-secret".into()))
    }
    fn set(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}
fn provider(kind: &str) -> ProviderEntry {
    serde_json::from_value(json!({"id":"test","name":"Test","kind":kind,"enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"model","enabled":true},{"id":"other","enabled":true}]})).unwrap()
}
fn temp() -> PathBuf {
    let path = std::env::temp_dir().join(format!("taucode-acp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn engine(data: &Path) -> ClaudeAcpEngine {
    let mut engine = ClaudeAcpEngine::new(
        data.to_path_buf(),
        "workspace".into(),
        data.to_string_lossy().into(),
        None,
        Arc::new(|_, _| {}),
    );
    engine.fake = true;
    engine.keys = Arc::new(Keys);
    engine
}
async fn event(rx: &mut broadcast::Receiver<Value>, method: &str) -> Value {
    let mut seen = vec![];
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let e = rx.recv().await.unwrap();
            if e["method"] == method {
                return e;
            }
            seen.push(e);
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Missing {method}; received {seen:?}"))
}
#[test]
fn registry_auth_environment_is_allowlisted_and_isolated() {
    let parent = vec![
        ("HOME".into(), "/home/user".into()),
        ("PATH".into(), "/bin".into()),
        ("ANTHROPIC_API_KEY".into(), "ambient".into()),
        ("CLAUDE_CODE_OAUTH_TOKEN".into(), "oauth".into()),
        ("CLAUDE_CONFIG_DIR".into(), "/home/user/.claude".into()),
        ("CLAUDE_CODE_USE_BEDROCK".into(), "1".into()),
        ("UNRELATED_SECRET".into(), "secret".into()),
    ];
    let mut p = provider("anthropic");
    assert!(!p.anthropic_compatible);
    p.base_url = Some("https://api.example.test".into());
    let env = config::environment(
        parent,
        Path::new("/app/claude/workspace"),
        &p,
        "registry-key",
        "model",
    )
    .unwrap();
    assert_eq!(env["ANTHROPIC_API_KEY"], "registry-key");
    assert_eq!(env["CLAUDE_CONFIG_DIR"], "/app/claude/workspace");
    assert_eq!(env["HOME"], "/app/claude/workspace");
    assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.example.test");
    assert!(!env.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
    assert!(!env.contains_key("CLAUDE_CODE_USE_BEDROCK"));
    assert!(!env.contains_key("UNRELATED_SECRET"));
    p.kind = crate::types::ProviderKind::OpenaiCompatible;
    assert_eq!(
        config::environment(vec![], Path::new("/app"), &p, "key", "model").unwrap_err(),
        config::PROVIDER_ERROR
    );
    p.anthropic_compatible = true;
    let env = config::environment(vec![], Path::new("/app"), &p, "key", "model").unwrap();
    assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "key");
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    p.base_url = None;
    assert!(config::environment(vec![], Path::new("/app"), &p, "key", "model").is_err());
    assert_eq!(
        client::redact("x key x".into(), &["key".into()]),
        "x [REDACTED] x"
    );
    let dir = temp();
    config::private_directory(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn permissions_obey_access_and_connector_policy() {
    use permissions::Decision::*;
    let dir = temp();
    let cwd = dir.to_str().unwrap();
    let read = json!({"kind":"read","name":"Read"});
    let edit = json!({"kind":"edit","name":"Edit","rawInput":{"file_path":"new.txt"}});
    let exec = json!({"kind":"execute","name":"Bash","rawInput":{"command":"echo hi"}});
    assert_eq!(permissions::decide("read-only", cwd, &[], &read), Allow);
    for t in [&edit, &exec] {
        assert_eq!(permissions::decide("read-only", cwd, &[], t), Reject);
        assert_eq!(permissions::decide("ask", cwd, &[], t), Ask);
        assert_eq!(permissions::decide("full-access", cwd, &[], t), Allow);
    }
    assert_eq!(permissions::decide("ask", cwd, &[], &read), Ask);
    assert_eq!(
        permissions::decide(
            "full-access",
            cwd,
            &[],
            &json!({"kind":"edit","rawInput":{"path":"/etc/passwd"}})
        ),
        Ask
    );
    let mut entries = crate::types::default_mcp_servers();
    let browser = entries
        .iter_mut()
        .find(|s| s.id == "taucode-browser")
        .unwrap();
    browser.enabled = true;
    browser.approval = crate::types::McpApproval::Ask;
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_browser__browser_click",
            "full-access"
        ),
        Ask
    );
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_browser__browser_click",
            "read-only"
        ),
        Reject
    );
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_browser__browser_snapshot",
            "read-only"
        ),
        Allow
    );
    entries
        .iter_mut()
        .find(|s| s.id == "taucode-browser")
        .unwrap()
        .tool_denylist = Some(vec!["browser_snapshot".into()]);
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_browser__browser_snapshot",
            "full-access"
        ),
        Reject
    );
    assert_eq!(
        permissions::connector_decision(&entries, "mcp__unknown__tool", "full-access"),
        Reject
    );
    entries
        .iter_mut()
        .find(|s| s.id == "taucode-computer")
        .unwrap()
        .enabled = true;
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_computer__computer_screenshot",
            "read-only"
        ),
        Allow
    );
    assert_eq!(
        permissions::connector_decision(
            &entries,
            "mcp__taucode_computer__computer_action",
            "read-only"
        ),
        Reject
    );
    let options = json!({"options":[{"kind":"allow_once","optionId":"one"},{"kind":"allow_always","optionId":"all"},{"kind":"reject_once","optionId":"no"}]});
    assert_eq!(
        permissions::outcome(&options, "acceptForSession")["outcome"]["optionId"],
        "one"
    );
    assert_eq!(
        permissions::outcome(&options, "decline")["outcome"]["optionId"],
        "no"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn translation_covers_tools_diffs_plans_and_usage() {
    let mut t = Translator::new("thread".into(), "/workspace".into(), vec![]);
    assert_eq!(t.start(json!([]))[0]["method"], "turn/started");
    for (kind, name, expected) in [
        ("execute", "Bash", "commandExecution"),
        ("edit", "Edit", "fileChange"),
        ("read", "Read", "dynamicToolCall"),
        ("search", "Grep", "dynamicToolCall"),
        ("fetch", "WebFetch", "dynamicToolCall"),
        ("other", "mcp__server__tool", "mcpToolCall"),
        ("other", "Agent", "collabAgentToolCall"),
    ] {
        let update = json!({"sessionUpdate":"tool_call","toolCallId":name,"name":name,"title":"human readable title","kind":kind,"rawInput":{"command":"echo hi","prompt":"inspect"},"content":[{"type":"diff","path":"file","oldText":"a\n","newText":"b\n"}]});
        let events = t.update(&update);
        assert_eq!(events[0]["params"]["item"]["type"], expected);
        if kind == "edit" {
            assert!(events[0]["params"]["item"]["changes"][0]["diff"]
                .as_str()
                .unwrap()
                .contains("-a\n+b"));
        }
        let events=t.update(&json!({"sessionUpdate":"tool_call_update","toolCallId":name,"status":"completed","rawOutput":{"exitCode":0}}));
        assert!(events.iter().any(|e| e["method"] == "item/completed"));
    }
    for kind in ["agent_message_chunk", "agent_thought_chunk"] {
        assert!(t
            .update(&json!({"sessionUpdate":kind,"content":{"type":"text","text":"chunk"}}))
            .iter()
            .any(|e| e["params"]["delta"] == "chunk"));
    }
    t.update(&json!({"sessionUpdate":"tool_call","toolCallId":"shell","name":"Bash","kind":"execute","rawInput":{"command":"ls"}}));
    let output = t.update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"shell","_meta":{"terminal_output_delta":{"terminal_id":"shell","data":"file.txt\n"}}}));
    assert!(output
        .iter()
        .any(|e| e["method"] == "item/commandExecution/outputDelta"
            && e["params"]["delta"] == "file.txt\n"));
    let done = t.update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"shell","status":"completed","_meta":{"terminal_exit":{"terminal_id":"shell","exit_code":0}}}));
    assert_eq!(done[0]["params"]["item"]["exitCode"], 0);
    assert_eq!(done[0]["params"]["item"]["aggregatedOutput"], "file.txt\n");
    assert!(t
        .update(
            &json!({"sessionUpdate":"plan","entries":[{"content":"step","status":"in_progress"}]})
        )
        .iter()
        .any(|e| e["method"] == "turn/plan/updated"));
    assert_eq!(
        t.update(&json!({"sessionUpdate":"usage_update","used":42,"size":100}))[0]["params"]
            ["tokenUsage"]["last"]["totalTokens"],
        42
    );
    assert!(t
        .finish("cancelled", None)
        .iter()
        .any(|e| e["params"]["turn"]["status"] == "interrupted"));
    assert!(prompt_input(
        &json!([{"type":"image","url":"data:image/png;base64,YQ=="}]),
        false
    )
    .is_err());
    assert!(prompt_input(
        &json!([{"type":"image","url":"data:image/png;base64,YQ=="}]),
        true
    )
    .is_ok());
}

#[test]
fn replay_groups_user_blocks_and_keeps_turn_identity() {
    let mut t = Translator::new("thread".into(), "/workspace".into(), vec![]);
    t.update(&json!({"sessionUpdate":"user_message_chunk","messageId":"u1","content":{"type":"text","text":"question"}}));
    t.update(&json!({"sessionUpdate":"user_message_chunk","messageId":"u1","content":{"type":"image","mimeType":"image/png","data":"YQ=="}}));
    assert_eq!(t.turns.len(), 1);
    assert_eq!(t.turn_id(), "thread:turn:1");
    assert_eq!(
        t.turns[0]["items"][0]["content"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        t.turns[0]["items"][0]["content"][1]["url"],
        "data:image/png;base64,YQ=="
    );
    t.update(&json!({"sessionUpdate":"user_message_chunk","messageId":"u2","content":{"type":"text","text":"next"}}));
    assert_eq!(t.turn_id(), "thread:turn:2");
}
#[tokio::test]
async fn single_model_policy_disables_acp_delegation_and_reloads_when_changed() {
    let data = temp();
    let engine = engine(&data);
    let provider = provider("anthropic");
    let mut policy = AgentPolicy::default();
    policy.subagents.enabled = false;
    engine.configure("workspace", vec![provider.clone()], policy.clone()).await;
    let start = engine.request("workspace", "thread/start", json!({})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    let params = engine.session_params("workspace", id, data.to_str().unwrap(), "model").await.unwrap();
    let disabled = params["_meta"]["claudeCode"]["options"]["disallowedTools"].as_array().unwrap();
    assert!(disabled.contains(&json!("Task")) && disabled.contains(&json!("Agent")));
    let previous = engine.runtimes.lock().await["workspace"].client.id.clone();
    policy.subagents.enabled = true;
    engine.configure("workspace", vec![provider.clone()], policy).await;
    let client = engine.spawn("workspace", data.to_str().unwrap(), &provider, "model").await.unwrap();
    assert_ne!(client.id, previous);
    let params = engine.session_params("workspace", id, data.to_str().unwrap(), "model").await.unwrap();
    assert!(!params["_meta"]["claudeCode"]["options"]["disallowedTools"].as_array().unwrap().contains(&json!("Task")));
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(data).unwrap();
}
#[tokio::test]
async fn fake_agent_stream_approval_resume_cancel_auth_and_fork() {
    let data = temp();
    let engine = engine(&data);
    engine
        .configure(
            "workspace",
            vec![provider("anthropic")],
            AgentPolicy::default(),
        )
        .await;
    let mut events = engine.events();
    let start = engine
        .request("workspace", "thread/start", json!({}))
        .await
        .unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    let session = start["result"]["thread"]["sessionId"].clone();
    assert_ne!(id, session.as_str().unwrap());
    assert!(engine.owns("workspace", id).await);
    engine
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"input":[{"type":"text","text":"hello"}]}),
        )
        .await
        .unwrap();
    let approval = event(&mut events, "item/commandExecution/requestApproval").await;
    assert_eq!(
        approval["params"]["availableDecisions"],
        json!(["accept", "acceptForSession", "decline", "cancel"])
    );
    engine
        .respond_to_server_request(
            approval["id"].clone(),
            json!({"decision":"acceptForSession"}),
        )
        .await
        .unwrap();
    assert_eq!(
        event(&mut events, "turn/completed").await["params"]["turn"]["status"],
        "completed"
    );
    let history = engine
        .request("workspace", "thread/read", json!({"threadId":id}))
        .await
        .unwrap();
    assert!(!history.to_string().contains("fixture-api-secret"));
    assert!(history["result"]["thread"]["turns"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["type"] == "agentMessage" && i["text"] == "Hello world"));
    engine
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"input":[{"type":"text","text":"same approved command"}]}),
        )
        .await
        .unwrap();
    // The exact command is approved for this session without a durable adapter grant.
    assert_eq!(
        event(&mut events, "turn/completed").await["params"]["turn"]["status"],
        "completed"
    );
    engine
        .request(
            "workspace",
            "thread/name/set",
            json!({"threadId":id,"name":"Title"}),
        )
        .await
        .unwrap();
    engine
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"input":[{"type":"text","text":"wait"}]}),
        )
        .await
        .unwrap();
    event(&mut events, "turn/started").await;
    engine.credentials_changed().await;
    assert!(engine
        .request("workspace", "thread/start", json!({}))
        .await
        .unwrap_err()
        .contains("Finish or interrupt"));
    engine
        .request(
            "workspace",
            "turn/steer",
            json!({"threadId":id,"input":[{"type":"text","text":"followup"}]}),
        )
        .await
        .unwrap();
    engine
        .request("workspace", "turn/interrupt", json!({"threadId":id}))
        .await
        .unwrap();
    assert_eq!(
        event(&mut events, "turn/completed").await["params"]["turn"]["status"],
        "interrupted"
    );
    engine.shutdown().await.unwrap();
    let restored = super::tests::engine(&data);
    restored
        .configure(
            "workspace",
            vec![provider("anthropic")],
            AgentPolicy::default(),
        )
        .await;
    let list = restored
        .request("workspace", "thread/list", json!({}))
        .await
        .unwrap();
    assert_eq!(list["result"]["data"][0]["name"], "Title");
    assert_eq!(list["result"]["data"][0]["sessionId"], session);
    restored
        .request("workspace", "thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    let history = restored
        .request("workspace", "thread/read", json!({"threadId":id}))
        .await
        .unwrap();
    assert!(history.to_string().contains("restored answer"));
    let fork = restored
        .request("workspace", "thread/fork", json!({"threadId":id}))
        .await
        .unwrap();
    assert_ne!(fork["result"]["thread"]["id"], id);
    assert_ne!(fork["result"]["thread"]["sessionId"], session);
    assert_eq!(
        restored
            .request("workspace", "model/list", json!({"threadId":id}))
            .await
            .unwrap()["result"]["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut events = restored.events();
    restored
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"model":"test/other","input":[{"type":"text","text":"auth"}]}),
        )
        .await
        .unwrap();
    let completed = event(&mut events, "turn/completed").await;
    assert_eq!(
        completed["params"]["turn"]["error"]["message"],
        client::AUTH_ERROR
    );
    assert!(!completed.to_string().contains("sign in"));
    restored
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"input":[{"type":"text","text":"wait"}]}),
        )
        .await
        .unwrap();
    restored
        .request("workspace", "turn/interrupt", json!({"threadId":id}))
        .await
        .unwrap();
    assert_eq!(
        event(&mut events, "turn/completed").await["params"]["turn"]["status"],
        "interrupted"
    );
    let client = restored.runtimes.lock().await["workspace"].client.clone();
    client.capabilities.lock().await["agentCapabilities"]["loadSession"] = json!(false);
    restored
        .threads
        .lock()
        .await
        .get_mut(id)
        .unwrap()
        .loaded
        .clear();
    let fallback = restored
        .request("workspace", "thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    assert_ne!(fallback["result"]["thread"]["sessionId"], session);
    assert!(fallback["result"]["thread"]["resumeNote"]
        .as_str()
        .unwrap()
        .contains("fresh"));
    client.capabilities.lock().await["agentCapabilities"]["sessionCapabilities"] = json!({});
    assert_eq!(
        restored
            .request("workspace", "thread/fork", json!({"threadId":id}))
            .await
            .unwrap_err(),
        "Claude Code adapter doesn't support forking sessions"
    );
    restored.shutdown().await.unwrap();
    std::fs::remove_dir_all(data).unwrap();
}

#[tokio::test]
async fn router_checkpoints_sidechats_and_task_transcripts() {
    use crate::backend::events::AppServerEvent;
    use crate::shared::{checkpoints::Checkpoints, engines::WorkspaceSession};
    let data = temp();
    let root = data.join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("file"), "before").unwrap();
    let checkpoint_events = Arc::new(std::sync::Mutex::new(vec![]));
    let captured = checkpoint_events.clone();
    let checkpoints = Checkpoints::with_limits(data.clone(), "workspace".into(), root.to_string_lossy().into(), Arc::new(move |e| captured.lock().unwrap().push(e)), crate::shared::checkpoints::CheckpointLimits::testing());
    let (tx, mut completed) = tokio::sync::mpsc::unbounded_channel();
    let observer = checkpoints.clone();
    let mut session = WorkspaceSession::lazy(Box::new(|| Box::pin(async { Err("Codex must not be used".into()) })), vec![provider("anthropic")], AgentPolicy::default(), None, None, "workspace".into());
    session.checkpoints = Some(checkpoints.clone());
    session.pi = crate::shared::pi::PiEngine::new(data.clone(), "workspace".into(), root.to_string_lossy().into(), None, Arc::new(|_, _| {}));
    session.claude = engine(&data);
    session.claude.sink = Arc::new(move |workspace, message| {
        let complete = message["method"] == "turn/completed";
        if let Some(handle) = observer.observe(&AppServerEvent { workspace_id: workspace.into(), message }) {
            if complete { tx.send(handle).unwrap(); }
        }
    });
    let session = Arc::new(session);
    let start = session.send_request_for_workspace("workspace", "thread/start", json!({"engine":"claude"})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    let mut events = session.claude.events();
    for (prompt, status) in [("hello", "completed"), ("wait", "interrupted"), ("auth", "failed")] {
        session.send_request_for_workspace("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":prompt}]})).await.unwrap();
        if prompt != "auth" {
            let before = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"list"})).await.unwrap();
            assert_eq!(before["checkpoints"].as_array().unwrap().last().unwrap()["metadata"]["kind"], "before");
        }
        std::fs::write(root.join("file"), prompt).unwrap();
        if prompt == "hello" {
            let approval = event(&mut events, "item/commandExecution/requestApproval").await;
            session.claude.respond_to_server_request(approval["id"].clone(), json!({"decision":"accept"})).await.unwrap();
        } else if prompt == "wait" {
            session.send_request_for_workspace("workspace", "turn/interrupt", json!({"threadId":id})).await.unwrap();
        }
        assert_eq!(event(&mut events, "turn/completed").await["params"]["turn"]["status"], status);
        completed.recv().await.unwrap().await.unwrap();
    }
    let rows = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"list"})).await.unwrap();
    let rows = rows["checkpoints"].as_array().unwrap();
    assert_eq!(rows.len(), 6, "checkpoint events: {:?}", checkpoint_events.lock().unwrap().iter().map(|e| &e.message).collect::<Vec<_>>());
    assert!(rows.iter().all(|r| r["commit"].is_string() && r["metadata"]["engine"] == "claude"));
    for pair in rows.chunks_exact(2) {
        assert_eq!(pair[0]["metadata"]["userItemId"], pair[1]["metadata"]["userItemId"]);
        assert_eq!(pair[0]["metadata"]["turnId"], pair[1]["metadata"]["turnId"]);
    }
    assert_ne!(rows[0]["metadata"]["userItemId"], rows[2]["metadata"]["userItemId"]);
    assert_ne!(rows[0]["metadata"]["turnId"], rows[2]["metadata"]["turnId"]);
    let capabilities = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"capabilities"})).await.unwrap();
    assert_eq!(capabilities, json!({"engine":"claude","conversation":false}));
    let rewind = json!({"threadId":id,"action":"rewind","checkpointId":rows[0]["id"],"mode":"conversation","paths":["file"]});
    assert_eq!(checkpoints.request(&session, "workspace", rewind.clone()).await.unwrap_err(), "Conversation rewind isn't supported for Claude Code threads yet");
    let mut files = rewind;
    files["mode"] = json!("files");
    checkpoints.request(&session, "workspace", files).await.unwrap();
    assert_eq!(std::fs::read_to_string(root.join("file")).unwrap(), "before");
    checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"undo"})).await.unwrap();
    assert_eq!(std::fs::read_to_string(root.join("file")).unwrap(), "auth");

    let side = session.send_request_for_workspace("workspace", "thread/sidechat", json!({"threadId":id})).await.unwrap();
    let child = side["result"]["thread"]["id"].as_str().unwrap();
    let read = session.send_request_for_workspace("workspace", "thread/read", json!({"threadId":child})).await.unwrap();
    assert_eq!(read["result"]["thread"]["sideOf"]["threadId"], id);
    let core_sessions = Mutex::new(HashMap::from([("workspace".into(), session.clone())]));
    let workspaces = Mutex::new(HashMap::from([("workspace".into(), serde_json::from_value(json!({"id":"workspace","name":"Workspace","path":root,"settings":{}})).unwrap())]));
    for mode in [Some("current".into()), None] {
        crate::shared::codex_core::send_user_message_core(&core_sessions, &workspaces, "workspace".into(), child.into(), "hello".into(), None, None, None, mode, None, None).await.unwrap();
        event(&mut events, "turn/completed").await;
        completed.recv().await.unwrap().await.unwrap();
    }
    assert_eq!(session.claude.threads.lock().await[child].mode, "read-only");
    assert!(session.claude.dialogs.lock().await.is_empty());
    for mode in ["ask", "full-access"] {
        session.send_request_for_workspace("workspace", "thread/accessMode/set", json!({"threadId":child,"accessMode":mode})).await.unwrap();
        assert_eq!(session.claude.threads.lock().await[child].mode, mode);
    }
    assert!(session.send_request_for_workspace("workspace", "thread/sidechat", json!({"threadId":id,"atItemId":"message"})).await.unwrap_err().contains("Message-level"));
    let client = session.claude.runtimes.lock().await["workspace"].client.clone();
    client.capabilities.lock().await["agentCapabilities"]["sessionCapabilities"] = json!({"fork":false});
    assert_eq!(session.send_request_for_workspace("workspace", "thread/sidechat", json!({"threadId":id})).await.unwrap_err(), "Claude Code adapter doesn't support forking sessions");

    {
        let mut threads = session.claude.threads.lock().await;
        let thread = threads.get_mut(id).unwrap();
        thread.history.start(Value::Null);
        thread.history.update(&json!({"sessionUpdate":"tool_call","toolCallId":"task","title":"Task","rawInput":{"subagent_type":"Explore","description":"Inspect parser"},"status":"in_progress"}));
        thread.history.update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"task","content":[{"type":"content","content":{"type":"text","text":"Found parser"}}],"status":"completed"}));
        thread.history.finish("end_turn", None);
        thread.save().unwrap();
    }
    let read = session.send_request_for_workspace("workspace", "thread/read", json!({"threadId":id})).await.unwrap();
    let node = &read["result"]["thread"]["turns"].as_array().unwrap().last().unwrap()["items"][0]["agentsTree"][0];
    assert_eq!(node["agent"], "Explore");
    assert_eq!(node["task"], "Inspect parser");
    assert_eq!(node["status"], "completed");
    assert_eq!(node["lastMessage"], "Found parser");
    let transcript = session.send_request_for_workspace("workspace", "subagent/transcript", json!({"threadId":id,"nodeId":node["id"]})).await.unwrap();
    assert_eq!(transcript["messages"].as_array().unwrap().len(), 2);
    assert_eq!(transcript["truncated"], true);
    assert!(transcript["note"].as_str().unwrap().contains("Task tool updates"));
    assert!(session.send_request_for_workspace("workspace", "subagent/transcript", json!({"threadId":id,"nodeId":"unrelated"})).await.is_err());
    session.shutdown().await;
    std::fs::remove_dir_all(data).unwrap();
}

#[test]
fn connectors_keep_transport_credentials_and_exact_lists() {
    let entries: Vec<McpServerEntry> = serde_json::from_value(json!([
        {"id":"stdio","enabled":true,"transport":{"type":"stdio","command":"node","args":["server.js"],"env":{"TOKEN":{"secret":true},"LITERAL":{"value":"${TAUCODE_SESSION_KEY}"}}},"toolAllowlist":["read"],"approval":"ask"},
        {"id":"http","enabled":true,"transport":{"type":"http","url":"https://example.test/mcp","headers":{"X-Key":{"secret":true}},"bearer":{"secret":true}},"approval":"ask"}
    ])).unwrap();
    let (servers, secrets) = config::mcp_servers(&entries, &Keys, "thread").unwrap();
    let stdio = servers
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "stdio")
        .unwrap();
    assert_eq!(stdio["command"], "node");
    assert!(stdio["env"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "TOKEN" && v["value"] == "fixture-api-secret"));
    assert!(stdio["env"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "LITERAL" && v["value"] == "${TAUCODE_SESSION_KEY}"));
    let http = servers
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "http")
        .unwrap();
    assert_eq!(http["type"], "http");
    assert!(http["headers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "Authorization" && v["value"] == "Bearer fixture-api-secret"));
    assert!(secrets.contains(&"fixture-api-secret".into()));
    assert_eq!(
        permissions::connector_decision(&entries, "mcp__stdio__write", "full-access"),
        permissions::Decision::Reject
    );
    assert_eq!(
        permissions::connector_decision(&entries, "mcp__stdio__read", "ask"),
        permissions::Decision::Ask
    );
}

#[tokio::test]
async fn prompt_errors_preserve_adapter_details_kind_and_redaction() {
    let data = temp();
    let engine = engine(&data);
    engine.configure("workspace", vec![provider("anthropic")], AgentPolicy::default()).await;
    let start = engine.request("workspace", "thread/start", json!({})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    let mut events = engine.events();
    for prompt in ["beta-error", "chunk-error", "result-error"] {
        engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":prompt}]})).await.unwrap();
        let done = event(&mut events, "turn/completed").await;
        assert_eq!(done["params"]["turn"]["status"], "failed");
        let error = &done["params"]["turn"]["error"];
        assert!(error["message"].as_str().unwrap().contains("API Error: 400 Routing and Anthropic beta override headers are disabled for Claude."));
        assert!(!done.to_string().contains("fixture-api-secret"));
        if prompt != "result-error" { assert_eq!(error["errorKind"], "provider_error"); }
    }
    let client = engine.runtimes.lock().await["workspace"].client.clone();
    let session = start["result"]["thread"]["sessionId"].clone();
    let error = client.request("session/prompt", json!({"sessionId":session,"prompt":[{"type":"text","text":"beta-error"}]})).await.unwrap_err();
    assert!(error.contains("Internal error: API Error: 400"));
    assert!(error.contains("provider_error"));
    assert!(!error.contains("fixture-api-secret"));
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(data).unwrap();
}

#[tokio::test]
async fn advertised_effort_is_clamped_and_applied_before_prompt() {
    let data = temp();
    let engine = engine(&data);
    engine.configure("workspace", vec![provider("anthropic")], AgentPolicy::default()).await;
    let mut events = engine.events();
    let started = engine.request("workspace", "thread/start", json!({})).await.unwrap();
    let id = started["result"]["thread"]["id"].as_str().unwrap();
    assert_eq!(engine.reasoning_options(id).await.unwrap().2.levels, ["low", "high"]);
    engine.request("workspace", "turn/start", json!({"threadId":id,"effort":"max","input":[{"type":"text","text":"reasoning-high"}]})).await.unwrap();
    assert_eq!(event(&mut events, "turn/completed").await["params"]["turn"]["status"], "completed");
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(data).unwrap();
}
