use super::bridge::{Bridge, Transport};
use super::*;
use std::{collections::VecDeque, sync::Mutex as StdMutex};

#[tokio::test]
async fn pinned_playwright_lists_verified_schemas_without_a_browser_or_sockets() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let cli = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../engines/browser/node_modules/@playwright/mcp/cli.js");
    let data = std::env::temp_dir().join(format!("browser-mcp-{}", uuid::Uuid::new_v4()));
    let mut child = launcher::spawn(&cli, &data, 1, &Settings::default()).unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    input.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"taucode-test","version":"1"}}})).as_bytes()).await.unwrap();
    let initialized = tokio::time::timeout(Duration::from_secs(15), output.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&initialized).unwrap()["id"],
        1
    );
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n").await.unwrap();
    let listed = tokio::time::timeout(Duration::from_secs(15), output.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let listed: Value = serde_json::from_str(&listed).unwrap();
    let tools = listed["result"]["tools"].as_array().unwrap();
    let click = tools.iter().find(|t| t["name"] == "browser_click").unwrap();
    assert!(click["inputSchema"]["properties"].get("target").is_some());
    assert!(click["inputSchema"]["properties"].get("ref").is_none());
    assert!(tools.iter().any(|t| t["name"] == "browser_snapshot"));
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    std::fs::remove_dir_all(data).unwrap();
}

struct Fake {
    sent: Arc<StdMutex<Vec<Value>>>,
    replies: VecDeque<Value>,
}

#[tokio::test]
async fn shutdown_prevents_queued_requests_from_relaunching_chrome() {
    let browser = Browser::new(PathBuf::from("/unused-browser-data"));
    browser.shutdown().await;
    let error = browser
        .local_request(json!({"action":"ensure"}))
        .await
        .unwrap_err();
    assert_eq!(error, "Browser host is shutting down");
    assert!(browser.running.lock().await.is_none());
    browser.shutdown().await; // Repeated shutdown (signal, then app exit) is harmless.
}

#[tokio::test]
async fn crashed_process_or_bridge_is_replaced_and_healthy_runtime_is_reused() {
    struct Runtime {
        alive: bool,
        connected: bool,
        stops: Arc<StdMutex<u32>>,
    }
    impl Managed for Runtime {
        fn healthy(&mut self) -> bool {
            self.alive && self.connected
        }
        async fn stop(&mut self) {
            *self.stops.lock().unwrap() += 1;
        }
    }
    let stops = Arc::new(StdMutex::new(0));
    let starts = Arc::new(StdMutex::new(0));
    let launch = || async {
        *starts.lock().unwrap() += 1;
        Ok(Runtime {
            alive: true,
            connected: true,
            stops: stops.clone(),
        })
    };
    let mut running = None;
    ensure_running(&mut running, launch).await.unwrap();
    ensure_running(&mut running, launch).await.unwrap();
    assert_eq!(*starts.lock().unwrap(), 1);
    running.as_mut().unwrap().alive = false;
    ensure_running(&mut running, launch).await.unwrap();
    assert_eq!(*starts.lock().unwrap(), 2);
    assert_eq!(*stops.lock().unwrap(), 1);
    running.as_mut().unwrap().connected = false;
    ensure_running(&mut running, launch).await.unwrap();
    assert_eq!(*starts.lock().unwrap(), 3);
    assert_eq!(*stops.lock().unwrap(), 2);
    running.as_mut().unwrap().alive = false;
    assert!(
        ensure_running(&mut running, || async { Err("cannot restart".into()) })
            .await
            .is_err()
    );
    assert!(running.is_none());
}
impl Transport for Fake {
    async fn send(&mut self, v: Value) -> Result<(), String> {
        self.sent.lock().unwrap().push(v.clone());
        let result = match v["method"].as_str().unwrap() {
            "Target.getTargets" => {
                json!({"targetInfos":[{"targetId":"one","type":"page","url":"https://one.test","title":"One"},{"targetId":"two","type":"page","url":"https://two.test","title":"Two"},{"targetId":"worker","type":"service_worker"}]})
            }
            "Target.attachToTarget" => {
                json!({"sessionId":format!("session-{}",v["params"]["targetId"].as_str().unwrap())})
            }
            "Page.captureScreenshot" => json!({"data":"first-frame"}),
            "Runtime.evaluate" if v["params"]["expression"] == "document.hasFocus()" => {
                json!({"result":{"value":v["sessionId"] == "session-two"}})
            }
            "Page.getNavigationHistory" => {
                json!({"currentIndex":1,"entries":[{"id":10},{"id":20},{"id":30}]})
            }
            "Target.createTarget" => json!({"targetId":"new"}),
            _ => json!({}),
        };
        self.replies
            .push_back(json!({"id":v["id"],"result":result}));
        Ok(())
    }
    async fn receive(&mut self) -> Result<Value, String> {
        self.replies
            .pop_front()
            .ok_or("fake peer disconnected".into())
    }
}
#[tokio::test]
async fn bridge_tracks_tabs_streams_and_navigates_without_sockets() {
    let sent = Arc::new(StdMutex::new(vec![]));
    let (state, _) = watch::channel(json!({}));
    let mut bridge = Bridge::new(
        Fake {
            sent: sent.clone(),
            replies: VecDeque::new(),
        },
        state.clone(),
    );
    bridge.initialize("/home/test/Downloads").await.unwrap();
    assert_eq!(state.borrow()["tabs"].as_array().unwrap().len(), 2);
    bridge.event(json!({"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"two","type":"page","url":"https://one.test","title":"Duplicate URL"}}})).await.unwrap();
    bridge
        .command(json!({"action":"agentTab","url":"https://one.test"}))
        .await
        .unwrap();
    assert_eq!(state.borrow()["active"], "two");
    bridge
        .command(json!({"action":"activate","id":"one"}))
        .await
        .unwrap();
    bridge
        .command(json!({"action":"attach","width":800,"height":600,"dpr":2}))
        .await
        .unwrap();
    assert_eq!(state.borrow()["frame"], "first-frame");
    bridge
        .command(json!({"action":"activate","id":"two"}))
        .await
        .unwrap();
    assert_eq!(state.borrow()["active"], "two");
    bridge.event(json!({"method":"Page.screencastFrame","sessionId":"session-one","params":{"sessionId":7,"data":"stale"}})).await.unwrap();
    assert_eq!(state.borrow()["frame"], "first-frame");
    bridge.event(json!({"method":"Page.screencastFrame","sessionId":"session-two","params":{"sessionId":8,"data":"live"}})).await.unwrap();
    assert_eq!(state.borrow()["frame"], "live");
    bridge.event(json!({"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"two","type":"page","url":"https://two.test/new","title":"Updated"}}})).await.unwrap();
    assert!(state.borrow()["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["title"] == "Updated"));
    bridge
        .event(json!({"method":"Target.targetDestroyed","params":{"targetId":"two"}}))
        .await
        .unwrap();
    assert_eq!(state.borrow()["active"], "one");
    for action in ["back", "forward", "reload", "stop", "new"] {
        bridge.command(json!({"action":action})).await.unwrap();
    }
    bridge
        .command(json!({"action":"navigate","url":"https://example.test"}))
        .await
        .unwrap();
    assert!(bridge
        .command(json!({"action":"navigate","url":"javascript:alert(1)"}))
        .await
        .is_err());
    bridge
        .command(json!({"action":"close","id":"one"}))
        .await
        .unwrap();
    bridge.command(json!({"action":"detach"})).await.unwrap();
    let calls = sent.lock().unwrap();
    for id in [7, 8] {
        assert!(calls
            .iter()
            .any(|v| v["method"] == "Page.screencastFrameAck" && v["params"]["sessionId"] == id));
    }
    let methods: Vec<_> = calls
        .iter()
        .map(|v| v["method"].as_str().unwrap())
        .collect();
    assert!(
        methods.iter().position(|m| *m == "Page.enable").unwrap()
            < methods
                .iter()
                .position(|m| *m == "Page.startScreencast")
                .unwrap()
    );
    assert!(methods
        .windows(2)
        .any(|w| w == ["Page.stopScreencast", "Target.detachFromTarget"]));
    assert!(calls
        .iter()
        .any(|v| v["method"] == "Page.startScreencast" && v["params"]["maxWidth"] == 1600));
    assert!(calls
        .iter()
        .any(|v| v["method"] == "Emulation.setDeviceMetricsOverride"
            && v["params"]["deviceScaleFactor"] == 2.));
    for id in [10, 30] {
        assert!(calls
            .iter()
            .any(|v| v["method"] == "Page.navigateToHistoryEntry" && v["params"]["entryId"] == id));
    }
}
#[test]
fn launch_paths_arguments_and_validation() {
    let settings = Settings {
        binary: Some("/custom/chrome".into()),
        ..Default::default()
    };
    assert_eq!(
        process::discover(&settings, |_| true).unwrap(),
        Path::new("/custom/chrome")
    );
    assert!(process::discover(&settings, |_| false)
        .unwrap_err()
        .contains("No browser found"));
    assert_eq!(
        process::profile(Path::new("/data")),
        Path::new("/data/browser/profile")
    );
    let args = process::arguments(Path::new("/data"), 800, 600);
    assert!(args.contains(&"--remote-debugging-port=0".into()));
    assert!(args.contains(&"--window-size=800,600".into()));
    assert!(!args.iter().any(|a| a.contains("remote-allow-origins")));
    assert_eq!(
        process::parse_port("1234\n/devtools/browser/abc\n").unwrap(),
        (1234, "ws://127.0.0.1:1234/devtools/browser/abc".into())
    );
    for text in [
        "0\n/devtools/browser/a",
        "65536\n/devtools/browser/a",
        "123\nws://evil",
        "123",
        "junk",
    ] {
        assert!(process::parse_port(text).is_err());
    }
    let settings = Settings {
        allowed_origins: vec!["https://a.test".into(), "https://b.test".into()],
        blocked_origins: vec!["https://blocked.test".into()],
        ..Default::default()
    };
    let args = launcher::arguments(
        Path::new("/runtime/cli.js"),
        Path::new("/app data"),
        1234,
        &settings,
    );
    // Both schema-only startup and the real CDP connection use this command.
    for port in [1, 1234] {
        let command = launcher::command(
            Path::new("/runtime/cli.js"),
            Path::new("/app data"),
            port,
            &settings,
        );
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(Path::new("/app data/browser/output"))
        );
        assert_eq!(
            command
                .as_std()
                .get_args()
                .map(|s| s.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            launcher::arguments(
                Path::new("/runtime/cli.js"),
                Path::new("/app data"),
                port,
                &settings
            )
        );
    }
    assert_eq!(
        args,
        [
            "/runtime/cli.js",
            "--cdp-endpoint",
            "http://127.0.0.1:1234",
            "--output-dir",
            "/app data/browser/output",
            "--allowed-origins",
            "https://a.test;https://b.test",
            "--blocked-origins",
            "https://blocked.test"
        ]
    );
    assert!(validate(&settings).is_ok());
    assert!(validate(&Settings {
        search_url: "javascript:{query}".into(),
        ..Default::default()
    })
    .is_err());
    assert_eq!(
        launcher::current_url(
            &json!({"content":[{"type":"text","text":"- 0: [A](https://a.test)\n- 1: (current) [B](https://b.test)"}]})
        ),
        Some("https://b.test".into())
    );
}
#[test]
fn input_mapping_and_shortcuts() {
    for (button, name) in [
        (0, "left"),
        (1, "middle"),
        (2, "right"),
        (3, "back"),
        (4, "forward"),
        (-1, "none"),
    ] {
        let (method,p) = bridge::input(&json!({"action":"mouse","type":"mousePressed","button":button,"x":10,"y":20,"buttons":1,"clickCount":2,"modifiers":9})).unwrap();
        assert_eq!(method, "Input.dispatchMouseEvent");
        assert_eq!(p["button"], name);
        assert_eq!(p["modifiers"], 9);
        assert_eq!(p["clickCount"], 2);
    }
    let (_, p) =
        bridge::input(&json!({"action":"mouse","type":"mouseWheel","x":0,"y":0,"deltaY":120}))
            .unwrap();
    assert_eq!(p["deltaY"], 120.);
    let (_,p) = bridge::input(&json!({"action":"key","type":"keyDown","key":"a","code":"KeyA","modifiers":4,"mac":true,"text":"a"})).unwrap();
    assert_eq!(p["commands"], json!(["selectAll"]));
    assert!(p.get("text").is_none());
    let (_, p) = bridge::input(
        &json!({"action":"key","type":"keyDown","key":"a","code":"KeyA","modifiers":2,"text":"a"}),
    )
    .unwrap();
    assert_eq!(p["modifiers"], 2);
    assert!(p.get("text").is_none());
    assert_eq!(
        bridge::input(&json!({"action":"text","text":"日本語"})).unwrap(),
        ("Input.insertText", json!({"text":"日本語"}))
    );
}

#[test]
fn cancellation_drops_a_tool_waiting_for_tab_selection() {
    let mut pending = std::collections::HashMap::from([
        (
            "internal-1".into(),
            json!({"id":7,"method":"tools/call","params":{"name":"browser_click"}}).to_string(),
        ),
        (
            "internal-2".into(),
            json!({"id":8,"method":"tools/call"}).to_string(),
        ),
    ]);
    assert_eq!(
        launcher::cancel_pending(&mut pending, &json!(7)),
        ["internal-1"]
    );
    assert!(!pending.contains_key("internal-1"));
    assert!(pending.contains_key("internal-2"));
    assert!(launcher::cancel_pending(&mut pending, &json!("8")).is_empty());
}
