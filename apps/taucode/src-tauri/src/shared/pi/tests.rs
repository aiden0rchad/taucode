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
    let (failed, errors) = replay("rpc_compact_too_small.jsonl");
    assert_eq!(failed.turns.last().unwrap()["status"], "failed");
    assert!(errors.iter().any(|e| e["method"] == "error"
        && e["params"]["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Nothing to compact")));
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
            let env = config::prepare(&dir, &registry, None, &Keys).unwrap();
            if auth["type"] == "apiKey" {
                assert_eq!(env["TAUCODE_PROVIDER_LOCAL_KEY"], "keychain-secret-canary");
                assert_eq!(
                    models["providers"][&id]["apiKey"],
                    "$TAUCODE_PROVIDER_LOCAL_KEY"
                );
            }
            if auth["type"] == "command" {
                let cmd = models["providers"][&id]["apiKey"]
                    .as_str()
                    .unwrap()
                    .strip_prefix('!')
                    .unwrap();
                assert_eq!(
                    shell_words::split(cmd).unwrap(),
                    vec!["/path with spaces/key", "a'b", "$(never)"]
                );
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
    let next = engine.sessions.lock().await[id].process.clone().unwrap();
    assert!(!Arc::ptr_eq(&original, &next));
    assert_eq!(engine.sessions.lock().await[id].file, file);
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
