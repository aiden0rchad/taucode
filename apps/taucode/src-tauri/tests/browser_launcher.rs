#[test]
fn browser_launcher_errors_before_tauri_or_daemon_initialization() {
    for binary in [
        env!("CARGO_BIN_EXE_codex-monitor"),
        env!("CARGO_BIN_EXE_codex_monitor_daemon"),
    ] {
        let output = std::process::Command::new(binary)
            .arg("--tau-browser-mcp")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Missing browser app data directory")
        );
        assert!(output.stdout.is_empty());
    }
}
