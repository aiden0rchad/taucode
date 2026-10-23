use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct Keys(Mutex<BTreeMap<String, String>>);
impl KeyStore for Keys {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}
fn server(id: &str, transport: Value) -> McpServerEntry {
    serde_json::from_value(json!({"id":id,"name":"Example","enabled":true,"transport":transport,"approval":"ask","toolAllowlist":["read","write"],"toolDenylist":["write"]})).unwrap()
}
#[test]
fn both_transports_are_secret_free_for_both_engines() {
    let stdio = server(
        "local",
        json!({"type":"stdio","command":"server","args":["--stdio"],"cwd":"/workspace","env":{"API_KEY":{"secret":true},"LITERAL":{"value":"!echo ${DONT_EXECUTE}"}}}),
    );
    let http = server(
        "remote",
        json!({"type":"http","url":"https://example.test/mcp","headers":{"X-Key":{"secret":true},"Version":{"value":"1"}},"bearer":{"secret":true}}),
    );
    let keys = Keys::default();
    for name in ["mcp:local:API_KEY", "mcp:remote:X-Key", "mcp:remote:bearer"] {
        keys.set(name, "very-private-token").unwrap();
    }
    validate(&[stdio.clone(), http.clone()], false).unwrap();
    let out = delivery(&[stdio.clone(), http.clone()], &keys).unwrap();
    for serialized in [
        out.pi.to_string(),
        out.codex.join(" "),
        serde_json::to_string(&[stdio, http]).unwrap(),
    ] {
        assert!(!serialized.contains("very-private-token"));
    }
    assert_eq!(out.env[&env_name("local", "API_KEY")], "very-private-token");
    assert_eq!(
        out.pi["mcpServers"]["local"]["env"]["API_KEY"],
        format!("${{{}}}", env_name("local", "API_KEY"))
    );
    assert_eq!(
        out.pi["mcpServers"]["local"]["toolExposure"]["write"],
        "hidden"
    );
    assert_eq!(
        out.pi["mcpServers"]["remote"]["headers"]["Authorization"],
        format!("Bearer ${{{}}}", env_name("remote", "bearer"))
    );
    for arg in out.codex.iter().skip(1).step_by(2) {
        arg.parse::<toml_edit::Document>().unwrap();
    }
    let stdio = out.codex[3].parse::<toml_edit::Document>().unwrap();
    assert_eq!(
        stdio["mcp_servers"]["local"]["default_tools_approval_mode"].as_str(),
        Some("prompt")
    );
    assert_eq!(
        stdio["mcp_servers"]["local"]["env_vars"][0].as_str(),
        Some(env_name("local", "API_KEY").as_str())
    );
    let http = out.codex[5].parse::<toml_edit::Document>().unwrap();
    assert_eq!(
        http["mcp_servers"]["remote"]["env_http_headers"]["X-Key"].as_str(),
        Some(env_name("remote", "X-Key").as_str())
    );
}
#[test]
fn reserved_ids_and_overrides() {
    let s = server("local", json!({"type":"stdio","command":"server"}));
    for id in [
        "codegraph",
        "taucode-browser",
        "taucode-computer",
        "taucode_browser",
    ] {
        let mut entry = s.clone();
        entry.id = id.into();
        assert!(validate(&[entry], false).is_err());
    }
    let app = AppSettings {
        mcp_servers: vec![s.clone()],
        ..Default::default()
    };
    let mut workspace = WorkspaceSettings {
        mcp_servers: vec![
            serde_json::from_value(json!({"id":"local","enabled":false})).unwrap(),
            server(
                "remote",
                json!({"type":"http","url":"https://example.test/mcp"}),
            ),
        ],
        ..Default::default()
    };
    let entries = effective(&app, &workspace, None).unwrap();
    assert_eq!(entries.len(), 3);
    assert!(!entries[0].enabled);
    assert_eq!(entries[0].transport, s.transport);
    assert_eq!(entries[0].approval, Approval::Ask);
    assert!(entries.iter().any(|s| s.id == "codegraph"));
    workspace.mcp_servers[0].transport = s.transport;
    assert!(effective(&app, &workspace, None).is_err());
    let disabled = delivery(&entries, &Keys::default()).unwrap();
    assert!(disabled.pi["mcpServers"].get("local").is_none());
}
#[test]
fn credential_validation_and_names_do_not_alias() {
    assert_ne!(env_name("a-b", "X-Key"), env_name("a_b", "X_Key"));
    assert_ne!(env_name("a", "Key"), env_name("a", "KEY"));
    for transport in [
        json!({"type":"http","url":"https://user:password@example.test"}),
        json!({"type":"http","url":"file:///etc/passwd"}),
        json!({"type":"http","url":"https://example.test","bearer":{"value":"secret"}}),
        json!({"type":"stdio","command":"server","env":{"KEY":{"secret":false}}}),
    ] {
        assert!(validate(&[server("test", transport)], false).is_err());
    }
    assert!(serde_json::from_value::<Credential>(json!({"secret":true,"value":"leak"})).is_err());
}

#[test]
fn stdio_servers_can_use_different_values_for_the_same_variable() {
    let keys = Keys::default();
    keys.set("mcp:first:API_KEY", "first-private-value")
        .unwrap();
    keys.set("mcp:second:API_KEY", "second-private-value")
        .unwrap();
    let entries = ["first", "second"].map(|id| {
        server(
            id,
            json!({"type":"stdio","command":"server","env":{"API_KEY":{"secret":true}}}),
        )
    });
    let out = delivery(&entries, &keys).unwrap();
    assert_eq!(
        out.env[&env_name("first", "API_KEY")],
        "first-private-value"
    );
    assert_eq!(
        out.env[&env_name("second", "API_KEY")],
        "second-private-value"
    );
    assert!(!out.env.contains_key("API_KEY"));
    assert!(out.codex[3].contains("--tau-mcp-env"));
    assert!(!out.codex.join(" ").contains("private-value"));
    let root = std::env::temp_dir().join(format!("mcp-config-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    super::super::pi::config::write_json(&root.join("mcp.json"), &out.pi).unwrap();
    assert!(!std::fs::read_to_string(root.join("mcp.json"))
        .unwrap()
        .contains("private-value"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn removing_a_connector_deletes_only_its_declared_keys() {
    let keys = Keys::default();
    keys.set("mcp:old:bearer", "old-private").unwrap();
    keys.set("mcp:retained:bearer", "keep-private").unwrap();
    let old = server(
        "old",
        json!({"type":"http","url":"https://example.test/mcp","bearer":{"secret":true}}),
    );
    delete_removed_secrets(&[old], &[], &keys).unwrap();
    assert!(keys.get("mcp:old:bearer").unwrap().is_none());
    assert_eq!(
        keys.get("mcp:retained:bearer").unwrap().as_deref(),
        Some("keep-private")
    );
}

#[test]
fn codex_cannot_load_an_oauth_credential_file() {
    let home = std::env::temp_dir().join(format!("mcp-no-oauth-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".credentials.json"), "{}").unwrap();
    let error = super::super::engines::prepare_codex(
        &home,
        &[],
        &crate::types::AgentPolicy::default(),
        &Keys::default(),
    )
    .unwrap_err();
    assert!(error.contains("OAuth sign-in is not supported"));
    std::fs::remove_dir_all(home).unwrap();
}
#[tokio::test]
async fn credential_commands_only_use_key_store() {
    let entry = server(
        "test",
        json!({"type":"http","url":"https://example.test","bearer":{"secret":true}}),
    );
    let app = tokio::sync::Mutex::new(AppSettings {
        mcp_servers: vec![entry],
        ..Default::default()
    });
    let workspaces = tokio::sync::Mutex::new(std::collections::HashMap::new());
    let keys = Keys::default();
    request(
        json!({"action":"setSecret","id":"test","name":"bearer","value":"private"}),
        &app,
        &workspaces,
        None,
        &keys,
    )
    .await
    .unwrap();
    assert_eq!(
        keys.get("mcp:test:bearer").unwrap().as_deref(),
        Some("private")
    );
    assert_eq!(
        request(
            json!({"action":"secrets","id":"test"}),
            &app,
            &workspaces,
            None,
            &keys
        )
        .await
        .unwrap(),
        json!({"bearer":true})
    );
    let entries = request(json!({"action":"list"}), &app, &workspaces, None, &keys)
        .await
        .unwrap();
    assert!(!entries.to_string().contains("private"));
    assert!(request(
        json!({"action":"setSecret","id":"test","name":"other","value":"private"}),
        &app,
        &workspaces,
        None,
        &keys
    )
    .await
    .is_err());
    request(
        json!({"action":"deleteSecret","id":"test","name":"bearer"}),
        &app,
        &workspaces,
        None,
        &keys,
    )
    .await
    .unwrap();
    assert!(keys.get("mcp:test:bearer").unwrap().is_none());
}
