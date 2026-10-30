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
async fn conversation_teams_persist_fork_and_keep_project_allowances() {
    for profile in ["pi", "omp"] {
        let dir = temp();
        let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_, _| {}));
        engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), profile).unwrap().0;
        let mut base = AgentPolicy::default();
        base.subagents.daily_token_limit = 10000;
        engine.configure("workspace", vec![], base.clone()).await;
        let sessions = engine.directory("workspace").unwrap();
        std::fs::create_dir_all(&sessions).unwrap();
        for id in ["one", "two"] {
            std::fs::write(sessions.join(format!("{id}.jsonl")), format!("{}\n", json!({"type":"session","version":3,"id":id,"engine":profile,"cwd":dir}))).unwrap();
        }
        let mut team = base.subagents.clone();
        team.max_run_minutes = 7;
        team.daily_token_limit = 0;
        let set = engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"set","policy":team})).await.unwrap();
        assert_eq!(set["overridden"], true);
        assert_eq!(set["policy"]["subagents"]["dailyTokenLimit"], 10000);
        assert_eq!(set["policy"]["subagents"]["maxRunMinutes"], 7);
        let other = engine.request("workspace", "thread/agentPolicy", json!({"threadId":"two","action":"get"})).await.unwrap();
        assert_eq!(other["overridden"], false);
        assert_eq!(other["policy"]["subagents"]["maxRunMinutes"], 0);
        assert!(engine.sessions.lock().await.values().all(|s| s.process.is_none()), "editing teams must not start models or read credentials");
        assert!(engine.request("wrong-workspace", "thread/agentPolicy", json!({"threadId":"one","action":"get"})).await.is_err());
        team.max_run_minutes = 1441;
        assert!(engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"set","policy":team})).await.is_err());
        assert!(engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"unknown"})).await.is_err());
        let fork = engine.request("workspace", "thread/fork", json!({"threadId":"one","sideOf":{"threadId":"one","workspaceId":"workspace"}})).await.unwrap();
        let fork_id = fork["result"]["thread"]["id"].as_str().unwrap();
        let home = dir.join(format!("{profile}/agent/workspace"));
        assert_eq!(config::read_team(&home, fork_id).unwrap().unwrap().max_run_minutes, 7);
        engine.sessions.lock().await.clear();
        base.subagents.daily_token_limit = 5000;
        engine.configure("workspace", vec![], base).await;
        let persisted = engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"get"})).await.unwrap();
        assert_eq!(persisted["policy"]["subagents"]["maxRunMinutes"], 7);
        assert_eq!(persisted["policy"]["subagents"]["dailyTokenLimit"], 5000);
        std::fs::write(home.join("taucode/teams/one.json"), "invalid json").unwrap();
        assert!(engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"get"})).await.is_err());
        let reset = engine.request("workspace", "thread/agentPolicy", json!({"threadId":"one","action":"reset"})).await.unwrap();
        assert_eq!(reset["overridden"], false);
        assert_eq!(reset["policy"]["subagents"]["maxRunMinutes"], 0);
        assert_eq!(config::read_team(&home, fork_id).unwrap().unwrap().max_run_minutes, 7);
        assert!(config::write_team(&home, "../escape", None).is_err());
        assert!(config::read_team(&home, "").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn conversation_teams_snapshot_each_engine_session_until_next_spawn() {
    let dir = temp();
    let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_, _| {}));
    engine.fake = true;
    engine.keys = Arc::new(Keys);
    let providers = serde_json::from_value(json!([{"id":"first","name":"First","kind":"ollama","enabled":true,"auth":{"type":"apiKey"},"models":[{"id":"fake","enabled":true}]}])).unwrap();
    let base = AgentPolicy::default();
    engine.configure("workspace", providers, base.clone()).await;
    let first = engine.request("workspace", "thread/start", json!({"model":"first/fake"})).await.unwrap();
    let one = first["result"]["thread"]["id"].as_str().unwrap();
    let second = engine.request("workspace", "thread/start", json!({"model":"first/fake"})).await.unwrap();
    let two = second["result"]["thread"]["id"].as_str().unwrap();
    let home = dir.join("pi/agent/workspace");
    let snapshot = |id: &str| -> Value { serde_json::from_slice(&std::fs::read(home.join("taucode/policies").join(format!("{id}.json"))).unwrap()).unwrap() };
    let mut team = base.subagents.clone(); team.max_run_minutes = 7;
    engine.request("workspace", "thread/agentPolicy", json!({"threadId":one,"action":"set","policy":team})).await.unwrap();
    assert_eq!(snapshot(one)["maxRunMinutes"], 0, "saving must not alter a running session's policy");
    engine.request("workspace", "turn/start", json!({"threadId":one,"input":[{"type":"text","text":"hello"}]})).await.unwrap();
    assert_eq!(snapshot(one)["maxRunMinutes"], 7);
    assert_eq!(snapshot(two)["maxRunMinutes"], 0);
    team.max_run_minutes = 9; team.enabled = true;
    engine.request("workspace", "thread/agentPolicy", json!({"threadId":one,"action":"set","policy":team})).await.unwrap();
    config::write_policy(&home, &[], &base).unwrap();
    assert_eq!(snapshot(one)["maxRunMinutes"], 7, "project and team edits apply to subsequent turns");
    std::fs::write(home.join("taucode/teams").join(format!("{one}.json")), "broken").unwrap();
    engine.request("workspace", "turn/interrupt", json!({"threadId":one})).await.unwrap();
    config::write_team(&home, one, Some(&team)).unwrap();
    std::fs::write(dir.join("settings.json"), json!({"features":{"subagents":false}}).to_string()).unwrap();
    engine.request("workspace", "turn/start", json!({"threadId":one,"input":[{"type":"text","text":"hello"}]})).await.unwrap();
    assert_eq!(snapshot(one)["enabled"], false, "conversation overrides must respect the global feature switch");
    assert_eq!(snapshot(one)["maxConcurrent"], 0);
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn conversation_teams_reject_chat_and_work() {
    let dir = temp();
    for engine in [PiEngine::chat(dir.clone(), Arc::new(|_, _| {})), PiEngine::work(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), Arc::new(|_, _| {}))] {
        let workspace = if engine.chat { "chats" } else { "workspace" };
        engine.configure(workspace, vec![], AgentPolicy::default()).await;
        let sessions = engine.directory(workspace).unwrap();
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join("one.jsonl"), format!("{}\n", json!({"type":"session","version":3,"id":"one","cwd":dir}))).unwrap();
        assert!(engine.request(workspace, "thread/agentPolicy", json!({"threadId":"one","action":"get"})).await.unwrap_err().contains("only in Code"));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn chain_of_command_is_code_only_and_does_not_change_saved_policy() {
    let dir = temp();
    let mut policy = AgentPolicy::default();
    policy.subagents.enabled = true;
    policy.subagents.mode = crate::types::SubagentMode::ChainOfCommand;
    policy.subagents.default_model = Some(crate::types::ModelRef { provider_id: "local".into(), model_id: "worker".into() });
    let original = json!(policy);
    for (engine, code) in [
        (PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_, _| {})), true),
        (PiEngine::chat(dir.clone(), Arc::new(|_, _| {})), false),
        (PiEngine::work(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), Arc::new(|_, _| {})), false),
    ] {
        engine.configure("workspace", vec![], policy.clone()).await;
        let effective = engine.registry.lock().await["workspace"].1.clone();
        assert_eq!(effective.subagents.enabled, code);
        assert_eq!(effective.subagents.mode == crate::types::SubagentMode::ChainOfCommand, code);
        assert_eq!(json!(effective.subagents.roles), json!(policy.subagents.roles));
        assert_eq!(json!(effective.subagents.default_model), json!(policy.subagents.default_model));
        // Custom delegation remains independent of the Code-only hierarchy.
        let mut custom = policy.clone();
        custom.subagents.mode = crate::types::SubagentMode::Custom;
        engine.configure("workspace", vec![], custom).await;
        assert!(engine.registry.lock().await["workspace"].1.subagents.enabled);
    }
    assert_eq!(json!(policy), original);
    std::fs::remove_dir_all(dir).unwrap();
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
fn late_subagent_progress_cannot_restart_a_finished_tool_or_turn() {
    for aborted in [false, true] {
        let mut t = Translator::new("test", "/workspace");
        t.apply(&json!({"type":"agent_start"}));
        t.apply(&json!({"type":"tool_execution_start","toolCallId":"child","toolName":"subagent","args":{"agent":"colonel","task":"delegate"}}));
        let progress = json!({"type":"tool_execution_update","toolCallId":"child","partialResult":{"details":{"results":[{"agent":"colonel","status":"running","messages":[],"usage":{}}]}}});
        assert!(t.apply(&progress).iter().any(|e| e["method"] == "item/started"));
        if !aborted {
            t.apply(&json!({"type":"tool_execution_end","toolCallId":"child","result":{"details":{"results":[]}}}));
        }
        t.apply(&json!({"type":"agent_settled","aborted":aborted}));
        let settled = t.turns.clone();
        assert!(t.apply(&progress).is_empty(), "late progress must not re-open Working");
        assert_eq!(t.turns, settled);
        t.apply(&json!({"type":"agent_start"}));
        t.apply(&json!({"type":"tool_execution_start","toolCallId":"next","toolName":"subagent","args":{"agent":"major","task":"review"}}));
        let mut next = progress.clone();
        next["toolCallId"] = json!("next");
        assert!(t.apply(&next).iter().any(|e| e["method"] == "item/started"));
        assert!(t.apply(&progress).is_empty());
    }
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
            && e["params"]["tokenUsage"]["total"]["knownCost"] == t.cost
            && e["params"]["tokenUsage"]["total"]["cost"].is_null()));
}

#[test]
fn unknown_costs_survive_aggregation_without_hiding_known_subtotals() {
    let mut t = Translator::new("cost", "/workspace");
    t.apply(&json!({"type":"agent_start"}));
    t.apply(&json!({"type":"tool_execution_start","toolCallId":"team","toolName":"subagent","args":{"agent":"colonel","task":"Check"}}));
    let mut result = json!({"details":{"results":[{"agent":"colonel","status":"running","usage":{"input":10,"cost":0.25},"messages":[{"role":"toolResult","toolCallId":"child","details":{"results":[
        {"agent":"major","status":"completed","usage":{"input":5,"cost":0.125}},
        {"agent":"enlisted","status":"running","usage":{"input":3,"cost":0}}
    ]}}]}]}});
    t.apply(&json!({"type":"tool_execution_update","toolCallId":"team","partialResult":result}));
    let usage = &t.turns[0]["items"][0]["agentsTree"][0]["usage"];
    assert_eq!(usage["cost"], Value::Null);
    assert_eq!(usage["knownCost"], 0.375);
    assert_eq!(usage["input"], 18);
    assert_eq!(t.total["cost"], Value::Null);
    assert_eq!(t.total["knownCost"], 0.375);
    // A later complete snapshot resolves that source's uncertainty without double counting.
    result["details"]["results"][0]["messages"][0]["details"]["results"][1]["usage"]["cost"] = json!(0.5);
    t.apply(&json!({"type":"tool_execution_end","toolCallId":"team","result":result}));
    assert_eq!(t.total["cost"], 0.875);
    let total = t.total.clone();
    t.apply(&json!({"type":"tool_execution_end","toolCallId":"team","result":result}));
    assert_eq!(t.total, total);
    // Explicitly known zero is different from the old unpriced zero placeholder.
    let zero = agents::tree("free", &[json!({"agent":"major","usage":{"input":2,"cost":0,"costKnown":true}})], &Value::Null, true, None, &mut Default::default());
    assert_eq!(zero[0]["usage"]["cost"], 0.0);
    // A separate unpriced commander response keeps the whole conversation total unknown.
    t.apply(&json!({"type":"message_start","message":{"role":"assistant","timestamp":1}}));
    t.apply(&json!({"type":"message_end","message":{"role":"assistant","content":[],"usage":{"input":7,"cost":{"total":0}}}}));
    t.apply(&json!({"type":"message_start","message":{"role":"assistant","timestamp":2}}));
    t.apply(&json!({"type":"message_end","message":{"role":"assistant","content":[],"usage":{"input":4,"cost":{"total":0.125}}}}));
    assert_eq!(t.total["cost"], Value::Null);
    assert_eq!(t.total["knownCost"], 1.0);
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

#[test]
fn virtual_subagent_results_keep_nested_progress_usage_and_history() {
    for transport in ["write", "edit"] {
        let args = json!({"path":"xd://subagent","content":"{\"agent\":\"colonel\",\"task\":\"Delegate\"}"});
        let wrap = |results: Value| json!({"xdev":{"tool":"subagent","mode":"execute","inner":{"results":results}}});
        let mut child = json!({"agent":"enlisted","model":"local/worker","status":"running","usage":{"input":17,"output":3,"cost":0.25},"messages":[{"role":"assistant","content":[{"type":"text","text":"Nested worker evidence"}]}]});
        let mut parent = json!({"agent":"colonel","model":"cloud/planner","status":"running","usage":{"input":11,"output":2,"cost":0.5},"messages":[{"role":"toolResult","toolCallId":"nested","toolName":transport,"details":wrap(json!([child]))}]});
        let mut t = Translator::new("thread", "/workspace");
        t.apply(&json!({"type":"agent_start"}));
        t.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":transport,"args":args}));
        let update = json!({"type":"tool_execution_update","toolCallId":"call","toolName":transport,"partialResult":{"details":wrap(json!([parent]))}});
        let events = t.apply(&update);
        assert!(events.iter().any(|e| e["params"]["item"]["agentsTree"][0]["children"][0]["status"] == "running"));
        let started = t.turns[0]["items"][0]["agentsTree"][0]["startedAt"].clone();
        child["status"] = json!("completed");
        let failed = json!({"agent":"major","status":"failed","stderr":"QA failed","usage":{"input":5},"messages":[]});
        parent["status"] = json!("completed");
        parent["messages"][0]["details"] = wrap(json!([child, failed]));
        let result = json!({"content":[{"type":"text","text":"Team returned"}],"details":wrap(json!([parent]))});
        let end = json!({"type":"tool_execution_end","toolCallId":"call","toolName":transport,"result":result});
        t.apply(&end);
        let node = &t.turns[0]["items"][0]["agentsTree"][0];
        assert_eq!(node["status"], "completed");
        assert_eq!(node["startedAt"], started);
        assert!(node["endedAt"].is_number());
        assert_eq!(node["usage"]["input"], 33);
        assert_eq!(node["usage"]["output"], 5);
        assert_eq!(node["usage"]["cost"], Value::Null);
        assert_eq!(node["usage"]["knownCost"], 0.75);
        assert_eq!(node["children"][0]["id"], "call:agent:0/nested:agent:0");
        assert_eq!(node["children"][0]["status"], "completed");
        assert_eq!(node["children"][0]["model"], "local/worker");
        assert_eq!(node["children"][1]["status"], "failed");
        assert_eq!(node["children"][1]["errorMessage"], "QA failed");
        let total = t.total.clone();
        t.apply(&end);
        assert_eq!(t.total, total);
        assert!(!t.transcripts["call:agent:0"].to_string().contains("Nested worker evidence"));
        assert!(t.transcripts["call:agent:0/nested:agent:0"].to_string().contains("Nested worker evidence"));
        let history = Translator::history("thread", "/workspace", &[
            json!({"role":"user","timestamp":1,"content":"Delegate"}),
            json!({"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"call","name":transport,"arguments":args}]}),
            json!({"role":"toolResult","toolCallId":"call","toolName":transport,"content":result["content"],"details":result["details"]}),
        ]);
        let restored = &history.turns[0]["items"][1]["agentsTree"][0];
        assert_eq!(restored["status"], "completed");
        assert_eq!(restored["usage"], t.turns[0]["items"][0]["agentsTree"][0]["usage"]);
        assert_eq!(restored["children"][0]["status"], "completed");
        assert_eq!(restored["children"][1]["status"], "failed");
        assert_eq!(history.transcripts, t.transcripts);
    }
}

#[test]
fn cancelled_subagent_history_restores_timing_and_evidence_on_its_branch() {
    let dir = temp();
    let file = dir.join("session.jsonl");
    let result = json!({"isError":true,"content":[],"details":{"results":[{
        "agent":"colonel","status":"failed","stopReason":"aborted","startedAt":1000,"endedAt":9000,"usage":{"input":10},
        "messages":[{"role":"toolResult","toolCallId":"nested","details":{"xdev":{"tool":"subagent","inner":{"results":[
            {"agent":"enlisted","status":"running","startedAt":2000,"usage":{"input":4},"messages":[{"role":"assistant","content":[{"type":"text","text":"Partial worker evidence"}]}]},
            {"agent":"major","status":"completed","startedAt":3000,"endedAt":6000,"messages":[]},
            {"agent":"enlisted","status":"pending","messages":[]}
        ]}}}}]
    }]}});
    let mut entries = vec![
        json!({"type":"session","id":"timing","cwd":"/workspace"}),
        json!({"type":"message","id":"user","parentId":null,"message":{"role":"user","timestamp":1,"content":"Delegate"}}),
        json!({"type":"message","id":"call","parentId":"user","message":{"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"delegate","name":"write","arguments":{"path":"xd://subagent","content":"{\"agent\":\"colonel\",\"task\":\"Delegate\"}"}}]}}),
        json!({"type":"custom","customType":"taucode-subagent-result","id":"saved","parentId":"call","data":{"toolCallId":"delegate","result":result}}),
        json!({"type":"message","id":"cancelled","parentId":"saved","message":{"role":"toolResult","toolCallId":"delegate","toolName":"write","isError":true,"content":[{"type":"text","text":"Operation aborted"}],"details":{}}}),
    ];
    let write = |entries: &[Value]| std::fs::write(&file, entries.iter().map(|e| format!("{e}\n")).collect::<String>()).unwrap();
    write(&entries);
    let (_, restored) = read_session(&file, &[]).unwrap();
    let node = &restored.turns[0]["items"][1]["agentsTree"][0];
    assert_eq!(node["status"], "aborted");
    assert_eq!(node["startedAt"], 1000);
    assert_eq!(node["endedAt"], 9000);
    assert_eq!(node["children"][0]["status"], "aborted");
    assert_eq!(node["children"][0]["startedAt"], 2000);
    assert_eq!(node["children"][0]["endedAt"], 9000);
    assert_eq!(node["children"][1]["status"], "completed");
    assert_eq!(node["children"][1]["endedAt"], 6000);
    assert_eq!(node["children"][2]["startedAt"], Value::Null);
    assert_eq!(node["usage"]["input"], 14);
    assert_eq!(restored.turns[0]["status"], "interrupted");
    assert!(restored.transcripts["delegate:agent:0/nested:agent:0"].to_string().contains("Partial worker evidence"));
    // OMP may persist its generic cancellation before extension cleanup finishes.
    entries[3]["parentId"] = json!("cancelled");
    entries[4]["parentId"] = json!("call");
    entries.swap(3, 4);
    write(&entries);
    let (_, late) = read_session(&file, &[]).unwrap();
    assert_eq!(late.turns[0]["status"], restored.turns[0]["status"]);
    assert_eq!(late.turns[0]["items"][1]["agentsTree"], restored.turns[0]["items"][1]["agentsTree"]);
    assert_eq!(late.total, restored.total);
    assert_eq!(late.transcripts, restored.transcripts);
    // Rewinding before the tool call must not pull abandoned evidence into the branch.
    entries.push(json!({"type":"message","id":"fork","parentId":"user","message":{"role":"assistant","timestamp":3,"stopReason":"stop","content":[{"type":"text","text":"Alternative"}]}}));
    write(&entries);
    let (_, branch) = read_session(&file, &[]).unwrap();
    assert!(branch.transcripts.is_empty());
    assert!(!json!(branch.turns).to_string().contains("Partial worker evidence"));
    entries.push(json!({"type":"custom","customType":"taucode-subagent-result","id":"unmatched","parentId":"fork","data":{"toolCallId":"delegate","result":result}}));
    write(&entries);
    assert_eq!(read_session(&file, &[]).unwrap().1.turns, branch.turns, "ignore checkpoints without a matching delegation on this branch");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stopping_a_team_preserves_completed_children_without_reporting_a_failure() {
    let mut t = Translator::new("thread", "/workspace");
    t.apply(&json!({"type":"agent_start"}));
    t.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":"subagent","args":{"agent":"colonel","task":"Delegate"}}));
    let update = json!({"type":"tool_execution_update","toolCallId":"call","partialResult":{"details":{"results":[{"agent":"colonel","status":"running","messages":[{"role":"toolResult","toolCallId":"nested","details":{"results":[{"agent":"enlisted","status":"running"},{"agent":"major","status":"completed","startedAt":1000,"endedAt":2000}]}}]}]}}});
    t.apply(&update);
    t.apply(&json!({"type":"abort_requested"}));
    t.apply(&json!({"type":"tool_execution_end","toolCallId":"call","result":{"isError":true,"content":[{"type":"text","text":"Operation aborted"}]}}));
    t.apply(&update); // Late progress cannot reopen stopped nodes.
    t.apply(&json!({"type":"agent_settled"}));
    let node = &t.turns[0]["items"][0]["agentsTree"][0];
    assert_eq!(node["status"], "aborted");
    assert_eq!(node["children"][0]["status"], "aborted");
    assert_eq!(node["children"][1]["status"], "completed");
    assert_eq!(node["children"][1]["endedAt"], 2000);
    assert_eq!(t.turns[0]["status"], "interrupted");
    let never_started = json!([{"agent":"enlisted","status":"failed","stopReason":"aborted","endedAt":9000}]);
    let queued = agents::tree("queued", never_started.as_array().unwrap(), &Value::Null, true, Some(10000), &mut t.transcripts);
    assert_eq!(queued[0]["startedAt"], Value::Null, "a cancelled queue entry never acquires a synthetic start time");
    assert_eq!(queued[0]["endedAt"], 9000);
}

#[test]
fn crashed_delegation_recovers_only_unfinished_calls_on_the_current_branch() {
    use sha2::{Digest, Sha256};
    let dir = temp();
    let file = dir.join("session.jsonl");
    let mut entries = vec![
        json!({"type":"session","id":"crash","cwd":"/workspace"}),
        json!({"type":"message","id":"user","parentId":null,"message":{"role":"user","timestamp":1,"content":"Delegate"}}),
        json!({"type":"message","id":"call","parentId":"user","message":{"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"delegate","name":"subagent","arguments":{"agent":"colonel","task":"Delegate"}}]}}),
    ];
    let write = |entries: &[Value]| std::fs::write(&file, entries.iter().map(|e| format!("{e}\n")).collect::<String>()).unwrap();
    write(&entries);
    let recovery = dir.join("session.jsonl.agents");
    std::fs::create_dir(&recovery).unwrap();
    let checkpoint = recovery.join(format!("{:x}.json", Sha256::digest(b"delegate")));
    let saved = json!({"version":1,"toolCallId":"delegate","savedAt":9000,"result":{"details":{"results":[{
        "agent":"colonel","status":"running","startedAt":1000,"usage":{"input":10},
        "messages":[{"role":"toolResult","toolCallId":"nested","details":{"results":[
            {"agent":"enlisted","status":"running","startedAt":2000,"messages":[{"role":"assistant","content":[{"type":"text","text":"Partial crash evidence"}]}]},
            {"agent":"major","status":"completed","startedAt":3000,"endedAt":6000,"messages":[]}
        ]}}]
    }]}}});
    std::fs::write(&checkpoint, saved.to_string()).unwrap();
    let (_, restored) = read_session(&file, &[]).unwrap();
    let node = &restored.turns[0]["items"][1]["agentsTree"][0];
    assert_eq!(restored.turns[0]["status"], "interrupted");
    assert_eq!(node["status"], "aborted");
    assert_eq!(node["startedAt"], 1000);
    assert_eq!(node["endedAt"], Value::Null, "a checkpoint is not an observed end time");
    assert_eq!(node["children"][0]["status"], "aborted");
    assert_eq!(node["children"][1]["status"], "completed");
    assert_eq!(node["children"][1]["endedAt"], 6000);
    assert!(restored.transcripts.values().any(|v| v.to_string().contains("Partial crash evidence")));
    // A subsequent user turn must not merge into the interrupted one.
    entries.push(json!({"type":"message","id":"next","parentId":"call","message":{"role":"user","timestamp":10000,"content":"Continue"}}));
    write(&entries);
    assert_eq!(read_session(&file, &[]).unwrap().1.turns.len(), 2);
    entries.pop();
    // OMP may persist only a generic abort result when its desktop parent exits.
    entries.push(json!({"type":"message","id":"abort","parentId":"call","message":{"role":"toolResult","toolCallId":"delegate","toolName":"subagent","isError":true,"content":[{"type":"text","text":"Aborted: The operation was aborted."}]}}));
    write(&entries);
    let (_, aborted) = read_session(&file, &[]).unwrap();
    assert_eq!(aborted.turns[0]["status"], "interrupted");
    let node = &aborted.turns[0]["items"][1]["agentsTree"][0];
    assert_eq!(node["children"][0]["status"], "aborted");
    assert_eq!(node["children"][1]["status"], "completed");
    assert_eq!(node["children"][1]["endedAt"], 6000);
    assert!(aborted.transcripts.values().any(|v| v.to_string().contains("Partial crash evidence")));
    entries.pop();
    // Durable final results always supersede a leftover checkpoint.
    entries.push(json!({"type":"message","id":"done","parentId":"call","message":{"role":"toolResult","toolCallId":"delegate","toolName":"subagent","details":{"results":[{"agent":"colonel","status":"completed","startedAt":1000,"endedAt":10000,"messages":[]}]}}}));
    write(&entries);
    let (_, finished) = read_session(&file, &[]).unwrap();
    assert_eq!(finished.turns[0]["items"][1]["agentsTree"][0]["status"], "completed");
    assert!(!json!(finished.turns).to_string().contains("Partial crash evidence"));
    // A different branch, malformed checkpoint or wrong call ID cannot restore it.
    entries.pop();
    entries.push(json!({"type":"message","id":"fork","parentId":"user","message":{"role":"assistant","timestamp":3,"stopReason":"stop","content":[{"type":"text","text":"Alternative"}]}}));
    write(&entries);
    assert!(read_session(&file, &[]).unwrap().1.transcripts.is_empty());
    entries.pop(); write(&entries);
    let mut wrong = saved.clone(); wrong["toolCallId"] = json!("unrelated");
    for bad in ["{".to_string(), wrong.to_string()] {
        std::fs::write(&checkpoint, bad).unwrap();
        let (_, missing) = read_session(&file, &[]).unwrap();
        assert!(!json!(missing.transcripts).to_string().contains("Partial crash evidence"));
        assert_eq!(missing.turns[0]["status"], "interrupted");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn root_stream_recovery_preserves_branch_identity_and_durable_results() {
    use sha2::{Digest, Sha256};
    let dir = temp();
    let file = dir.join("session.jsonl");
    let folder = dir.join("session.jsonl.streams");
    std::fs::create_dir(&folder).unwrap();
    let save = |key: &str, value: Value| std::fs::write(folder.join(format!("{:x}.json", Sha256::digest(key.as_bytes()))), value.to_string()).unwrap();
    let mut entries = vec![
        json!({"type":"session","id":"root-crash","cwd":"/workspace"}),
        json!({"type":"message","id":"user","parentId":null,"message":{"role":"user","timestamp":1,"content":"Start"}}),
    ];
    let write = |entries: &[Value]| std::fs::write(&file, entries.iter().map(|e| format!("{e}\n")).collect::<String>()).unwrap();
    let partial = json!({"version":1,"savedAt":100,"anchor":"user","result":{"role":"assistant","timestamp":2,"content":[{"type":"text","text":"ROOT_PARTIAL"},{"type":"toolCall","id":"unsafe","name":"bash","arguments":{"command":"never executed"}}]}});
    save("message:user", partial.clone()); write(&entries);
    let original = std::fs::read(&file).unwrap();
    let (_, restored) = read_session(&file, &[]).unwrap();
    assert_eq!(restored.turns[0]["status"], "interrupted");
    assert_eq!(restored.turns[0]["items"][1]["text"], "ROOT_PARTIAL");
    assert_eq!(restored.turns[0]["items"].as_array().unwrap().len(), 2, "partial calls must not appear executed");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    entries.push(json!({"type":"message","id":"next","parentId":"user","message":{"role":"user","timestamp":3,"content":"Continue"}}));
    write(&entries);
    assert_eq!(read_session(&file, &[]).unwrap().1.turns.len(), 2);
    entries.pop();
    // A completed response, alternative branch or explicit rewind suppresses it.
    for next in [
        json!({"type":"message","id":"answer","parentId":"user","message":{"role":"assistant","timestamp":2,"stopReason":"stop","content":[{"type":"text","text":"Completed"}]}}),
        json!({"type":"message","id":"alternative","parentId":"user","message":{"role":"assistant","timestamp":4,"stopReason":"stop","content":[{"type":"text","text":"Alternative"}]}}),
        json!({"type":"custom","customType":"taucode-rewind","id":"rewind","parentId":"user"}),
    ] {
        entries.push(next); write(&entries);
        assert!(!json!(read_session(&file, &[]).unwrap().1.turns).to_string().contains("ROOT_PARTIAL"));
        entries.pop();
    }
    entries.push(json!({"type":"message","id":"abort","parentId":"user","message":{"role":"assistant","timestamp":2,"stopReason":"aborted","content":[]}}));
    write(&entries);
    assert_eq!(read_session(&file, &[]).unwrap().1.turns[0]["items"][1]["text"], "ROOT_PARTIAL");
    entries.pop();
    save("message:user", json!({"version":1,"savedAt":100,"anchor":"wrong","result":partial["result"]}));
    write(&entries);
    assert!(!json!(read_session(&file, &[]).unwrap().1.turns).to_string().contains("ROOT_PARTIAL"));
    entries.push(json!({"type":"message","id":"call","parentId":"user","message":{"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[{"type":"toolCall","id":"shell","name":"bash","arguments":{"command":"timer"}}]}}));
    save("tool:shell", json!({"version":1,"savedAt":100,"toolCallId":"shell","toolName":"bash","result":{"content":[{"type":"text","text":"TOOL_PARTIAL"}]}}));
    write(&entries);
    let (_, tool) = read_session(&file, &[]).unwrap();
    assert_eq!(tool.turns[0]["status"], "interrupted");
    assert_eq!(tool.turns[0]["items"][1]["aggregatedOutput"], "TOOL_PARTIAL");
    assert_eq!(tool.turns[0]["items"][1]["exitCode"], Value::Null);
    assert_eq!(tool.turns[0]["items"][1]["durationMs"], Value::Null);
    entries.push(json!({"type":"message","id":"result","parentId":"call","message":{"role":"toolResult","toolCallId":"shell","toolName":"bash","content":[{"type":"text","text":"DURABLE_RESULT"}]}}));
    write(&entries);
    let (_, done) = read_session(&file, &[]).unwrap();
    assert_eq!(done.turns[0]["items"][1]["aggregatedOutput"], "DURABLE_RESULT");
    let last = entries.last_mut().unwrap();
    last["message"]["isError"] = json!(true);
    last["message"]["content"] = json!([{"type":"text","text":"Aborted: The operation was aborted."}]);
    write(&entries);
    assert_eq!(read_session(&file, &[]).unwrap().1.turns[0]["items"][1]["aggregatedOutput"], "TOOL_PARTIAL\nAborted: The operation was aborted.");
    // OMP exits with a cancellation snapshot plus a durable copy of its output.
    save("tool:shell", json!({"version":1,"savedAt":100,"toolCallId":"shell","toolName":"bash","result":{"content":[{"type":"text","text":"[Command cancelled]\nTOOL_PARTIAL\n"}]}}));
    entries.last_mut().unwrap()["message"]["content"] = json!([{"type":"text","text":"TOOL_PARTIAL\n"}]);
    entries.insert(entries.len()-1, json!({"type":"custom","customType":"session_exit","id":"exit","parentId":"call","data":{"pendingToolCalls":[{"toolCallId":"shell"}]}}));
    entries.last_mut().unwrap()["parentId"] = json!("exit");
    entries.push(json!({"type":"message","id":"followup","parentId":"result","message":{"role":"user","timestamp":3,"content":"Continue"}}));
    entries.push(json!({"type":"message","id":"reply","parentId":"followup","message":{"role":"assistant","timestamp":4,"stopReason":"stop","content":[{"type":"text","text":"Follow-up complete"}]}}));
    write(&entries);
    let (_, stopped) = read_session(&file, &[]).unwrap();
    assert_eq!(stopped.turns.len(), 2);
    assert_eq!(stopped.turns[0]["status"], "interrupted");
    assert_eq!(stopped.turns[1]["status"], "completed");
    assert_eq!(stopped.turns[0]["items"][1]["aggregatedOutput"], "[Command cancelled]\nTOOL_PARTIAL\n");
    assert_eq!(stopped.turns[0]["items"][1]["exitCode"], Value::Null);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn supervised_commands_display_the_requested_command() {
    let mut t = Translator::new("thread", "/workspace");
    t.apply(&json!({"type":"agent_start"}));
    t.apply(&json!({"type":"message_end","message":{"role":"assistant","timestamp":1,"content":[{"type":"toolCall","id":"call","name":"bash","arguments":{"command":"printf hello"}}]}}));
    t.apply(&json!({"type":"tool_execution_start","toolCallId":"call","toolName":"bash","args":{"command":"'/runtime' '/extensions/lib/commandGuard.ts' '123' '/bin/bash' 'printf hello'"}}));
    assert_eq!(t.turns[0]["items"][0]["command"], "printf hello");
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
    policy.subagents.roles[0].command_rank = Some(crate::types::CommandRank::Enlisted);
    config::write_policy(&home, &[], &policy).unwrap();
    assert_eq!(read(&home.join("taucode/policy.json"))["mode"], "chain-of-command");
    assert_eq!(read(&home.join("taucode/policy.json"))["roles"][0]["commandRank"], "enlisted");
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
        .env("FAKE_PI_REQUESTS", marker.with_extension("requests"))
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
        let mut crashed = entries.to_vec();
        crashed.extend([
            json!({"type":"message","id":"crash-user","parentId":"a","message":{"role":"user","timestamp":3,"content":"Delegate"}}),
            json!({"type":"message","id":"crash-call","parentId":"crash-user","message":{"role":"assistant","timestamp":4,"stopReason":"toolUse","content":[{"type":"toolCall","id":"interrupted","name":"subagent","arguments":{"agent":"major","task":"Check"}}]}}),
        ]);
        std::fs::write(&file, crashed.iter().map(Value::to_string).collect::<Vec<_>>().join("\n") + "\n").unwrap();
        use sha2::{Digest, Sha256};
        let recovery = sessions.join("stored.jsonl.agents");
        std::fs::create_dir(&recovery).unwrap();
        std::fs::write(recovery.join(format!("{:x}.json", Sha256::digest(b"interrupted"))), json!({"version":1,"toolCallId":"interrupted","savedAt":5,"result":{"details":{"results":[{"agent":"major","status":"running","startedAt":4,"messages":[{"role":"assistant","content":[{"type":"text","text":"Offline recovery evidence"}]}]}]}}}).to_string()).unwrap();
        let recovered = engine.request("workspace", "thread/resume", json!({"threadId":"stored"})).await.unwrap();
        assert_eq!(recovered["result"]["thread"]["turns"][1]["status"], "interrupted");
        assert!(recovered.to_string().contains("Offline recovery evidence"));
        assert!(engine.sessions.lock().await["stored"].process.is_none(), "recovery must not restart agent work or request credentials");
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
    assert!(args.contains(&"read,write,edit,ls,find,grep,bash,subagent,ask_user,preview_server,mcp__*".into()));
    assert!(args.contains(&"/synthetic/work/skills".into()));
    assert!(args.contains(&"--no-context-files".into()));
    assert!(args.contains(&"--no-skills".into()));
    assert!(args.contains(&WORK_PROMPT.into()));
    assert!(WORK_PROMPT.contains("outputs/") && WORK_PROMPT.contains("In Ask mode, ask before deleting or overwriting"));
    assert!(WORK_PROMPT.contains("Do not use ask_user to request tool permission"));
    assert!(command.as_std().get_envs().any(|(k, v)| k == "TAUCODE_WORK" && v == Some(std::ffi::OsStr::new("1"))));
    assert!(command.as_std().get_envs().any(|(k, v)| k == "TAUCODE_WORK_PYTHON" && v == Some(std::ffi::OsStr::new("/synthetic/work/.venv/bin/python"))));
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
    let mut engine = PiEngine::work(dir.join("data"), "workspace".into(), space.to_string_lossy().into_owned(), Arc::new(|_, _| {}));
    engine.previews = Some(crate::shared::managed_tasks::ManagedTasks::new());
    let provider: ProviderEntry = serde_json::from_value(json!({"id":"local","name":"Offline","kind":"ollama","enabled":true,"auth":{"type":"none"},"models":[{"id":"offline","enabled":true}]})).unwrap();
    let mut policy = AgentPolicy::default();
    policy.subagents.mode = crate::types::SubagentMode::ChainOfCommand;
    policy.subagents.enabled = true;
    engine.configure("workspace", vec![provider], policy).await;
    let response = engine.request("workspace", "thread/start", json!({"model":"local/offline"})).await.unwrap();
    let id = response["result"]["thread"]["id"].as_str().unwrap();
    assert!(engine.sessions.lock().await[id].process.is_none());
    let (registry, policy) = engine.registry.lock().await["workspace"].clone();
    let process = engine.spawn(engine.sessions.lock().await.get_mut(id).unwrap(), &registry, &policy).await.unwrap();
    let preview = process.preview.lock().await.clone().unwrap();
    assert!(preview.runtime.guard.as_ref().unwrap().1.is_file());
    let env: HashMap<_, _> = preview.runtime.env.as_ref().unwrap().iter().cloned().collect();
    assert_eq!(env["HOME"], dir.join("data/pi/agent/workspace").to_string_lossy());
    assert!(env.keys().all(|key| matches!(key.as_str(), "PATH" | "LANG" | "TMPDIR" | "SYSTEMROOT" | "WINDIR" | "TERM" | "HOME" | "XDG_CONFIG_HOME" | "OTEL_SDK_DISABLED" | "TAUCODE_WORK_PYTHON")));
    let delivered: Value = serde_json::from_slice(&std::fs::read(dir.join("data/pi/agent/workspace/taucode/policy.json")).unwrap()).unwrap();
    assert_eq!(delivered["enabled"], false);
    assert_eq!(delivered["mode"], "custom");
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
    let mut session = WorkspaceSession::lazy(Box::new(|_| Box::pin(async { Err("Codex must not be used".into()) })),
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
#[cfg(unix)]
async fn command_deadline_stops_a_real_delegated_shell_tree() {
    use tokio::io::AsyncBufReadExt;
    if !Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp/node_modules/.bin/bun").exists() { return; }
    let mut server = tokio::process::Command::new("node").arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/omp_fetch.mjs")).arg("--serve").stdout(std::process::Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    let port = tokio::io::BufReader::new(server.stdout.take().unwrap()).lines().next_line().await.unwrap().unwrap();
    let dir = temp();
    let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
    engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), "omp").unwrap().0;
    let provider = serde_json::from_value(json!({"id":"local","name":"Stub","kind":"openai-compatible","baseUrl":format!("http://127.0.0.1:{port}/v1"),"enabled":true,"auth":{"type":"none"},"models":[{"id":"stub","enabled":true,"contextWindow":8192}]})).unwrap();
    let policy = serde_json::from_value(json!({"subagents":{"mode":"chain-of-command","maxRunMinutes":1,"roles":[{"name":"enlisted","description":"Work","toolAccess":"full","instructions":""}]}})).unwrap();
    engine.configure("workspace", vec![provider], policy).await;
    let mut events = engine.events();
    let start = engine.request("workspace", "thread/start", json!({"model":"local/stub"})).await.unwrap();
    let id = start["result"]["thread"]["id"].as_str().unwrap();
    let started = std::time::Instant::now();
    engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"LIMIT_TIMER_ROOT"}],"accessMode":"full-access"})).await.unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !dir.join("timer.pid").exists() { tokio::time::sleep(Duration::from_millis(50)).await; }
    }).await.unwrap();
    let pid: i32 = std::fs::read_to_string(dir.join("timer.pid")).unwrap().parse().unwrap();
    tokio::time::timeout(Duration::from_secs(70), async {
        loop {
            let event = events.recv().await.unwrap();
            if event["method"] == "turn/completed" {
                assert_eq!(event["params"]["turn"]["status"], "interrupted", "{event}");
                assert!(event.to_string().contains("time limit"), "{event}");
                break;
            }
        }
    }).await.unwrap();
    assert!(started.elapsed() >= Duration::from_secs(59) && started.elapsed() < Duration::from_secs(75));
    tokio::time::timeout(Duration::from_secs(3), async {
        while unsafe { libc::kill(pid, 0) } == 0 { tokio::time::sleep(Duration::from_millis(30)).await; }
    }).await.expect("deadline must stop the delegated command's child process");
    engine.shutdown().await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn project_token_allowance_survives_engine_restart_and_blocks_new_provider_calls() {
    use tokio::io::AsyncBufReadExt;
    if !Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp/node_modules/.bin/bun").exists() { return; }
    let mut server = tokio::process::Command::new("node").arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/omp_fetch.mjs")).arg("--serve").stdout(std::process::Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    let port = tokio::io::BufReader::new(server.stdout.take().unwrap()).lines().next_line().await.unwrap().unwrap();
    let dir = temp();
    for profile in ["pi", "omp"] {
        let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
        engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), profile).unwrap().0;
        let provider = serde_json::from_value(json!({"id":"local","name":"Stub","kind":"openai-compatible","baseUrl":format!("http://127.0.0.1:{port}/v1"),"enabled":true,"auth":{"type":"none"},"models":[{"id":"stub","enabled":true,"contextWindow":8192}]})).unwrap();
        let policy = serde_json::from_value(json!({"subagents":{"mode":"chain-of-command","dailyTokenLimit":1}})).unwrap();
        engine.configure("workspace", vec![provider], policy).await;
        let mut events = engine.events();
        let start = engine.request("workspace", "thread/start", json!({"model":"local/stub"})).await.unwrap();
        let id = start["result"]["thread"]["id"].as_str().unwrap();
        engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"QA_MISSING_FIXTURE"}],"accessMode":"full-access"})).await.unwrap();
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let event = events.recv().await.unwrap();
                if event["method"] == "turn/completed" {
                    assert_eq!(event["params"]["turn"]["status"], "interrupted", "{profile}: {event}");
                    assert!(event.to_string().contains("daily token allowance"), "{profile}: {event}");
                    break;
                }
            }
        }).await.unwrap();
        let file = engine.sessions.lock().await[id].file.clone();
        let (_, restored) = read_session(&file, &[]).unwrap();
        assert_eq!(restored.turns.last().unwrap()["status"], "interrupted");
        assert!(json!(restored.turns).to_string().contains("daily token allowance"));
        let count = reqwest::get(format!("http://127.0.0.1:{port}/count")).await.unwrap().text().await.unwrap();
        assert_eq!(count, "1", "an exhausted project allowance must block the restarted engine's first provider request");
        engine.shutdown().await.unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn execution_limit_replay_keeps_the_reason_on_its_own_turn() {
    let history = Translator::history("thread", "/workspace", &[
        json!({"role":"user","timestamp":1,"content":"First"}),
        json!({"role":"assistant","timestamp":2,"stopReason":"toolUse","content":[]}),
        json!({"role":"taucodeExecutionLimit","record":{"message":"First allowance","beforeUser":false}}),
        json!({"role":"user","timestamp":3,"content":"Second"}),
        json!({"role":"assistant","timestamp":4,"stopReason":"stop","content":[{"type":"text","text":"OK"}]}),
        json!({"role":"taucodeExecutionLimit","record":{"message":"Third allowance","beforeUser":true}}),
        json!({"role":"user","timestamp":5,"content":"Third"}),
    ]);
    assert_eq!(history.turns.len(), 3);
    for (index, status) in ["interrupted", "completed", "interrupted"].iter().enumerate() {
        assert_eq!(history.turns[index]["status"], *status);
    }
    assert!(history.turns[0].to_string().contains("First allowance"));
    assert!(!history.turns[1].to_string().contains("allowance"));
    assert!(history.turns[2].to_string().contains("Third allowance"));
}

#[tokio::test]
async fn real_pi_and_omp_verification_gates_completion_and_survives_reload() {
    use tokio::io::AsyncBufReadExt;
    if !Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/omp/node_modules/.bin/bun").exists() { return; }
    let mut server = tokio::process::Command::new("node").arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/omp_fetch.mjs")).arg("--serve").stdout(std::process::Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    let port = tokio::io::BufReader::new(server.stdout.take().unwrap()).lines().next_line().await.unwrap().unwrap();
    for profile in ["pi", "omp"] {
        let dir = temp();
        std::fs::write(dir.join("answer.txt"), "42\n").unwrap();
        let mut engine = PiEngine::new(dir.clone(), "workspace".into(), dir.to_string_lossy().into(), None, Arc::new(|_,_| {}));
        engine.profile.definition = crate::shared::engine_registry::resolve(&crate::types::AppSettings::default(), profile).unwrap().0;
        let provider = serde_json::from_value(json!({"id":"local","name":"Stub","kind":"openai-compatible","baseUrl":format!("http://127.0.0.1:{port}/v1"),"enabled":true,"auth":{"type":"none"},"models":[{"id":"stub","enabled":true,"contextWindow":8192}]})).unwrap();
        let policy = serde_json::from_value(json!({"subagents":{"mode":"chain-of-command","roles":[{"name":"major","description":"QA","toolAccess":"read-only","instructions":""}]}})).unwrap();
        engine.configure("workspace", vec![provider], policy).await;
        let mut events = engine.events();
        for (marker, expected) in [("QA_MISSING_FIXTURE", "failed"), ("QA_PASS_FIXTURE", "completed"), ("QA_FAIL_FIXTURE", "failed")] {
            let start = engine.request("workspace", "thread/start", json!({"model":"local/stub"})).await.unwrap();
            let id = start["result"]["thread"]["id"].as_str().unwrap();
            engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":marker}],"accessMode":"full-access"})).await.unwrap();
            let mut verification = false;
            tokio::time::timeout(Duration::from_secs(45), async {
                loop {
                    let event = events.recv().await.unwrap();
                    if event["method"] == "item/completed" && event["params"]["item"]["text"].as_str().is_some_and(|text| text.contains("Chain of Command verification")) { verification = true; }
                    if event["method"] == "turn/completed" {
                        assert_eq!(event["params"]["turn"]["status"], expected, "{profile} {marker}: {event}");
                        assert!(verification, "QA status must arrive before turn completion: {profile} {marker} {event}");
                        break;
                    }
                }
            }).await.unwrap();
            let file = engine.sessions.lock().await[id].file.clone();
            let (_, restored) = read_session(&file, &[]).unwrap();
            assert_eq!(restored.turns.last().unwrap()["status"], expected, "reloaded {marker}");
            assert!(json!(restored.turns).to_string().contains("Chain of Command verification"));
        }
        engine.shutdown().await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
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
    if cfg!(unix) {
        engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"STREAM_CRASH_FIXTURE"}],"accessMode":"read-only"})).await.unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            while events.recv().await.unwrap()["method"] != "turn/completed" {}
        }).await.unwrap();
        let file = engine.sessions.lock().await[id].file.clone();
        let (_, recovered) = read_session(&file, &[]).unwrap();
        assert_eq!(recovered.turns.last().unwrap()["status"], "interrupted");
        assert!(json!(recovered.turns).to_string().contains("ROOT_PARTIAL"), "real OMP extension must save streaming evidence before termination");
        assert!(!std::fs::read_to_string(&file).unwrap().contains("ROOT_PARTIAL"), "partial output stays outside model context");
        engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"Say hello again"}],"accessMode":"read-only"})).await.unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let event = events.recv().await.unwrap();
                if event["method"] == "turn/completed" { assert_eq!(event["params"]["turn"]["status"], "completed", "{event}"); break; }
            }
        }).await.unwrap();
        let (_, followed_up) = read_session(&file, &[]).unwrap();
        assert_eq!(followed_up.turns.len(), 3);
        assert_eq!(followed_up.turns[1]["status"], "interrupted");
        assert_eq!(followed_up.turns[2]["status"], "completed");
        #[cfg(unix)] {
            engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"GUARD_NATIVE_FIXTURE"}],"accessMode":"full-access"})).await.unwrap();
            tokio::time::timeout(Duration::from_secs(15), async {
                while events.recv().await.unwrap()["method"] != "turn/completed" {}
            }).await.unwrap();
            let (_, recovered) = read_session(&file, &[]).unwrap();
            let turn = recovered.turns.last().unwrap();
            assert_eq!(turn["status"], "interrupted");
            let output = turn["items"].as_array().unwrap().iter().find_map(|item| item["aggregatedOutput"].as_str()).unwrap();
            let pid: i32 = output.trim().strip_prefix("GUARD_NATIVE_READY ").expect(output).parse().unwrap();
            tokio::time::timeout(Duration::from_secs(4), async {
                while unsafe { libc::kill(pid, 0) } == 0 { tokio::time::sleep(Duration::from_millis(30)).await; }
            }).await.expect("native OMP foreground command survived engine SIGKILL");
            engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"Say hello after supervisor"}],"accessMode":"read-only"})).await.unwrap();
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let event = events.recv().await.unwrap();
                    if event["method"] == "turn/completed" { assert_eq!(event["params"]["turn"]["status"], "completed"); break; }
                }
            }).await.unwrap();
            let (_, final_history) = read_session(&file, &[]).unwrap();
            assert_eq!(final_history.turns.len(), 5);
            assert_eq!(final_history.turns[3]["status"], "interrupted");
            assert_eq!(final_history.turns[4]["status"], "completed");
            engine.request("workspace", "turn/start", json!({"threadId":id,"input":[{"type":"text","text":"GUARD_STOP_FIXTURE"}],"accessMode":"full-access"})).await.unwrap();
            let pid: i32 = tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let event = events.recv().await.unwrap();
                    if event["method"] == "item/commandExecution/outputDelta" {
                        if let Some(pid) = event["params"]["delta"].as_str().and_then(|s| s.trim().strip_prefix("GUARD_NATIVE_READY ")) { break pid.parse().unwrap(); }
                    }
                }
            }).await.unwrap();
            engine.request("workspace", "turn/interrupt", json!({"threadId":id})).await.unwrap();
            tokio::time::timeout(Duration::from_secs(15), async {
                while events.recv().await.unwrap()["method"] != "turn/completed" {}
            }).await.unwrap();
            tokio::time::timeout(Duration::from_secs(4), async {
                while unsafe { libc::kill(pid, 0) } == 0 { tokio::time::sleep(Duration::from_millis(30)).await; }
            }).await.expect("native OMP Stop left its foreground command alive");
            assert_eq!(read_session(&file, &[]).unwrap().1.turns.last().unwrap()["status"], "interrupted");
        }
    }
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

#[tokio::test]
#[cfg(unix)]
async fn managed_preview_bridge_serves_restarts_and_stops_without_user_dialogs() {
    use crate::shared::managed_tasks::{ManagedTasks, TaskRuntime};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = temp();
    std::fs::write(root.join("package.json"), "{}").unwrap();
    config::prepare(&root, &[], None).unwrap();
    let guard = root.join("extensions/lib/commandGuard.ts");
    let access = root.join("access.json");
    std::fs::write(&access, r#"{"kind":"work","accessMode":"full-access"}"#).unwrap();
    let manager = ManagedTasks::new();
    let node = crate::shared::engine_runtime::node_path();
    let preview = Arc::new(preview::Preview { manager: manager.clone(), data: root.clone(), access: access.clone(), workspace: "work".into(), thread: "fake-thread".into(), cwd: root.clone(),
        runtime: TaskRuntime { guard: Some((node, guard)), env: Some(crate::shared::engine_registry::environment(std::env::vars(), &root).into_iter().chain([("TAUCODE_PREVIEW_TEST".into(), "owned ✓".into())]).collect()) }, secrets: vec!["secret-canary".into()] });
    let (process, events) = fake(Duration::from_secs(600), &root.join("eof")).await;
    *process.preview.lock().await = Some(preview.clone());
    let script = root.join("server.cjs");
    std::fs::write(&script, r#"const http = require('node:http'); const server = http.createServer((_, res) => res.end(process.env.TAUCODE_PREVIEW_TEST)); server.listen(0, '127.0.0.1', () => console.log('http://127.0.0.1:' + server.address().port + '/ secret-canary'));"#).unwrap();
    let message = |id: &str, params: Value| json!({"type":"extension_ui_request","method":"select","id":id,"title":format!("taucode:preview:{params}"),"options":["Continue"]});
    process.receive(message("start", json!({"action":"start","command":"node server.cjs","label":"Preview","workspace":"unowned","cwd":"/"}))).await;
    let wait_url = || async {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let tasks = preview.request(json!({"action":"list"})).await.unwrap();
                if tasks.as_array().unwrap().iter().any(|t| t["status"] == "running" && t["urls"][0].is_string()) { break tasks; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap()
    };
    let tasks = wait_url().await;
    assert!(!tasks[0]["output"].as_str().unwrap().contains("MODULE_TYPELESS_PACKAGE_JSON"));
    let id = tasks[0]["id"].as_str().unwrap().to_string();
    assert_eq!(tasks[0]["workspaceId"], "work");
    assert_eq!(tasks[0]["cwd"], root.canonicalize().unwrap().to_string_lossy().as_ref());
    let url = tasks[0]["urls"][0].as_str().unwrap();
    let addr = url.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut body = String::new(); stream.read_to_string(&mut body).await.unwrap();
    assert!(body.contains("owned ✓"));
    assert!(manager.request_owned("work", "other", &root, "stop", json!({"id":id}), Default::default()).await.is_err());
    process.receive(message("list", json!({"action":"list"}))).await;
    let records = root.join("eof.requests");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(&records).unwrap_or_default().contains("[REDACTED]") { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    assert!(!std::fs::read_to_string(&records).unwrap().contains("secret-canary"));
    assert!(events.lock().unwrap().iter().all(|e| e["method"] != "item/tool/requestUserInput"));
    // Late requests must not launch a command after cancellation.
    process.request(json!({"type":"abort"})).await.unwrap();
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    process.receive(message("late", json!({"action":"start","command":"touch late-start"}))).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(&records).unwrap_or_default().contains("\"id\": \"late\"") { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    assert!(!root.join("late-start").exists());
    assert_eq!(preview.request(json!({"action":"list"})).await.unwrap()[0]["status"], "stopped");
    // A restarted preview retains the Work environment and survives engine shutdown.
    manager.request_owned("work", "fake-thread", &root, "restart", json!({"id":id}), Default::default()).await.unwrap();
    let restarted = wait_url().await;
    assert_eq!(restarted.as_array().unwrap().len(), 2);
    process.shutdown().await.unwrap();
    assert_eq!(preview.request(json!({"action":"list"})).await.unwrap()[0]["status"], "running");
    std::fs::write(&access, r#"{"kind":"work","accessMode":"read-only"}"#).unwrap();
    assert!(preview.request(json!({"action":"start","command":"touch read-only"})).await.unwrap_err().contains("write access"));
    std::fs::write(&access, r#"{"kind":"code","accessMode":"full-access"}"#).unwrap();
    assert!(preview.request(json!({"action":"start","command":"touch wrong-mode"})).await.is_err());
    std::fs::write(&access, r#"{"kind":"work","accessMode":"full-access"}"#).unwrap();
    std::fs::write(root.join("settings.json"), r#"{"features":{"terminal":false}}"#).unwrap();
    assert!(preview.request(json!({"action":"start","command":"touch disabled"})).await.is_err());
    assert!(preview.request(json!({"action":"restart","id":id})).await.is_err());
    preview.stop().await.unwrap();
    manager.shutdown().await.unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
