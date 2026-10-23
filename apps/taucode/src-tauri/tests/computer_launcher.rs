#[test]
fn computer_launcher_lists_tools_without_screen_input_or_app_startup() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    for binary in [
        env!("CARGO_BIN_EXE_codex-monitor"),
        env!("CARGO_BIN_EXE_codex_monitor_daemon"),
    ] {
        let missing = Command::new(binary)
            .arg("--tau-computer-mcp")
            .output()
            .unwrap();
        assert!(!missing.status.success());
        assert!(String::from_utf8_lossy(&missing.stderr).contains("Usage: --tau-computer-mcp"));
        let mut child = Command::new(binary)
            .args(["--tau-computer-mcp", "/nonexistent-computer-data"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "computer_run_task");
        assert_eq!(tools[1]["annotations"]["readOnlyHint"], true);
    }
}
