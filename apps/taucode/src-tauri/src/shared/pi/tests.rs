use super::*;
use crate::shared::providers::KeyStore;
use std::sync::Mutex as StdMutex;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pi-1.4.0")
            .join(name),
    )
    .unwrap()
}

#[tokio::test]
async fn chat_profile_is_isolated_and_forks_models_and_last_user_without_changing_parent() {
    let dir = temp();
    let mut engine = PiEngine::chat(dir.clone(), Arc::new(|_, _| {}));
    engine.fake = true;
    engine.keys = Arc::new(Keys);
    let providers: Vec<ProviderEntry> = ["first", "second"].iter().map(|id| serde_json::from_value(json!({"id":id,"name":id,"kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"fake","enabled":true}]})).unwrap()).collect();
    engine.configure("chats", providers.clone(), AgentPolicy::default()).await;
    let started = engine.request("chats", "thread/start", json!({"model":"first/fake","cwd":"/must-not-use"})).await.unwrap();
    let id = started["result"]["thread"]["id"].as_str().unwrap();
    let file = engine.directory("chats").unwrap().join(format!("{id}.jsonl"));
    assert!(file.starts_with(dir.join("chats")));
    assert!(!dir.join("pi/sessions").exists());
    let spawn: Value = serde_json::from_slice(&std::fs::read(file.with_extension("spawn")).unwrap()).unwrap();
    assert_eq!(spawn["cwd"], json!(dir.join("chats/scratch").join(id)));
    assert!(spawn["args"].as_array().unwrap().iter().any(|arg| arg == "--no-tools"));
    assert_eq!(std::fs::read_dir(dir.join("chats/scratch").join(id)).unwrap().count(), 0);
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join("chats/scratch").join(id)).unwrap().permissions().mode() & 0o777, 0o700);
    }
    engine.sessions.lock().await[id].process.as_ref().unwrap().shutdown().await.unwrap();
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new().append(true).open(&file).unwrap();
    for entry in [
        json!({"type":"model_change","id":"model","parentId":null,"provider":"taucode-first","modelId":"fake"}),
        json!({"type":"message","id":"u1","parentId":"model","message":{"role":"user","timestamp":1,"content":"First"}}),
        json!({"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","timestamp":2,"content":[{"type":"text","text":"Answer"}]}}),
        json!({"type":"message","id":"u2","parentId":"a1","message":{"role":"user","timestamp":3,"content":"Second"}}),
        json!({"type":"message","id":"a2","parentId":"u2","message":{"role":"assistant","timestamp":4,"content":[{"type":"text","text":"Last answer"}]}}),
    ] { writeln!(output, "{entry}").unwrap(); }
    drop(output);
    let original = std::fs::read(&file).unwrap();
    let fork = engine.request("chats", "thread/fork", json!({"threadId":id,"model":"second/fake","beforeUserItemId":format!("{id}:user:3")})).await.unwrap();
    let child = fork["result"]["thread"]["id"].as_str().unwrap();
    let child_file = engine.directory("chats").unwrap().join(format!("{child}.jsonl"));
    let (thread, history) = read_session(&child_file, &providers).unwrap();
    assert_eq!(thread["modelProvider"], "second");
    assert_eq!(thread["forkedFromId"], id);
    assert_eq!(history.turns.len(), 1);
    assert!(!json!(history.turns).to_string().contains("Second"));
    assert_ne!(fork["result"]["thread"]["cwd"], started["result"]["thread"]["cwd"]);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    let listed = engine.request("chats", "thread/list", json!({})).await.unwrap();
    assert_eq!(listed["result"]["data"].as_array().unwrap().iter().find(|t| t["id"] == child).unwrap()["modelProvider"], "second");
    engine.request("chats", "thread/resume", json!({"threadId":child})).await.unwrap();
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
fn replay(name: &str) -> (Translator, Vec<Value>) {
    let mut translator = Translator::new("recorded", "/tmp/taucode-fixture");
    let mut events = vec![];
    for line in fixture(name).split_terminator('\n') {
        let row: Value = serde_json::from_str(line).unwrap();
        if row["dir"] != "s2c" {
            continue;
        }
        let m = &row["msg"];
        events.extend(translator.apply(m));
    }
    (translator, events)
}
#[test]
fn recorded_translation_reconstructs_deltas_and_prompt_turns() {
    let (t, events) = replay("rpc_session.jsonl");
    assert_eq!(t.turns.len(), 6); // five prompts (including steer) plus manual compaction
    assert_eq!(t.turns[4]["status"], "interrupted");
    assert_eq!(t.turns[5]["status"], "completed");
    assert_eq!(t.model["id"], "Gemini 3.8 Flash (Account 1)");
    assert!(t.queue["steering"].is_array());
    let expected = [
        vec![
            "userMessage",
            "fileChange",
            "commandExecution",
            "agentMessage",
        ],
        vec![
            "userMessage",
            "reasoning",
            "commandExecution",
            "agentMessage",
        ],
        vec!["userMessage", "agentMessage"],
        vec!["userMessage", "agentMessage", "userMessage", "agentMessage"],
        vec!["userMessage", "agentMessage"],
        vec!["contextCompaction"],
    ];
    for (turn, expected) in t.turns.iter().zip(expected) {
        assert_eq!(
            turn["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert!(events
        .iter()
        .any(|e| e["method"] == "thread/settings/updated"
            && e["params"]["threadSettings"]["model"] == "Gemini 3.8 Flash (Account 1)"));
    assert!(events
        .iter()
        .filter(|e| e["method"] == "thread/tokenUsage/updated")
        .all(|e| e["params"]["tokenUsage"]["modelContextWindow"] == 128000));
    let kinds: Vec<_> = t.turns[0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "userMessage",
            "fileChange",
            "commandExecution",
            "agentMessage"
        ]
    );
    let mut deltas: HashMap<String, String> = HashMap::new();
    for event in &events {
        if matches!(
            event["method"].as_str(),
            Some("item/agentMessage/delta" | "item/commandExecution/outputDelta")
        ) {
            deltas
                .entry(event["params"]["itemId"].as_str().unwrap().into())
                .or_default()
                .push_str(event["params"]["delta"].as_str().unwrap());
        }
    }
    for turn in &t.turns {
        for item in turn["items"].as_array().unwrap() {
            if let Some(delta) = deltas.get(item["id"].as_str().unwrap()) {
                let final_text = match item["type"].as_str().unwrap() {
                    "reasoning" => item["content"][0].as_str(),
                    "commandExecution" => item["aggregatedOutput"].as_str(),
                    _ => item["text"].as_str(),
                };
                assert_eq!(Some(delta.as_str()), final_text);
            }
        }
    }
    assert!(t.total["totalTokens"].as_u64().unwrap() > 0);
    let (small, events) = replay("rpc_compact_too_small.jsonl");
    assert_eq!(small.turns.last().unwrap()["status"], "completed");
    assert!(!events.iter().any(|e| e["method"] == "error"));
    assert!(events.iter().any(|e| e["params"]["item"]["notice"] == "Nothing to compact yet"));
}
#[test]
fn recorded_history_equals_live_items_and_turns() {
    let (live, _) = replay("rpc_session.jsonl");
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pi-1.4.0/session_file.jsonl");
    let (thread, mut history) = read_session(&file, &[]).unwrap();
    assert_eq!(thread["engine"], "pi");
    // Normalize generated IDs only; all item content, statuses, ordering and grouping agree.
    fn normalize(turns: &mut [Value]) {
        for turn in turns {
            turn["id"] = Value::Null;
            for item in turn["items"].as_array_mut().unwrap() {
                item["id"] = Value::Null;
            }
        }
    }
    let mut turns = live.turns;
    normalize(&mut turns);
    normalize(&mut history.turns);
    assert_eq!(history.turns, turns);
}
#[test]
fn extension_ui_round_trip_and_login_rejection() {
    let request = fixture("rpc_session.jsonl")
        .split_terminator('\n')
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["msg"]["type"] == "extension_ui_request")
        .unwrap()["msg"]
        .clone();
    let canonical = translate::ui_request(&request, "thread", "turn");
    assert_eq!(canonical["method"], "item/tool/requestUserInput");
    let answer = json!({"answers":{"answer":{"answers":["Yes"]}}});
    assert_eq!(
        translate::ui_response(&request, &answer),
        json!({"type":"extension_ui_response","id":request["id"],"value":"Yes"})
    );
    assert_eq!(
        translate::ui_response(&json!({"id":"c","method":"confirm"}), &answer)["confirmed"],
        true
    );
    assert_eq!(
        translate::ui_response(&request, &json!({}))["cancelled"],
        true
    );
    assert!(translate::login_requested(
        &json!({"title":"Please /login to continue"})
    ));
}

#[tokio::test]
async fn gate_approvals_use_canonical_items_decisions_and_resolve() {
    let dir = temp();
    let (p, events) = fake(Duration::from_secs(600), &dir.join("eof")).await;
    p.receive(json!({"type":"agent_start"})).await;
    for (tool, kind) in [
        ("bash", "commandExecution"),
        ("write", "fileChange"),
        ("edit", "fileChange"),
    ] {
        let request = json!({"type":"extension_ui_request","id":tool,"method":"select","title":format!("taucode:approval:{}",json!({"toolCallId":format!("call-{tool}"),"tool":tool,"command":"echo oauth","path":".env","cwd":"/workspace"})),"options":["Allow","Allow for session","Block"]});
        let mapped = translate::ui_request(&request, "fake-thread", "turn");
        assert_eq!(mapped["method"], format!("item/{kind}/requestApproval"));
        assert_eq!(mapped["params"]["itemId"], format!("call-{tool}"));
        for (decision, expected) in [
            ("accept", "Allow"),
            ("acceptForSession", "Allow for session"),
            ("decline", "Block"),
            ("cancel", "Block"),
            ("unknown", "Block"),
        ] {
            assert_eq!(
                translate::ui_response(&request, &json!({"decision":decision}))["value"],
                expected
            );
        }
        p.receive(request).await;
        let id = format!("pi:fake-thread:{tool}");
        p.answer(&id, json!({"decision":"acceptForSession"}))
            .await
            .unwrap();
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e["method"] == "item/started"
            && e["params"]["item"]["id"] == format!("call-{tool}")
            && e["params"]["item"]["type"] == kind));
        assert!(events
            .iter()
            .any(|e| e["method"] == "serverRequest/resolved" && e["params"]["requestId"] == id));
    }
    p.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn subagent_validation_failure_keeps_error_and_single_agent_with_empty_modes() {
    let mut t = Translator::new("parent", "/tmp");
    t.apply(&json!({"type":"agent_start"}));
    t.apply(&json!({"type":"tool_execution_start","toolCallId":"spawn","toolName":"subagent",
        "args":{"agent":"colonel","task":"delegate","tasks":[],"chain":[],"model":null}}));
    let item = &t.turns[0]["items"][0];
    assert_eq!(item["prompt"], "delegate");
    assert_eq!(item["agentsTree"][0]["agent"], "colonel");
    assert_eq!(item["agentsStates"].as_object().unwrap().len(), 1);
    let events = t.apply(&json!({"type":"tool_execution_end","toolCallId":"spawn","toolName":"subagent",
        "isError":true,"result":{"content":[{"type":"text","text":"Delegation budget exhausted"}]}}));
    let item = &t.turns[0]["items"][0];
    assert_eq!(item["status"], "failed");
    assert_eq!(item["error"]["message"], "Delegation budget exhausted");
    assert_eq!(item["agentsTree"][0]["status"], "failed");
    assert!(events.iter().any(|e| e["method"] == "item/completed"
        && e["params"]["item"]["error"]["message"] == "Delegation budget exhausted"));
}

#[test]
fn recorded_subagents_stream_states_outputs_and_usage_without_double_counting() {
    let (mut t, events) = replay("rpc_subagents.jsonl");
    let item = t
        .turns
        .iter()
        .flat_map(|t| t["items"].as_array().unwrap())
        .find(|i| i["type"] == "collabAgentToolCall")
        .unwrap()
        .clone();
    assert_eq!(item["status"], "completed");
    let tree = item["agentsTree"].as_array().unwrap();
    assert_eq!(tree.len(), 2);
    for node in tree {
        assert_eq!(node["status"], "completed");
        assert!(node["startedAt"].is_i64());
        assert!(node["endedAt"].is_i64());
        assert!(node["lastMessage"].as_str().unwrap().chars().count() <= 300);
        assert!(t.transcripts[node["id"].as_str().unwrap()]["messages"].as_array().unwrap().len() > 0);
        assert!(node.get("messages").is_none());
    }
    let states = item["agentsStates"].as_object().unwrap();
    assert_eq!(states.len(), 2);
    assert!(states.values().all(|s| s["status"] == "completed"
        && s["model"] == "GLM 5.3"
        && s["message"].as_str().is_some_and(|s| !s.is_empty())));
    assert_eq!(
        states
            .values()
            .map(|s| s["usage"]["input"].as_u64().unwrap())
            .sum::<u64>(),
        3481
    );
    assert!(events.iter().any(|e| e["method"] == "item/started"
        && e["params"]["item"]["agentsStates"]
            .as_object()
            .is_some_and(|s| s.values().any(
                |s| s["status"] == "running" && s["usage"]["turns"].as_u64().unwrap_or(0) > 0
            ))));
    let last: Value = fixture("rpc_subagents.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["msg"]["type"] == "tool_execution_end" && r["msg"]["toolName"] == "subagent")
        .unwrap()["msg"]
        .clone();
    let total = t.total.clone();
    t.apply(&last);
    assert_eq!(t.total, total);
    let mut paid = last;
    paid["result"]["details"]["results"][0]["usage"]["cost"] = json!(0.125);
    let before = t.cost;
    let updated = t.apply(&paid);
    assert_eq!(t.cost, before + 0.125);
    assert!(updated
        .iter()
        .any(|e| e["method"] == "thread/tokenUsage/updated"
            && e["params"]["tokenUsage"]["total"]["cost"] == t.cost));
}

#[test]
fn nested_subagent_snapshots_keep_ids_timestamps_and_aggregate_usage() {
    let (mut t, _) = replay("rpc_subagents.jsonl");
    let mut frame: Value = fixture("rpc_subagents.jsonl").lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["msg"]["type"] == "tool_execution_end" && r["msg"]["toolName"] == "subagent").unwrap()["msg"].clone();
    let parent_id = format!("{}:agent:0", frame["toolCallId"].as_str().unwrap());
    let started = t.turns[0]["items"].as_array().unwrap().iter().find(|i|i["id"] == frame["toolCallId"]).unwrap()["agentsTree"][0]["startedAt"].clone();
    frame["result"]["details"]["results"][0]["messages"].as_array_mut().unwrap().push(json!({
        "role":"toolResult","toolCallId":"nested","toolName":"subagent","details":{"results":[{
            "agent":"reviewer","status":"failed","stderr":"fixture error","usage":{"input":17,"output":3,"cost":0.25},
            "messages":[{"role":"assistant","content":[{"type":"text","text":"Nested transcript"}]}]
        }]}
    }));
    let before = t.cost;
    t.apply(&frame);
    let node = t.turns[0]["items"].as_array().unwrap().iter().find(|i|i["id"] == frame["toolCallId"]).unwrap()["agentsTree"][0].clone();
    assert_eq!(node["id"], parent_id);
    assert_eq!(node["startedAt"], started);
    assert_eq!(node["usage"]["input"], frame["result"]["details"]["results"][0]["usage"]["input"].as_u64().unwrap() + 17);
    assert_eq!(node["children"][0]["id"], format!("{parent_id}/nested:agent:0"));
    assert_eq!(node["children"][0]["status"], "failed");
    assert_eq!(node["children"][0]["errorMessage"], "fixture error");
    assert_eq!(t.cost, before + 0.25);
    let total = t.total.clone();
    t.apply(&frame);
    assert_eq!(t.total, total);
    let retained = &t.transcripts[&format!("{parent_id}/nested:agent:0")];
    assert_eq!(retained["messages"][0]["content"][0]["text"], "Nested transcript");
    assert!(!t.transcripts[&parent_id].to_string().contains("Nested transcript"));
}

#[test]
fn transcript_caps_keep_the_latest_messages_and_flag_truncation() {
    let capped = agents::capped((0..510).map(|i|json!({"id":i,"content":"text"})));
    assert_eq!(capped["messages"].as_array().unwrap().len(), 500);
    assert_eq!(capped["messages"][0]["id"], 10);
    assert_eq!(capped["truncated"], true);
    let capped = agents::capped([json!({"content":"x".repeat(2 * 1024 * 1024)}), json!({"content":"last"})]);
    assert_eq!(capped["messages"].as_array().unwrap().len(), 1);
    assert_eq!(capped["messages"][0]["content"], "last");
    assert_eq!(capped["truncated"], true);
    assert_eq!(agents::capped([json!({"content":"small"})])["truncated"], false);
}

#[tokio::test]
async fn transcript_command_restores_capped_messages_from_session_file() {
    let dir = temp();
    let engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into_owned(), None, Arc::new(|_, _| {}));
    let sessions = engine.directory("workspace").unwrap();
    std::fs::create_dir_all(&sessions).unwrap();
    let entries = [json!({"type":"session","version":3,"id":"parent","cwd":dir}),
        json!({"type":"message","id":"u","parentId":null,"message":{"role":"user","content":"delegate"}}),
        json!({"type":"message","id":"a","parentId":"u","message":{"role":"assistant","content":[{"type":"toolCall","id":"spawn","name":"subagent","arguments":{"agent":"reviewer","task":"review"}}],"stopReason":"toolUse"}}),
        json!({"type":"message","id":"r","parentId":"a","message":{"role":"toolResult","toolCallId":"spawn","toolName":"subagent","details":{"results":[{"agent":"reviewer","status":"completed","messages":(0..505).map(|n|json!({"role":"assistant","content":[{"type":"text","text":format!("message {n}")}]})).collect::<Vec<_>>()}]}}})];
    std::fs::write(sessions.join("parent.jsonl"), entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n").unwrap();
    let transcript = engine.request("workspace", "subagent/transcript", json!({"threadId":"parent","nodeId":"spawn:agent:0"})).await.unwrap();
    assert_eq!(transcript["truncated"], true);
    assert_eq!(transcript["messages"].as_array().unwrap().len(), 500);
    assert_eq!(transcript["messages"][0]["content"][0]["text"], "message 5");
    assert!(engine.request("workspace", "subagent/transcript", json!({"threadId":"parent","nodeId":"missing"})).await.is_err());
    assert!(engine.request("other", "subagent/transcript", json!({"threadId":"parent","nodeId":"spawn:agent:0"})).await.is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn side_chats_copy_latest_or_message_and_persist_read_only_links() {
    let dir = temp();
    let engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into_owned(), None, Arc::new(|_, _| {}));
    let sessions = engine.directory("workspace").unwrap();
    std::fs::create_dir_all(&sessions).unwrap();
    let entries = [json!({"type":"session","version":3,"id":"parent","cwd":dir}),
        json!({"type":"message","id":"u1","parentId":null,"message":{"role":"user","content":"first","timestamp":1}}),
        json!({"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","content":[{"type":"text","text":"answer one"}],"timestamp":2,"stopReason":"stop"}}),
        json!({"type":"message","id":"u2","parentId":"a1","message":{"role":"user","content":"second","timestamp":3}}),
        json!({"type":"message","id":"a2","parentId":"u2","message":{"role":"assistant","content":[{"type":"text","text":"answer two"}],"timestamp":4,"stopReason":"stop"}})];
    let source = entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(sessions.join("parent.jsonl"), &source).unwrap();
    for at in [Value::Null, json!("parent:message:2")] {
        let side_of = json!({"threadId":"parent","workspaceId":"workspace","atItemId":at});
        let fork = engine.request("workspace", "thread/fork", json!({"threadId":"parent","sideOf":side_of,"atItemId":at})).await.unwrap();
        let id = fork["result"]["thread"]["id"].as_str().unwrap();
        assert_eq!(fork["result"]["sandbox"]["type"], "readOnly");
        let (thread, _) = read_session(&sessions.join(format!("{id}.jsonl")), &[]).unwrap();
        assert_eq!(thread["sideOf"], side_of);
        assert_eq!(thread["forkedFromId"], "parent");
        assert_eq!(thread["turns"].as_array().unwrap().len(), if at.is_null() { 2 } else { 1 });
        let access: Value = serde_json::from_slice(&std::fs::read(dir.join("pi/agent/workspace/taucode/threads").join(format!("{id}.json"))).unwrap()).unwrap();
        assert_eq!(access["accessMode"], "read-only");
    }
    assert_eq!(std::fs::read_to_string(sessions.join("parent.jsonl")).unwrap(), source);
    let bad = engine.request("workspace", "thread/fork", json!({"threadId":"parent","sideOf":{"threadId":"parent"},"atItemId":"missing"})).await;
    assert!(bad.is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn connector_items_names_and_every_call_approvals() {
    let mut t = Translator::new("thread", "/workspace")
        .with_connectors(HashMap::from([("my_server".into(), "My server".into())]));
    t.apply(&json!({"type":"agent_start"}));
    let events = t.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":"mcp__my_server__read","args":{}}));
    let item = &events
        .iter()
        .find(|e| e["method"] == "item/started")
        .unwrap()["params"]["item"];
    assert_eq!(item["type"], "mcpToolCall");
    assert_eq!(item["server"], "My server");
    assert_eq!(item["tool"], "read");
    let request = json!({"type":"extension_ui_request","id":"approval","method":"select","title":format!("taucode:approval:{}",json!({"toolCallId":"call","tool":"mcp__my_server__read","command":"mcp__my_server__read","cwd":"/workspace"})),"options":["Allow","Block"]});
    let mapped = translate::ui_request(&request, "thread", "turn");
    assert_eq!(mapped["method"], "item/commandExecution/requestApproval");
    assert_eq!(
        mapped["params"]["availableDecisions"],
        json!(["accept", "decline", "cancel"])
    );
    assert_eq!(
        translate::ui_response(&request, &json!({"decision":"accept"}))["value"],
        "Allow"
    );
}

#[test]
fn virtual_connector_writes_and_edits_request_tool_approval() {
    for tool in ["write", "edit"] {
        for (name, label) in [
            ("taucode_browser_browser_navigate", "taucode_browser · browser_navigate"),
            ("my_server__read", "my_server · read"),
            ("ambiguous_server_tool", "ambiguous_server_tool"),
            ("a_a_b_b", "a_a_b_b"),
        ] {
            let request = json!({"id":"approval","method":"select","title":format!("taucode:approval:{}",json!({"toolCallId":"call","tool":tool,"path":format!("xd://mcp__{name}"),"cwd":"/workspace"}))});
            let mapped = translate::ui_request(&request, "thread", "turn");
            assert_eq!(mapped["method"], "item/commandExecution/requestApproval");
            assert_eq!(mapped["params"]["reason"], format!("Connector requests permission to call {label}"));
            assert_eq!(mapped["params"]["tool"], format!("mcp__{name}"));
            assert_eq!(mapped["params"]["availableDecisions"], json!(["accept", "decline", "cancel"]));
        }
    }
}

#[test]
fn virtual_connectors_keep_tool_items_results_and_replay_without_file_diffs() {
    for tool in ["write", "edit"] {
        let args = json!({"path":"xd://mcp__taucode_browser_browser_navigate","content":"{\"url\":\"https://example.com\"}"});
        let mut t = Translator::new("thread", "/workspace")
            .with_connectors(HashMap::from([("taucode_browser".into(), "Browser".into())]));
        t.apply(&json!({"type":"agent_start"}));
        let started = t.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":tool,"args":args}));
        let item = &started[0]["params"]["item"];
        assert_eq!(item["type"], "mcpToolCall");
        assert_eq!(item["server"], "Browser");
        assert_eq!(item["tool"], "browser_navigate");
        assert_eq!(item["arguments"]["url"], "https://example.com");
        let result = json!({"content":[{"type":"text","text":"Opened example.com"}]});
        let completed = t.apply(&json!({"type":"tool_execution_end","toolCallId":"call","toolName":tool,"result":result}));
        assert!(!completed.iter().any(|e| e["method"] == "turn/diff/updated"));
        assert_eq!(t.turns[0]["items"][0]["result"]["content"], result["content"]);
        let history = Translator::history("thread", "/workspace", &[
            json!({"role":"user","timestamp":1,"content":"Browse"}),
            json!({"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"call","name":tool,"arguments":args}]}),
            json!({"role":"toolResult","toolCallId":"call","toolName":tool,"content":result["content"]}),
        ]).with_connectors(HashMap::from([("taucode_browser".into(), "Browser".into())]));
        assert_eq!(history.turns[0]["items"][1], t.turns[0]["items"][0]);
    }
}

#[test]
fn virtual_mutating_tools_are_not_file_changes_in_live_or_replayed_transcripts() {
    for (name, expected) in [("subagent", "collabAgentToolCall"), ("custom_mutation", "dynamicToolCall")] {
        for transport in ["write", "edit"] {
            let input = json!({"task":"Inspect the project","agent":"scout"});
            let args = json!({"path":format!("xd://{name}"),"content":input.to_string()});
            let mut translator = Translator::new("thread", "/workspace");
            translator.apply(&json!({"type":"agent_start"}));
            let started = translator.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":transport,"args":args}));
            assert_eq!(started[0]["params"]["item"]["type"], expected);
            if name == "subagent" { assert_eq!(started[0]["params"]["item"]["prompt"], "Inspect the project"); }
            let result = json!({"content":[{"type":"text","text":"Tool call failed"}],"isError":true});
            let completed = translator.apply(&json!({"type":"tool_execution_end","toolCallId":"call","toolName":transport,"result":result,"isError":true}));
            assert!(!completed.iter().any(|event| event["method"] == "turn/diff/updated"));
            assert_eq!(translator.turns[0]["items"][0]["status"], "failed");
            assert!(translator.turns[0]["items"][0].get("changes").is_none());
            if name == "subagent" { assert_eq!(translator.turns[0]["items"][0]["agentsTree"][0]["status"], "failed"); }
            let history = Translator::history("thread", "/workspace", &[
                json!({"role":"user","timestamp":1,"content":"Inspect"}),
                json!({"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"call","name":transport,"arguments":args}]}),
                json!({"role":"toolResult","toolCallId":"call","toolName":transport,"content":result["content"],"isError":true}),
            ]);
            assert_eq!(history.turns[0]["items"][1]["type"], expected);
            assert_eq!(history.turns[0]["items"][1]["status"], "failed");
        }
    }
}

#[tokio::test]
async fn native_tool_gate_approvals_are_decisions_instead_of_user_questions() {
    let dir = temp();
    let (p, events) = fake(Duration::from_secs(600), &dir.join("eof")).await;
    p.receive(json!({"type":"agent_start"})).await;
    for tool in ["eval", "lsp", "debug"] {
        let request = json!({"type":"extension_ui_request","id":tool,"method":"select","title":format!("taucode:approval:{}",json!({"toolCallId":tool,"tool":tool,"command":tool,"cwd":"/workspace"})),"options":["Allow","Block"]});
        let mapped = translate::ui_request(&request, "fake-thread", "turn");
        assert_eq!(mapped["method"], "item/commandExecution/requestApproval");
        assert_eq!(mapped["params"]["reason"], format!("Pi requests permission to use {tool}"));
        assert_eq!(mapped["params"]["availableDecisions"], json!(["accept", "decline", "cancel"]));
        p.receive(request).await;
        p.answer(&format!("pi:fake-thread:{tool}"), json!({"decision":"decline"})).await.unwrap();
    }
    assert!(!events.lock().unwrap().iter().any(|e| e["method"] == "item/tool/requestUserInput"));
    p.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn managed_policy_and_access_files_update_and_preserve_session_rules() {
    let dir = temp();
    let home = dir.join("pi/agent/workspace");
    let mut policy = AgentPolicy::default();
    config::write_policy(&home, &[], &policy).unwrap();
    let read =
        |path: &Path| serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(read(&home.join("taucode/policy.json"))["maxConcurrent"], 4);
    policy.subagents.max_concurrent = 2;
    config::write_policy(&home, &[], &policy).unwrap();
    assert_eq!(read(&home.join("taucode/policy.json"))["maxConcurrent"], 2);
    policy.subagents.mode = crate::types::SubagentMode::ChainOfCommand;
    config::write_policy(&home, &[], &policy).unwrap();
    assert_eq!(read(&home.join("taucode/policy.json"))["mode"], "chain-of-command");
    config::write_access(&home, "thread", Some("current")).unwrap();
    let file = home.join("taucode/threads/thread.json");
    assert_eq!(read(&file)["accessMode"], "ask");
    config::write_json(
        &file,
        &json!({"accessMode":"ask","sessionAllow":[{"tool":"write","path":"/workspace/file"}]}),
    )
    .unwrap();
    config::write_access(&home, "thread", Some("read-only")).unwrap();
    assert_eq!(read(&file)["sessionAllow"].as_array().unwrap().len(), 1);
    assert_eq!(read(&file)["accessMode"], "read-only");
    config::write_access(&home, "thread", Some("current")).unwrap();
    assert_eq!(read(&file)["accessMode"], "read-only");
    assert!(config::write_access(&home, "../escape", None).is_err());
    assert!(config::write_access(&home, "thread", Some("invalid")).is_err());
    let workspace: crate::types::WorkspaceEntry = serde_json::from_value(json!({"id":"workspace","name":"Project","path":dir,"settings":{"agentPolicy":{"subagents":{"maxConcurrent":7}}}})).unwrap();
    crate::storage::write_workspaces(&dir.join("workspaces.json"), &[workspace]).unwrap();
    let mut settings = crate::types::AppSettings::default();
    settings.agent_policy.subagents.max_per_turn = 12;
    config::sync_policies(&dir, &settings).unwrap();
    let merged = read(&home.join("taucode/policy.json"));
    assert_eq!(merged["maxConcurrent"], 7);
    assert_eq!(merged["maxPerTurn"], 12);
    std::fs::remove_dir_all(dir).unwrap();
}
struct Keys;
impl KeyStore for Keys {
    fn get(&self, _: &str) -> Result<Option<String>, String> {
        Ok(Some("keychain-secret-canary".into()))
    }
    fn set(&self, _: &str, _: &str) -> Result<(), String> {
        panic!("must not write credentials")
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        panic!("must not delete credentials")
    }
}
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("tau-pi-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn every_provider_and_auth_config_is_secret_free() {
    let dir = temp();
    for kind in [
        "openai",
        "anthropic",
        "google",
        "openrouter",
        "groq",
        "mistral",
        "xai",
        "deepseek",
        "zai",
        "ollama",
        "lmstudio",
        "openai-compatible",
    ] {
        for auth in [
            json!({"type":"none"}),
            json!({"type":"apiKey"}),
            json!({"type":"env","var":"PI_TEST_KEY"}),
            json!({"type":"command","command":"/path with spaces/key","args":["a'b","$(never)"]}),
        ] {
            let p:ProviderEntry=serde_json::from_value(json!({"id":"local","name":"Test","kind":kind,"enabled":true,"auth":auth,"models":[{"id":"fake","enabled":true}]})).unwrap();
            let registry = vec![p];
            let models = config::models(&registry);
            let id = config::provider_id(&registry[0], &registry);
            assert_eq!(models["providers"][&id]["models"][0]["id"], "fake");
            let mut env = config::prepare(&dir, &registry, None).unwrap();
            env.extend(config::credentials(&registry, &Keys).unwrap());
            if auth["type"] == "apiKey" {
                assert_eq!(env["TAUCODE_PROVIDER_LOCAL_KEY"], "keychain-secret-canary");
                assert_eq!(
                    models["providers"][&id]["apiKey"],
                    "$TAUCODE_PROVIDER_LOCAL_KEY"
                );
            }
            if auth["type"] == "command" {
                assert_eq!(models["providers"][&id]["apiKey"], "$TAUCODE_PROVIDER_LOCAL_KEY");
            }
            for name in ["models.json", "mcp.json", "settings.json"] {
                assert!(!std::fs::read_to_string(dir.join(name))
                    .unwrap()
                    .contains("keychain-secret-canary"));
            }
            assert!(!dir.join("auth.json").exists());
        }
    }
    let graph = CodegraphSetup {
        proxy_exe: "/taucode".into(),
        upstream: vec!["codegraph".into(), "serve".into(), "--mcp".into()],
        state_dir: dir.join("graph"),
    };
    let mcp = config::mcp(Some(&graph));
    assert_eq!(
        mcp["mcpServers"]["codegraph"]["args"],
        json!(["--tau-mcp-proxy", "--", "codegraph", "serve", "--mcp"])
    );
    assert_eq!(
        mcp["mcpServers"]["codegraph"]["env"]["TAUCODE_SESSION_KEY"],
        "${TAUCODE_SESSION_KEY}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
async fn fake(idle: Duration, marker: &Path) -> (Arc<Process>, Arc<StdMutex<Vec<Value>>>) {
    let mut command = Command::new("python3");
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_pi.py"))
        .arg("--session")
        .arg("session.jsonl")
        .env("FAKE_PI_EOF", marker)
        .env("TAUCODE_SESSION_KEY", "fake-thread");
    let events = Arc::new(StdMutex::new(vec![]));
    let sink = events.clone();
    let p = Process::spawn(
        command,
        Translator::new("fake-thread", "/workspace"),
        Arc::new(move |e| sink.lock().unwrap().push(e)),
        None,
        idle,
        vec![],
    )
    .await
    .unwrap();
    (p, events)
}
#[tokio::test]
async fn fake_correlates_ids_frames_unicode_and_settles_before_abort_response() {
    let dir = temp();
    let marker = dir.join("eof");
    let (p, events) = fake(Duration::from_secs(600), &marker).await;
    let held = p.clone();
    let request = tokio::spawn(async move { held.request(json!({"type":"hold"})).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    p.request(json!({"type":"release"})).await.unwrap();
    request.await.unwrap().unwrap();
    p.request(json!({"type":"prompt","message":"hello"}))
        .await
        .unwrap();
    p.request(json!({"type":"abort"})).await.unwrap();
    let recorded = events.lock().unwrap().clone();
    assert!(recorded
        .iter()
        .any(|e| e["params"]["delta"] == "one\u{2028}two\u{2029}three"));
    assert!(
        recorded
            .iter()
            .any(|e| e["method"] == "turn/completed"
                && e["params"]["turn"]["status"] == "interrupted")
    );
    p.shutdown().await.unwrap();
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "graceful");
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn fake_crash_fails_turn_and_idle_closes_stdin() {
    let dir = temp();
    let (p, events) = fake(Duration::from_secs(600), &dir.join("crash")).await;
    p.request(json!({"type":"prompt","message":"crash"}))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while p.alive.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e["method"] == "turn/completed" && e["params"]["turn"]["status"] == "failed"));
    let marker = dir.join("idle");
    let (p, _) = fake(Duration::from_millis(50), &marker).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while p.alive.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "graceful");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn interrupt_cancels_pending_ui_before_waiting_for_pi_to_settle() {
    let dir = temp();
    let (p, events) = fake(Duration::from_secs(600), &dir.join("eof")).await;
    p.request(json!({"type":"prompt","message":"approval"})).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !events.lock().unwrap().iter().any(|e| e["method"] == "item/tool/requestUserInput") {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), p.request(json!({"type":"abort"}))).await.unwrap().unwrap();
    assert!(events.lock().unwrap().iter().any(|e| e["method"] == "serverRequest/resolved"));
    assert!(p.answer("pi:fake-thread:blocked", json!({"decision":"accept"})).await.is_err());
    p.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn questions_expire_and_login_requests_never_reach_the_ui() {
    let dir = temp();
    let (p, events) = fake(Duration::from_secs(600), &dir.join("eof")).await;
    p.receive(json!({"type":"extension_ui_request","id":"choice","method":"input","title":"Name","timeout":10})).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if events
                .lock()
                .unwrap()
                .iter()
                .any(|e| e["method"] == "serverRequest/resolved")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(p.answer("pi:fake-thread:choice", json!({})).await.is_err());
    p.receive(json!({"type":"extension_ui_request","id":"login","method":"input","title":"Please /login"})).await;
    assert!(!events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e["id"] == "pi:fake-thread:login"));
    assert!(events.lock().unwrap().iter().any(|e| e["method"] == "error"
        && e["params"]["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Accounts and logins")));
    assert!(!translate::login_requested(
        &json!({"title":"Allow edit of login.ts?"})
    ));
    p.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tool_mapping_handles_edits_mcp_dynamic_calls_and_exit_codes() {
    for (name, args, kind) in [
        (
            "edit",
            json!({"path":"a.txt","edits":[{"oldText":"before","newText":"after"}]}),
            "fileChange",
        ),
        (
            "mcp__codegraph__explore",
            json!({"query":"main"}),
            "mcpToolCall",
        ),
        ("read", json!({"path":"a.txt"}), "dynamicToolCall"),
        ("bash", json!({"command":"exit 42"}), "commandExecution"),
    ] {
        let mut t = Translator::new("test", "/workspace");
        t.apply(&json!({"type":"agent_start"}));
        t.apply(
            &json!({"type":"tool_execution_start","toolCallId":"tool","toolName":name,"args":args}),
        );
        if name == "bash" {
            let a=t.apply(&json!({"type":"tool_execution_update","toolCallId":"tool","partialResult":{"content":[{"type":"text","text":"one"}]}}));
            let b=t.apply(&json!({"type":"tool_execution_update","toolCallId":"tool","partialResult":{"content":[{"type":"text","text":"onetwo"}]}}));
            assert_eq!(a[0]["params"]["delta"], "one");
            assert_eq!(b[0]["params"]["delta"], "two");
        }
        t.apply(&json!({"type":"tool_execution_end","toolCallId":"tool","result":{"content":[{"type":"text","text":"onetwo"}],"structuredContent":{"exit_code":42}},"isError":name=="bash"}));
        let item = &t.turns[0]["items"][0];
        assert_eq!(item["type"], kind);
        match name {
            "edit" => assert!(item["changes"][0]["diff"]
                .as_str()
                .unwrap()
                .contains("-before\n+after")),
            "bash" => {
                assert_eq!(item["exitCode"], 42);
                assert_eq!(item["status"], "failed");
            }
            "read" => assert_eq!(item["success"], true),
            _ => assert_eq!(item["server"], "codegraph"),
        }
    }
}

#[test]
fn repeated_writes_are_updates_with_the_previous_content_in_the_diff() {
    let mut t = Translator::new("test", "/workspace");
    for (id, content) in [("first", "before\n"), ("second", "after\n")] {
        t.apply(&json!({"type":"agent_start"}));
        t.apply(&json!({"type":"tool_execution_start","toolCallId":id,"toolName":"write","args":{"path":"note.txt","content":content}}));
        t.apply(&json!({"type":"tool_execution_end","toolCallId":id,"result":{"content":[]},"isError":false}));
        t.apply(&json!({"type":"agent_settled","aborted":false}));
    }
    assert_eq!(t.turns[0]["items"][0]["changes"][0]["kind"]["type"], "add");
    let change = &t.turns[1]["items"][0]["changes"][0];
    assert_eq!(change["kind"]["type"], "update");
    assert!(change["diff"]
        .as_str()
        .unwrap()
        .contains("-before\n+after\n"));
}

#[tokio::test]
async fn engine_switches_in_place_clones_and_respawns_after_idle() {
    let dir = temp();
    let mut engine = PiEngine::new(
        dir.clone(),
        "workspace".into(),
        dir.to_string_lossy().into_owned(),
        None,
        Arc::new(|_, _| {}),
    );
    engine.fake = true;
    engine.keys = Arc::new(Keys);
    engine.idle = Duration::from_millis(60);
    let providers:Vec<ProviderEntry>=["first","second"].iter().map(|id|serde_json::from_value(json!({"id":id,"name":id,"kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"fake","enabled":true}]})).unwrap()).collect();
    engine
        .configure("workspace", providers, AgentPolicy::default())
        .await;
    let started = engine
        .request("workspace", "thread/start", json!({"model":"first/fake"}))
        .await
        .unwrap();
    let id = started["result"]["thread"]["id"].as_str().unwrap();
    let original = engine.sessions.lock().await[id].process.clone().unwrap();
    assert_eq!(started["result"]["sandbox"]["type"], "workspaceWrite");
    engine
        .request(
            "workspace",
            "turn/start",
            json!({"threadId":id,"model":"second/fake","input":[{"type":"text","text":"hello"}]}),
        )
        .await
        .unwrap();
    engine
        .request(
            "workspace",
            "thread/accessMode/set",
            json!({"threadId":id,"accessMode":"read-only"}),
        )
        .await
        .unwrap();
    let access: Value = serde_json::from_slice(
        &std::fs::read(
            dir.join("pi/agent/workspace/taucode/threads")
                .join(format!("{id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(access["accessMode"], "read-only");
    assert_eq!(original.translator.lock().await.access_mode, "read-only");
    engine
        .request("workspace", "turn/interrupt", json!({"threadId":id}))
        .await
        .unwrap();
    let file = engine.sessions.lock().await[id].file.clone();
    let requests: Vec<Value> = std::fs::read_to_string(file.with_extension("requests"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let commands: Vec<_> = requests
        .iter()
        .map(|r| r["type"].as_str().unwrap())
        .collect();
    assert_eq!(commands, vec!["get_state", "set_model", "prompt", "abort"]);
    assert_eq!(
        engine.sessions.lock().await[id].thread["modelProvider"],
        "second"
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while original.alive.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    engine
        .request("workspace", "thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&original, engine.sessions.lock().await[id].process.as_ref().unwrap()));
    engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"after resume"}]})).await.unwrap();
    let next = engine.sessions.lock().await[id].process.clone().unwrap();
    assert!(!Arc::ptr_eq(&original, &next));
    assert_eq!(engine.sessions.lock().await[id].file, file);
    engine.request("workspace", "turn/interrupt", json!({"threadId":id})).await.unwrap();
    let fork = engine
        .request(
            "workspace",
            "thread/fork",
            json!({"threadId":id,"model":"first/fake"}),
        )
        .await
        .unwrap();
    assert_ne!(fork["result"]["thread"]["id"], id);
    assert_eq!(fork["result"]["thread"]["modelProvider"], "first");
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn no_accounts_guard_blocks_credential_storage_without_affecting_other_files() {
    let dir = temp();
    let script = r#"
        import assert from 'node:assert/strict';
        import {readFileSync, writeFileSync, existsSync} from 'node:fs';
        import {join} from 'node:path';
        const auth=join(process.env.PI_CODING_AGENT_DIR,'auth.json');
        writeFileSync(auth,'{}');
        assert.equal(readFileSync(auth,'utf8'),'{}');
        assert.equal(existsSync(auth),false);
        assert.throws(()=>writeFileSync(auth,JSON.stringify({openai:{type:'api_key',key:'canary'}})),/disables accounts/);
        assert.equal(existsSync(auth),false);
        const ordinary=join(process.env.PI_CODING_AGENT_DIR,'ordinary.txt');
        writeFileSync(ordinary,'normal');
        assert.equal(readFileSync(ordinary,'utf8'),'normal');
    "#;
    let result = std::process::Command::new("node")
        .arg("--import")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/pi/no-accounts.mjs"))
        .args(["--input-type=module", "--eval", script])
        .env("PI_CODING_AGENT_DIR", &dir)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!dir.join("auth.json").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn real_pi_offline_startup_preserves_isolation_and_session_identity() {
    let dir = temp();
    let engine = PiEngine::new(
        dir.clone(),
        "workspace".into(),
        dir.to_string_lossy().into_owned(),
        None,
        Arc::new(|_, _| {}),
    );
    let provider:ProviderEntry=serde_json::from_value(json!({"id":"local","name":"Offline","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"taucode-offline-test","enabled":true}]})).unwrap();
    engine
        .configure("workspace", vec![provider], AgentPolicy::default())
        .await;
    // get_state only; no prompt, provider connection or credential lookup. The
    // test build explicitly sets PI_OFFLINE=1 on the child.
    let response = engine
        .request(
            "workspace",
            "thread/start",
            json!({"model":"local/taucode-offline-test"}),
        )
        .await
        .unwrap();
    let id = response["result"]["thread"]["id"].as_str().unwrap();
    assert_eq!(response["result"]["thread"]["engine"], "pi");
    let p = engine.sessions.lock().await[id].process.clone().unwrap();
    p.shutdown().await.unwrap();
    engine
        .request("workspace", "thread/resume", json!({"threadId":id}))
        .await
        .unwrap();
    engine.shutdown().await.unwrap();
    assert!(!dir.join("pi/agent/auth.json").exists());
    assert!(dir
        .join("pi/sessions/workspace")
        .join(format!("{id}.jsonl"))
        .exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn real_pi_chat_starts_offline_with_only_the_gate() {
    let dir = temp();
    let engine = PiEngine::chat(dir.clone(), Arc::new(|_, _| {}));
    let provider: ProviderEntry = serde_json::from_value(json!({"id":"local","name":"Offline","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"taucode-offline-test","enabled":true}]})).unwrap();
    engine.configure("chats", vec![provider], AgentPolicy::default()).await;
    let response = engine.request("chats", "thread/start", json!({})).await.unwrap();
    let id = response["result"]["thread"]["id"].as_str().unwrap();
    let process = engine.sessions.lock().await[id].process.clone().unwrap();
    assert_eq!(*process.extensions.lock().await, std::collections::HashSet::from(["taucode-gate".to_owned()]));
    assert!(dir.join("chats/scratch").join(id).is_dir());
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn recorded_compaction_resets_only_successful_thread_epoch() {
    for (fixture_name, expected) in [("rpc_session.jsonl", 1), ("rpc_compact_too_small.jsonl", 0)] {
        let dir = temp();
        let mut command = Command::new("python3");
        command.arg("-c").arg("import sys; sys.stdin.read()");
        let graph = CodegraphSetup {
            proxy_exe: "taucode".into(),
            upstream: vec![],
            state_dir: dir.clone(),
        };
        let p = Process::spawn(
            command,
            Translator::new("recorded", "/tmp/taucode-fixture"),
            Arc::new(|_| {}),
            Some(graph),
            Duration::from_secs(600),
            vec![],
        )
        .await
        .unwrap();
        for row in fixture(fixture_name)
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|r| {
                r["dir"] == "s2c"
                    && matches!(
                        r["msg"]["type"].as_str(),
                        Some("compaction_start" | "compaction_end")
                    )
            })
        {
            p.receive(row["msg"].clone()).await;
        }
        assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "recorded"), expected);
        assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "other"), 0);
        p.shutdown().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn disk_history_pages_turns_and_items_without_starting_pi() {
    let dir = temp();
    let sessions = dir.join("pi/sessions/workspace");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(
        sessions.join("recorded.jsonl"),
        fixture("session_file.jsonl"),
    )
    .unwrap();
    let engine = PiEngine::new(
        dir.clone(),
        "workspace".into(),
        "/tmp/taucode-fixture".into(),
        None,
        Arc::new(|_, _| {}),
    );
    let list = engine
        .request("workspace", "thread/list", json!({}))
        .await
        .unwrap();
    let id = list["result"]["data"][0]["id"].as_str().unwrap();
    assert!(engine.owns("workspace", id).await);
    let page = engine
        .request(
            "workspace",
            "thread/turns/list",
            json!({"threadId":id,"limit":2,"sortDirection":"asc","itemsView":"notLoaded"}),
        )
        .await
        .unwrap();
    assert_eq!(page["result"]["data"].as_array().unwrap().len(), 2);
    assert_eq!(page["result"]["nextCursor"], "2");
    assert_eq!(page["result"]["data"][0]["items"], json!([]));
    let turn = &page["result"]["data"][0]["id"];
    let items = engine
        .request(
            "workspace",
            "thread/items/list",
            json!({"threadId":id,"turnId":turn,"limit":2,"sortDirection":"asc"}),
        )
        .await
        .unwrap();
    assert_eq!(items["result"]["data"][0]["item"]["type"], "userMessage");
    assert_eq!(items["result"]["nextCursor"], "2");
    let rest = engine
        .request(
            "workspace",
            "thread/items/list",
            json!({"threadId":id,"turnId":turn,"cursor":"2","limit":2}),
        )
        .await
        .unwrap();
    assert_eq!(
        rest["result"]["data"][0]["item"]["type"],
        "commandExecution"
    );
    assert!(rest["result"]["nextCursor"].is_null());
    assert!(engine
        .sessions
        .lock()
        .await
        .values()
        .all(|s| s.process.is_none()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn stored_pi_and_omp_resume_without_credentials_and_spawn_only_when_sending() {
    struct OfflineKeys;
    impl KeyStore for OfflineKeys {
        fn get(&self, _: &str) -> Result<Option<String>, String> { Err("credential access during offline resume".into()) }
        fn set(&self, _: &str, _: &str) -> Result<(), String> { panic!("must not write credentials") }
        fn delete(&self, _: &str) -> Result<(), String> { panic!("must not delete credentials") }
    }
    for profile in ["pi", "omp"] {
        let dir = temp();
        let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_, _| {}));
        engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), profile).unwrap().0;
        engine.keys = Arc::new(OfflineKeys);
        let providers: Vec<ProviderEntry> = serde_json::from_value(json!([{"id":"first","name":"First","kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"fake","enabled":true}]}])).unwrap();
        engine.configure("workspace", providers, AgentPolicy::default()).await;
        let sessions = engine.directory("workspace").unwrap();
        std::fs::create_dir_all(&sessions).unwrap();
        let file = sessions.join("stored.jsonl");
        let entries = [json!({"type":"session","version":3,"id":"stored","engine":profile,"cwd":dir,"modelProvider":"first","model":"fake"}),
            json!({"type":"message","id":"u","parentId":null,"message":{"role":"user","timestamp":1,"content":"Saved question"}}),
            json!({"type":"message","id":"a","parentId":"u","message":{"role":"assistant","timestamp":2,"content":[{"type":"text","text":"Saved answer"}],"stopReason":"stop"}})];
        std::fs::write(&file, entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n").unwrap();
        config::write_access(&dir.join(format!("{profile}/agent/workspace")), "stored", Some("read-only")).unwrap();
        let resumed = engine.request("workspace", "thread/resume", json!({"threadId":"stored"})).await.unwrap();
        assert_eq!(resumed["result"]["thread"]["engine"], profile);
        assert_eq!(resumed["result"]["thread"]["turns"][0]["items"][1]["text"], "Saved answer");
        assert_eq!(resumed["result"]["sandbox"]["type"], "readOnly");
        assert!(engine.sessions.lock().await["stored"].process.is_none());
        assert!(!dir.join(format!("{profile}/agent/workspace/models.json")).exists());
        assert!(engine.request("other-workspace", "thread/resume", json!({"threadId":"stored"})).await.is_err());
        if profile == "pi" {
            engine.fake = true; engine.keys = Arc::new(Keys);
            engine.request("workspace", "turn/start", json!({"threadId":"stored","input":[{"type":"text","text":"Continue"}]})).await.unwrap();
            let sessions = engine.sessions.lock().await;
            let process = sessions["stored"].process.as_ref().unwrap();
            assert_eq!(sessions["stored"].file, file);
            assert_eq!(process.translator.lock().await.access_mode, "read-only");
            drop(sessions);
            engine.shutdown().await.unwrap();
            let requests = std::fs::read_to_string(file.with_extension("requests")).unwrap();
            assert!(requests.contains("\"type\": \"prompt\""));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn rewind_tree_excludes_user_message_and_undo_retains_old_branch() {
    let dir = std::env::temp_dir().join(format!("pi-rewind-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.jsonl");
    let rows = [
        json!({"type":"session","id":"thread","cwd":"/tmp","timestamp":"2026-10-23T00:00:00Z"}),
        json!({"type":"message","id":"u1","parentId":null,"message":{"role":"user","timestamp":1,"content":[{"type":"text","text":"first"}]}}),
        json!({"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","timestamp":2,"content":[{"type":"text","text":"answer"}]}}),
        json!({"type":"message","id":"u2","parentId":"a1","message":{"role":"user","timestamp":3,"content":[{"type":"text","text":"second"}]}}),
    ];
    std::fs::write(&file, rows.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let leaf = rewind_session(&file, "thread", Some("thread:user:3"), None).unwrap();
    assert_eq!(leaf, "u2");
    assert_eq!(read_session(&file, &[]).unwrap().1.turns.len(), 1);
    assert!(std::fs::read_to_string(&file).unwrap().contains("second"));
    rewind_session(&file, "thread", None, Some(leaf)).unwrap();
    assert_eq!(read_session(&file, &[]).unwrap().1.turns.len(), 2);
    rewind_session(&file, "thread", Some("thread:user:1"), None).unwrap();
    assert!(read_session(&file, &[]).unwrap().1.turns.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn work_spawn_profile_loads_only_bundled_skills_and_preserves_connectors_and_subagents() {
    let mut command = Command::new("node");
    work_profile(&mut command, Path::new("/synthetic/work"), Some(Path::new("/synthetic/work/.venv/bin/python")));
    let args: Vec<_> = command.as_std().get_args().map(|s| s.to_string_lossy().into_owned()).collect();
    assert!(args.contains(&"read,write,edit,ls,find,grep,bash,subagent,ask_user,mcp__*".into()));
    assert!(args.contains(&"/synthetic/work/skills".into()));
    assert!(args.contains(&"--no-context-files".into()));
    assert!(args.contains(&"--no-skills".into()));
    assert!(args.contains(&WORK_PROMPT.into()));
    assert!(WORK_PROMPT.contains("outputs/") && WORK_PROMPT.contains("In Ask mode, ask before deleting or overwriting"));
    assert!(WORK_PROMPT.contains("Do not use ask_user to request tool permission"));
    assert!(command.as_std().get_envs().any(|(k, v)| k == "TAUCODE_WORK" && v == Some(std::ffi::OsStr::new("1"))));
}
#[test]
fn markdown_checklists_translate_to_plan_items_and_updates() {
    let mut translator = Translator::new("task", "/synthetic/space");
    translator.apply(&json!({"type":"agent_start"}));
    let mut plans = vec![];
    for (timestamp, text) in [(1, "- [ ] Read notes\n- [ ] Write brief"), (2, "- [x] Read notes\n- [ ] Write brief")] {
        translator.apply(&json!({"type":"message_start","message":{"role":"assistant","timestamp":timestamp}}));
        plans.extend(translator.apply(&json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":text}]}})).into_iter().filter(|e| e["method"] == "item/completed" && e["params"]["item"]["type"] == "plan"));
    }
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[1]["params"]["item"]["plan"][0]["status"], "completed");
    assert_eq!(plans[1]["params"]["item"]["plan"][1]["status"], "pending");
}
#[tokio::test]
async fn real_pi_work_starts_offline_with_work_policy_and_skills() {
    let dir = temp();
    let space = dir.join("space");
    std::fs::create_dir(&space).unwrap();
    let engine = PiEngine::work(dir.join("data"), "workspace".into(), space.to_string_lossy().into_owned(), Arc::new(|_, _| {}));
    let provider: ProviderEntry = serde_json::from_value(json!({"id":"local","name":"Offline","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"offline","enabled":true}]})).unwrap();
    engine.configure("workspace", vec![provider], AgentPolicy::default()).await;
    let response = engine.request("workspace", "thread/start", json!({"model":"local/offline"})).await.unwrap();
    let id = response["result"]["thread"]["id"].as_str().unwrap();
    assert!(engine.sessions.lock().await[id].process.is_none());
    let (registry, policy) = engine.registry.lock().await["workspace"].clone();
    engine.spawn(engine.sessions.lock().await.get_mut(id).unwrap(), &registry, &policy).await.unwrap();
    let access: Value = serde_json::from_slice(&std::fs::read(dir.join("data/pi/agent/workspace/taucode/threads").join(format!("{id}.json"))).unwrap()).unwrap();
    assert_eq!(access["kind"], "work");
    assert_eq!(access["accessMode"], "ask");
    assert_eq!(access["workspace"], json!(space));
    engine.request("workspace", "thread/accessMode/set", json!({"threadId":id,"accessMode":"full-access"})).await.unwrap();
    let access: Value = serde_json::from_slice(&std::fs::read(dir.join("data/pi/agent/workspace/taucode/threads").join(format!("{id}.json"))).unwrap()).unwrap();
    assert_eq!(access["kind"], "work");
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn side_chat_current_blocks_mutations_and_two_turn_checkpoints_bind_real_items() {
    use crate::backend::events::AppServerEvent;
    use crate::shared::{checkpoints::{Checkpoints, CheckpointLimits}, engines::WorkspaceSession};
    let dir = temp();
    let root = dir.join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("file"), "before").unwrap();
    let checkpoints = Checkpoints::with_limits(dir.clone(), "workspace".into(), root.to_string_lossy().into(),
        Arc::new(|_| {}), CheckpointLimits::testing());
    let observer = checkpoints.clone();
    let (tx, mut completions) = tokio::sync::mpsc::unbounded_channel();
    let providers = vec![serde_json::from_value(json!({"id":"first","name":"First","kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"fake","enabled":true}]})).unwrap()];
    let mut session = WorkspaceSession::lazy(Box::new(|| Box::pin(async { Err("Codex must not be used".into()) })),
        providers, AgentPolicy::default(), None, None, "workspace".into());
    session.checkpoints = Some(checkpoints.clone());
    session.pi = PiEngine::new(dir.clone(), "workspace".into(), root.to_string_lossy().into(), None,
        Arc::new(move |workspace, message| {
            let complete = message["method"] == "turn/completed";
            if let Some(handle) = observer.observe(&AppServerEvent { workspace_id: workspace.into(), message }) {
                if complete { tx.send(handle).unwrap(); }
            }
        }));
    session.pi.fake = true;
    session.pi.keys = Arc::new(Keys);
    let session = Arc::new(session);
    let core_sessions = Mutex::new(HashMap::from([("workspace".into(), session.clone())]));
    let workspaces = Mutex::new(HashMap::from([("workspace".into(), serde_json::from_value(json!({"id":"workspace","name":"Workspace","path":root,"settings":{}})).unwrap())]));
    let parent = session.send_request_for_workspace("workspace", "thread/start", json!({"engine":"pi","model":"first/fake"})).await.unwrap();
    let parent = parent["result"]["thread"]["id"].as_str().unwrap();
    let side = session.send_request_for_workspace("workspace", "thread/sidechat", json!({"threadId":parent})).await.unwrap();
    let id = side["result"]["thread"]["id"].as_str().unwrap();
    let home = dir.join("pi/agent/workspace");
    let access = home.join("taucode/threads").join(format!("{id}.json"));
    for (text, mode) in [("first", Some("current".into())), ("second", None)] {
        crate::shared::codex_core::send_user_message_core(&core_sessions, &workspaces, "workspace".into(), id.into(), text.into(), None, None, None, mode, None, None).await.unwrap();
        session.send_request_for_workspace("workspace", "turn/interrupt", json!({"threadId":id})).await.unwrap();
        completions.recv().await.unwrap().await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(&access).unwrap()).unwrap()["accessMode"], "read-only");
        assert_eq!(session.pi.sessions.lock().await[id].process.as_ref().unwrap().translator.lock().await.access_mode, "read-only");
    }
    let gate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/pi/extensions/taucode-gate.ts");
    let output = std::process::Command::new("node").args(["--input-type=module", "-e", r#"
        import assert from 'node:assert/strict';
        import { pathToFileURL } from 'node:url';
        const { default: gate } = await import(pathToFileURL(process.argv[1]).href);
        let handler;
        gate({ on: (name, fn) => { if (name === 'tool_call') handler = fn; } });
        const ctx = { cwd: process.argv[2], hasUI: true, ui: { select: () => assert.fail('read-only must not ask for approval') } };
        for (const [toolName, input] of [
            ['write', { path: 'file', content: 'changed' }],
            ['edit', { path: 'file', oldText: 'before', newText: 'changed' }],
            ['bash', { command: 'touch forbidden' }],
        ]) assert.equal((await handler({ toolName, toolCallId: toolName, input }, ctx)).block, true);
    "#]).arg(gate).arg(&root).env("PI_CODING_AGENT_DIR", &home).env("TAUCODE_SESSION_KEY", id)
        .env_remove("TAUCODE_SUBAGENT_DEPTH").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let list = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"list"})).await.unwrap();
    let rows = list["checkpoints"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let history = session.send_request_for_workspace("workspace", "thread/read", json!({"threadId":id})).await.unwrap();
    for (index, pair) in rows.chunks_exact(2).enumerate() {
        let turn = &history["result"]["thread"]["turns"][index];
        let user = turn["items"].as_array().unwrap().iter().find(|i| i["type"] == "userMessage").unwrap();
        for row in pair {
            assert_eq!(row["metadata"]["turnId"], turn["id"]);
            assert_eq!(row["metadata"]["userItemId"], user["id"]);
        }
    }
    assert_ne!(rows[0]["metadata"]["userItemId"], rows[2]["metadata"]["userItemId"]);
    assert_ne!(rows[0]["metadata"]["turnId"], rows[2]["metadata"]["turnId"]);
    std::fs::write(root.join("file"), "after").unwrap();
    let rewind = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"rewind","checkpointId":rows[2]["id"],"mode":"files","paths":["file"]})).await.unwrap();
    assert_eq!(rewind["conversationChanged"], false);
    assert_eq!(std::fs::read_to_string(root.join("file")).unwrap(), "before");
    let undo = checkpoints.request(&session, "workspace", json!({"threadId":id,"action":"undo"})).await.unwrap();
    assert_eq!(undo["conversationChanged"], false);
    assert_eq!(std::fs::read_to_string(root.join("file")).unwrap(), "after");
    for mode in ["ask", "full-access"] {
        crate::shared::codex_core::send_user_message_core(&core_sessions, &workspaces, "workspace".into(), id.into(), "override".into(), None, None, None, Some(mode.into()), None, None).await.unwrap();
        session.send_request_for_workspace("workspace", "turn/interrupt", json!({"threadId":id})).await.unwrap();
        completions.recv().await.unwrap().await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(&access).unwrap()).unwrap()["accessMode"], mode);
    }
    session.shutdown().await;
    std::fs::remove_dir_all(dir).unwrap();
}


#[tokio::test]
async fn detected_levels_agree_with_pi_and_are_set_before_turn_on_model_switch() {
    let dir = temp();
    let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into_owned(), None, Arc::new(|_, _| {}));
    engine.fake = true;
    engine.keys = Arc::new(Keys);
    let first = crate::shared::reasoning::capability(true, &["low", "medium", "high", "xhigh", "max"], "effort", "api");
    let second = crate::shared::reasoning::capability(true, &["low", "high"], "effort", "api");
    let registry: Vec<ProviderEntry> = serde_json::from_value(json!([{"id":"first","name":"Test","kind":"openai-compatible","baseUrl":"https://lab.test/v1","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"a","enabled":true,"reasoning":first},{"id":"b","enabled":true,"reasoning":second}]}])).unwrap();
    engine.configure("workspace", registry, AgentPolicy::default()).await;
    let started = engine.request("workspace", "thread/start", json!({"model":"first/a"})).await.unwrap();
    let id = started["result"]["thread"]["id"].as_str().unwrap();
    let process = engine.sessions.lock().await[id].process.clone().unwrap();
    assert_eq!(process.request(json!({"type":"get_available_thinking_levels"})).await.unwrap()["levels"], json!(first.levels));
    engine.request("workspace", "turn/start", json!({"threadId":id,"model":"first/b","effort":"max","input":[{"type":"text","text":"Hello"}]})).await.unwrap();
    assert_eq!(process.request(json!({"type":"get_available_thinking_levels"})).await.unwrap()["levels"], json!(second.levels));
    let file = engine.sessions.lock().await[id].file.clone();
    let requests: Vec<Value> = std::fs::read_to_string(file.with_extension("requests")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let prompt = requests.iter().position(|r| r["type"] == "prompt").unwrap();
    assert_eq!(requests[prompt - 1]["type"], "set_thinking_level");
    assert_eq!(requests[prompt - 1]["level"], "high");
    assert_eq!(requests[prompt - 2]["type"], "set_model");
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
#[ignore = "requires the installed omp and Bun runtimes"]
async fn live_omp_streams_from_registry_stub() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0; 8192];
                loop {
                    let n = stream.read(&mut buffer).await.unwrap_or(0);
                    if n == 0 { return; }
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]);
                        let length = headers.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                        if request.len() >= end + 4 + length { break; }
                    }
                }
                let chunks = [json!({"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello from omp"},"finish_reason":null}]}),json!({"id":"stub","object":"chat.completion.chunk","created":1,"model":"stub","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})];
                let body = format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",chunks[0],chunks[1]);
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    let dir = temp();
    let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
    engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), "omp").unwrap().0;
    let provider: ProviderEntry = serde_json::from_value(json!({"id":"local","name":"Stub","kind":"openai-compatible","baseUrl":format!("http://{address}/v1"),"enabled":true,"auth":{"type":"none"},"models":[{"id":"stub","enabled":true,"contextWindow":8192}]})).unwrap();
    engine.configure("workspace", vec![provider], AgentPolicy::default()).await;
    let mut events = engine.events();
    let start = engine.request("workspace", "thread/start", json!({"model":"local/stub"})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"Say hello"}],"accessMode":"read-only"})).await.unwrap();
    let mut streamed = false;
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let event = events.recv().await.unwrap();
            if event["method"] == "item/agentMessage/delta" && event.to_string().contains("Hello from omp") { streamed = true; }
            if event["method"] == "turn/completed" { assert_eq!(event["params"]["turn"]["status"], "completed", "{event}"); break; }
        }
    }).await.unwrap();
    assert!(streamed);
    assert!(dir.join("omp/agent/workspace/models.yml").exists());
    assert!(!dir.join("omp/agent/workspace/auth.json").exists());
    engine.shutdown().await.unwrap();
    server.abort();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn omp_models_are_yaml_compatible_and_do_not_contain_credentials() {
    let providers: Vec<ProviderEntry> = serde_json::from_value(json!([{"id":"test","name":"Test","kind":"openai-compatible","baseUrl":"http://localhost:9000/v1","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"stub","enabled":true}]}])).unwrap();
    let models = profile::omp_models(&providers);
    assert_eq!(models["providers"]["taucode-test"]["apiKey"], "TAUCODE_PROVIDER_TEST_KEY");
    assert_eq!(models["providers"]["taucode-test"]["api"], "openai-completions");
}

#[tokio::test]
async fn real_omp_profile_with_offline_transport_fixture() {
    if !Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp/node_modules/.bin/bun").exists() { return; }
    let dir = temp();
    let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
    engine.fixture_transport = true;
    engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), "omp").unwrap().0;
    let provider: ProviderEntry = serde_json::from_value(json!({"id":"local","name":"Stub","kind":"openai-compatible","baseUrl":"http://127.0.0.1:9019/v1","enabled":true,"auth":{"type":"none"},"models":[{"id":"stub","enabled":true,"contextWindow":8192}]})).unwrap();
    engine.configure("workspace", vec![provider], AgentPolicy::default()).await;
    let mut events = engine.events();
    let start = engine.request("workspace", "thread/start", json!({"model":"local/stub"})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    assert_eq!(start["result"]["thread"]["engine"], "omp");
    engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"Say hello"}],"accessMode":"read-only"})).await.unwrap();
    let mut streamed = false;
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let event = events.recv().await.unwrap();
            if event["method"] == "item/agentMessage/delta" && event.to_string().contains("Hello from omp") { streamed = true; }
            if event["method"] == "turn/completed" { assert_eq!(event["params"]["turn"]["status"], "completed", "{event}"); break; }
        }
    }).await.unwrap();
    assert!(streamed);
    engine.shutdown().await.unwrap();
    assert!(dir.join("omp/agent/workspace/models.yml").exists());
    assert!(!dir.join("omp/agent/workspace/auth.json").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn omp_preload_blocks_auth_and_unconfigured_network() {
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp");
    let bun = runtime.join("node_modules/.bin/bun");
    if !bun.exists() { return; }
    let dir = temp();
    std::fs::create_dir_all(dir.join("taucode")).unwrap();
    std::fs::write(dir.join("taucode/network.json"), "[]").unwrap();
    let result = std::process::Command::new(bun).arg(runtime.join("no-accounts.test.mjs"))
        .env_clear().envs(crate::shared::engine_registry::environment(std::env::vars(), &dir))
        .env("PI_CODING_AGENT_DIR", &dir).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    std::fs::remove_dir_all(dir).unwrap();
}
