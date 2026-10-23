use std::path::{Path, PathBuf};

use serde_json::Value;

/// Index directory (inside the project) used by taucode's own codegraph daemon.
pub const CODEGRAPH_DIR: &str = ".codegraph-taucode";

#[derive(Clone, Debug)]
pub struct CodegraphSetup {
    pub proxy_exe: PathBuf,
    pub upstream: Vec<String>,
    pub state_dir: PathBuf,
}

impl CodegraphSetup {
    pub fn on_engine_message(&self, msg: &Value) {
        if msg.get("id").is_none()
            && (matches!(
                msg["method"].as_str(),
                Some("thread/compacted" | "thread/reverted")
            ) || (msg["method"] == "item/completed"
                && msg["params"]["item"]["type"] == "contextCompaction"))
        {
            if let Some(thread) = msg["params"]["threadId"].as_str() {
                self.bump_epoch(thread);
            }
        }
    }

    pub fn bump_epoch(&self, thread: &str) {
        tau_mcp_proxy::epoch::bump_epoch(&self.state_dir, thread);
    }
}

pub fn discover(app_state_dir: PathBuf) -> Option<CodegraphSetup> {
    let codegraph_available = std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            executable(&dir.join(if cfg!(windows) {
                "codegraph.exe"
            } else {
                "codegraph"
            }))
        })
    });
    match std::env::current_exe().ok() {
        Some(proxy_exe) if codegraph_available => Some(CodegraphSetup {
            proxy_exe,
            upstream: vec!["codegraph".into(), "serve".into(), "--mcp".into()],
            state_dir: app_state_dir.join("codegraph"),
        }),
        _ => {
            static LOG: std::sync::Once = std::sync::Once::new();
            LOG.call_once(|| {
                eprintln!("codegraph disabled: codegraph executable is missing from PATH")
            });
            None
        }
    }
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completed_compaction_items_bump_only_their_thread() {
        let dir = std::env::temp_dir().join(format!(
            "tau-compaction-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let setup = CodegraphSetup {
            proxy_exe: PathBuf::new(),
            upstream: vec![],
            state_dir: dir.clone(),
        };
        for (index, turn) in ["manual-compaction", "automatic-compaction"]
            .iter()
            .enumerate()
        {
            let mut message = json!({
                "method": "item/started",
                "params": {"threadId": "A", "turnId": turn, "item": {"type": "contextCompaction"}}
            });
            setup.on_engine_message(&message);
            assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "A"), index as u64);
            message["method"] = json!("item/completed");
            setup.on_engine_message(&message);
            assert_eq!(
                tau_mcp_proxy::epoch::read_epoch(&dir, "A"),
                index as u64 + 1
            );
            assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "B"), 0);
        }
        for message in [
            json!({"method":"item/completed","params":{"threadId":"A","item":{"type":"agentMessage"}}}),
            json!({"method":"item/completed","params":{"item":{"type":"contextCompaction"}}}),
            json!({"method":"item/completed","params":{"threadId":"A"}}),
            json!({"id":1,"method":"item/completed","params":{"threadId":"A","item":{"type":"contextCompaction"}}}),
            json!({"method":"turn/completed","params":{"threadId":"A","turn":{"id":"automatic-compaction","status":"completed"}}}),
        ] {
            setup.on_engine_message(&message);
        }
        assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "A"), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reset_notifications_only_bump_their_thread() {
        let dir = std::env::temp_dir().join(format!(
            "tau-codegraph-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let setup = CodegraphSetup {
            proxy_exe: PathBuf::new(),
            upstream: vec![],
            state_dir: dir.clone(),
        };
        for method in ["thread/compacted", "thread/reverted", "turn/completed"] {
            setup.on_engine_message(&json!({"method":method,"params":{"threadId":"A"}}));
        }
        setup.on_engine_message(
            &json!({"id":1,"method":"thread/reverted","params":{"threadId":"A"}}),
        );
        setup.on_engine_message(&json!({"method":"thread/reverted","params":{}}));
        assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "A"), 2);
        assert_eq!(tau_mcp_proxy::epoch::read_epoch(&dir, "B"), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
