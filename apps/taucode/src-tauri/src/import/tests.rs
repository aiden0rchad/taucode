use super::*;
use crate::shared::{engines::EngineBackend, pi::PiEngine};

struct Fixture {
    root: PathBuf,
    service: Arc<Imports>,
    ws: HashMap<String, WorkspaceEntry>,
    settings: AppSettings,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("import-{}", uuid::Uuid::new_v4()));
        let cwd = root.join("project");
        fs::create_dir_all(&cwd).unwrap();
        let roots = Roots {
            codex: root.join("source/codex"),
            claude: root.join("source/claude"),
            desktop: root.join("source/desktop"),
        };
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/import");
        for (source, destination) in [
            (
                "codex/rollout.jsonl",
                roots.codex.join("sessions/2026/10/24/rollout-clock.jsonl"),
            ),
            ("codex/config.toml", roots.codex.join("config.toml")),
            (
                "claude/local_code.json",
                roots
                    .desktop
                    .join("claude-code-sessions/acct/org/local_code.json"),
            ),
            (
                "claude/transcript.jsonl",
                roots
                    .claude
                    .join("projects/encoded/22222222-2222-4222-8222-222222222222.jsonl"),
            ),
            (
                "claude/claude_desktop_config.json",
                roots.desktop.join("claude_desktop_config.json"),
            ),
            (
                "cowork/local_task.json",
                roots
                    .desktop
                    .join("local-agent-mode-sessions/acct/org/local_task.json"),
            ),
            (
                "cowork/audit.jsonl",
                roots
                    .desktop
                    .join("local-agent-mode-sessions/acct/org/local_task/audit.jsonl"),
            ),
            (
                "cowork/outputs/report.txt",
                roots
                    .desktop
                    .join("local-agent-mode-sessions/acct/org/local_task/outputs/report.txt"),
            ),
        ] {
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(
                destination,
                fs::read_to_string(fixture.join(source))
                    .unwrap()
                    .replace("@CWD@", cwd.to_str().unwrap()),
            )
            .unwrap();
        }
        for path in [
            roots.codex.join("auth.json"),
            roots.claude.join(".credentials.json"),
            roots.desktop.join("buddy-tokens.json"),
            roots.desktop.join("Cookies"),
            roots.desktop.join("config.json"),
        ] {
            fs::write(path, "credential-sentinel").unwrap();
        }
        // Database created in the test, never shipped as an opaque binary fixture.
        let sql = format!("CREATE TABLE projects(id TEXT,name TEXT,metadata TEXT,position INTEGER,created_at_ms INTEGER,updated_at_ms INTEGER); CREATE TABLE project_roots(project_id TEXT,position INTEGER,path TEXT); CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,title TEXT,name TEXT,created_at_ms INTEGER,updated_at_ms INTEGER,archived INTEGER,model TEXT,model_provider TEXT,project_id TEXT,first_user_message TEXT,preview TEXT); INSERT INTO projects VALUES('p','Clock project','{{}}',0,0,0); INSERT INTO project_roots VALUES('p',0,'{}'); INSERT INTO project_roots VALUES('p',1,'/synthetic-missing-root'); INSERT INTO threads(id,cwd,title,archived,project_id) VALUES('11111111-1111-4111-8111-111111111111','{}','Database title',0,'p');",cwd.display(),cwd.display());
        assert!(std::process::Command::new("sqlite3")
            .arg(roots.codex.join("state_5.sqlite"))
            .arg(sql)
            .status()
            .unwrap()
            .success());
        let service = Imports::with_roots(root.join("app"), roots);
        Self {
            root,
            service,
            ws: HashMap::new(),
            settings: AppSettings::default(),
        }
    }
    fn call(&mut self, action: &str, params: Value) -> Value {
        self.service
            .perform(action, params, &mut self.ws, &mut self.settings)
            .unwrap()
    }
    fn detect_and_import(&mut self) -> Value {
        let detected = self.call("detect", json!({}));
        self.call("run",json!({"keys":detected["entries"].as_array().unwrap().iter().map(|e|e["key"].clone()).collect::<Vec<_>>()}))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn synthetic_sources_are_allowlisted_sanitized_idempotent_and_undoable() {
    let mut f = Fixture::new();
    io::OPENS.with(|v| v.borrow_mut().clear());
    let before = fs::read(
        f.service
            .roots
            .codex
            .join("sessions/2026/10/24/rollout-clock.jsonl"),
    )
    .unwrap();
    let log = f.detect_and_import();
    assert_eq!(f.ws.len(), 1);
    assert!(log["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().contains("Missing root")));
    let opens = io::OPENS.with(|v| v.borrow().clone());
    assert!(opens.iter().all(|p| ![
        "auth.json",
        ".credentials.json",
        "buddy-tokens.json",
        "Cookies",
        "config.json"
    ]
    .contains(&p.file_name().unwrap().to_str().unwrap())));
    assert!(opens.iter().all(|p| p.starts_with(&f.root)));
    let receipt: Log = serde_json::from_value(log.clone()).unwrap();
    let mut output = String::new();
    for r in &receipt.created {
        for file in r.files.keys() {
            output.push_str(&String::from_utf8_lossy(
                &fs::read(f.service.data.join(file)).unwrap(),
            ));
        }
    }
    output.push_str(&serde_json::to_string(&f.settings.mcp_servers).unwrap());
    assert!(!output.contains("private-user"));
    assert!(!output.contains("private-account"));
    assert!(!output.contains("private@example"));
    assert!(!output.contains("never-copy"));
    assert!(output.contains("Keep this unknown kind"));
    assert!(output.contains("Synthetic clock report"));
    let imported: Vec<_> = f
        .settings
        .mcp_servers
        .iter()
        .filter(|c| c.id.starts_with("import-"))
        .collect();
    assert_eq!(imported.len(), 3);
    assert!(imported.iter().all(|c| !c.enabled));
    assert!(serde_json::to_string(&imported)
        .unwrap()
        .contains("\"secret\":true"));
    assert!(serde_json::to_string(&imported)
        .unwrap()
        .contains("\"USER\":{\"secret\":true}"));
    assert_eq!(
        f.detect_and_import()["created"].as_array().unwrap().len(),
        0
    );
    let undone = f.call("undo", json!({"logId":log["id"]}));
    assert_eq!(undone["undone"], true);
    assert!(f.ws.is_empty());
    for r in receipt.created {
        for file in r.files.keys() {
            assert!(!f.service.data.join(file).exists());
        }
    }
    assert_eq!(
        fs::read(
            f.service
                .roots
                .codex
                .join("sessions/2026/10/24/rollout-clock.jsonl")
        )
        .unwrap(),
        before
    );
    assert!(!f.detect_and_import()["created"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn missing_cwd_history_changed_files_and_continuation_require_confirmation() {
    let mut f = Fixture::new();
    fs::remove_dir(&f.root.join("project")).unwrap();
    let log = f.detect_and_import();
    assert!(f.ws.is_empty());
    assert!(!f.service.data.join("engines/codex-home").exists());
    let record = log["created"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"].as_str().unwrap().starts_with("codex:"))
        .unwrap();
    let params = json!({"logId":log["id"],"key":record["key"]});
    assert_eq!(f.call("read", params.clone())["readOnly"], true);
    let child = f.call("continue", params);
    let confirmation = f.call("undo", json!({"logId":log["id"]}));
    assert!(confirmation["needsConfirmation"].is_array());
    f.call("undo", json!({"logId":log["id"],"confirmChanged":true}));
    assert!(f
        .service
        .data
        .join(format!(
            "chats/sessions/chats/{}.jsonl",
            child["threadId"].as_str().unwrap()
        ))
        .exists());

    let log = f.detect_and_import();
    let receipt: Log = serde_json::from_value(log.clone()).unwrap();
    let file = receipt
        .created
        .iter()
        .flat_map(|r| r.files.keys())
        .next()
        .unwrap();
    fs::write(f.service.data.join(file), "continued history").unwrap();
    assert!(f.call("undo", json!({"logId":log["id"]}))["needsConfirmation"].is_array());
}

#[tokio::test]
async fn exports_follow_main_branch_and_are_readable_by_pi_chat() {
    let mut f = Fixture::new();
    for source in ["claude-ai", "chatgpt"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/import/{source}/conversations.json"));
        let detection = f.call("export", json!({"path":path}));
        let log = f.call("run", json!({"keys":[detection["entries"][0]["key"]]}));
        let id = log["created"][0]["threadId"].as_str().unwrap();
        let engine = PiEngine::chat(f.service.data.clone(), Arc::new(|_, _| {}));
        let response = engine
            .request("chats", "thread/read", json!({"threadId":id}))
            .await
            .unwrap();
        let text = response.to_string();
        assert!(text.contains(if source == "chatgpt" {
            "Chosen answer"
        } else {
            "Preferred text blocks"
        }));
        assert!(!text.contains("Never import this branch"));
        let bytes = fs::read_to_string(
            f.service
                .data
                .join(format!("chats/sessions/chats/{id}.jsonl")),
        )
        .unwrap();
        assert!(bytes.contains("imported/"));
    }
    assert!(parse::exports(
        json!([{"id":"cycle","current_node":"x","mapping":{"x":{"parent":"x"}}}])
    )
    .is_err());
}

#[test]
fn limits_symlinks_cancellation_and_utf8_summary() {
    let f = Fixture::new();
    let huge = f.root.join("oversized.jsonl");
    fs::File::create(&huge)
        .unwrap()
        .set_len(io::LIMIT + 1)
        .unwrap();
    assert!(io::read(&huge, &f.service.cancel)
        .unwrap_err()
        .contains("200 MB"));
    #[cfg(unix)]
    {
        let link = f.root.join("link.jsonl");
        std::os::unix::fs::symlink(f.service.roots.codex.join("auth.json"), &link).unwrap();
        assert!(io::read(&link, &f.service.cancel).is_err());
    }
    f.service.cancel.store(true, Ordering::Relaxed);
    assert!(parse::detect(&f.service.roots, &f.service.cancel)
        .unwrap_err()
        .contains("cancelled"));
    let mut messages = vec![json!({"role":"user","text":"界".repeat(10000)})];
    for i in 0..10 {
        messages.push(json!({"role":"assistant","text":format!("last {i}")}));
    }
    let context = continuation(&messages);
    assert!(context.len() < 8500);
    assert!(context.contains("[Middle omitted]"));
    assert!(context.ends_with("last 9"));
}

#[test]
fn zip_ingress_sqlite_fallback_and_cli_group_are_synthetic() {
    let mut f = Fixture::new();
    let exports = f.root.join("export");
    fs::create_dir_all(&exports).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/import/chatgpt/conversations.json"),
        exports.join("conversations.json"),
    )
    .unwrap();
    // A credential member is never extracted or opened.
    fs::write(exports.join("auth.json"), "never-read-this-member").unwrap();
    let zip = f.root.join("export.zip");
    assert!(std::process::Command::new("zip")
        .current_dir(&exports)
        .args(["-q"])
        .arg(&zip)
        .args(["conversations.json", "auth.json"])
        .status()
        .unwrap()
        .success());
    let detection = f.call("export", json!({"path":zip}));
    assert_eq!(detection["entries"][0]["source"], "chatgpt");
    assert!(!f.service.data.join("auth.json").exists());
    fs::write(
        f.service.roots.codex.join("state_99.sqlite"),
        "unknown schema",
    )
    .unwrap();
    fs::remove_file(
        f.service
            .roots
            .desktop
            .join("claude-code-sessions/acct/org/local_code.json"),
    )
    .unwrap();
    let detection = f.call("detect", json!({}));
    let entries = detection["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["source"] == "claude-cli"));
    assert!(entries.iter().any(|e| e["source"] == "codex"));
    assert!(!entries.iter().any(|e| e["source"] == "codex-project"));
}

#[tokio::test]
async fn native_claude_metadata_loads_without_starting_an_engine() {
    let mut f = Fixture::new();
    let log = f.detect_and_import();
    let receipt = log["created"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"].as_str().unwrap().starts_with("claude:"))
        .unwrap();
    let ws = receipt["workspaceId"].as_str().unwrap();
    let engine = crate::shared::acp::ClaudeAcpEngine::new(
        f.service.data.clone(),
        ws.into(),
        f.root.join("project").to_string_lossy().into(),
        None,
        Arc::new(|_, _| {}),
    );
    let response = engine
        .request(ws, "thread/read", json!({"threadId":receipt["threadId"]}))
        .await
        .unwrap();
    assert_eq!(response["result"]["thread"]["engine"], "claude");
    assert_eq!(
        response["result"]["thread"]["sessionId"],
        "22222222-2222-4222-8222-222222222222"
    );
    assert!(response.to_string().contains("Clocks measure time"));
}

#[test]
fn interrupted_index_recovery_and_repeated_undo_preserve_new_imports() {
    let mut f = Fixture::new();
    let first = f.detect_and_import();
    fs::remove_file(f.service.data.join("imports/index.json")).unwrap();
    assert_eq!(
        f.detect_and_import()["created"].as_array().unwrap().len(),
        0
    );
    f.call("undo", json!({"logId":first["id"]}));
    let second = f.detect_and_import();
    f.call("undo", json!({"logId":first["id"]}));
    let receipt: Log = serde_json::from_value(second).unwrap();
    for r in receipt.created {
        for path in r.files.keys() {
            assert!(f.service.data.join(path).exists());
        }
    }
}
