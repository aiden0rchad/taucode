use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct CodegraphSetup {
    pub proxy_bin: PathBuf,
    pub upstream: Vec<String>,
    pub state_dir: PathBuf,
}

impl CodegraphSetup {
    pub fn codex_config_args(&self) -> Vec<OsString> {
        let args = std::iter::once("--")
            .chain(self.upstream.iter().map(String::as_str))
            .map(quote)
            .collect::<Vec<_>>()
            .join(", ");
        [
            format!(
                "mcp_servers.codegraph.command={}",
                quote(&self.proxy_bin.to_string_lossy())
            ),
            format!("mcp_servers.codegraph.args=[{args}]"),
            format!(
                "mcp_servers.codegraph.env={{ TAUCODE_STATE_DIR = {} }}",
                quote(&self.state_dir.to_string_lossy())
            ),
        ]
        .into_iter()
        .flat_map(|value| [OsString::from("-c"), OsString::from(value)])
        .collect()
    }

    pub fn on_engine_message(&self, msg: &Value) {
        if msg.get("id").is_none()
            && matches!(
                msg["method"].as_str(),
                Some("thread/compacted" | "thread/reverted")
            )
            && let Some(thread) = msg["params"]["threadId"].as_str()
        {
            self.bump_epoch(thread);
        }
    }

    pub fn bump_epoch(&self, thread: &str) {
        tau_mcp_proxy::epoch::bump_epoch(&self.state_dir, thread);
    }
}

pub fn discover(app_state_dir: PathBuf) -> Option<CodegraphSetup> {
    let proxy = std::env::var_os("TAUCODE_MCP_PROXY_BIN")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|exe| exe.with_file_name("tau-mcp-proxy"))
        });
    let codegraph_available = std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| executable(&dir.join("codegraph")))
    });
    match proxy.filter(|path| executable(path)) {
        Some(proxy_bin) if codegraph_available => Some(CodegraphSetup {
            proxy_bin,
            upstream: vec!["codegraph".into(), "serve".into(), "--mcp".into()],
            state_dir: app_state_dir,
        }),
        _ => {
            eprintln!("codegraph disabled: tau-mcp-proxy or codegraph executable is missing");
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

fn quote(value: &str) -> String {
    use std::fmt::Write;
    let mut result = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            c if c.is_control() => {
                write!(result, "\\u{:04X}", c as u32).expect("write to string");
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quotes_config_overrides() {
        let setup = CodegraphSetup {
            proxy_bin: PathBuf::from("/path with spaces/proxy\"\\"),
            upstream: vec!["codegraph".into(), "serve".into(), "--mcp".into()],
            state_dir: PathBuf::from("/state\n\t\u{c}"),
        };
        let args = setup.codex_config_args();
        assert_eq!(args[0], "-c");
        assert_eq!(
            args[1],
            "mcp_servers.codegraph.command=\"/path with spaces/proxy\\\"\\\\\""
        );
        assert_eq!(
            args[3],
            "mcp_servers.codegraph.args=[\"--\", \"codegraph\", \"serve\", \"--mcp\"]"
        );
        assert_eq!(
            args[5],
            "mcp_servers.codegraph.env={ TAUCODE_STATE_DIR = \"/state\\u000A\\u0009\\u000C\" }"
        );
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
            proxy_bin: PathBuf::new(),
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
