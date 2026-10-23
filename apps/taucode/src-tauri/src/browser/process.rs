use super::Settings;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::{Child, Command};

pub fn profile(data: &Path) -> PathBuf {
    data.join("browser/profile")
}
pub fn parse_port(text: &str) -> Result<(u16, String), String> {
    let mut lines = text.lines();
    let port = lines
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|p| *p != 0)
        .ok_or("Invalid DevToolsActivePort port")?;
    let path = lines
        .next()
        .filter(|s| s.starts_with("/devtools/browser/") && !s.contains(char::is_whitespace))
        .ok_or("Invalid DevToolsActivePort websocket path")?;
    Ok((port, format!("ws://127.0.0.1:{port}{path}")))
}
pub fn candidates(settings: &Settings) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = settings
        .binary
        .iter()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    #[cfg(target_os = "macos")]
    paths.extend(
        [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ]
        .map(PathBuf::from),
    );
    #[cfg(target_os = "linux")]
    paths.extend(
        [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/microsoft-edge",
            "/usr/bin/microsoft-edge-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ]
        .map(PathBuf::from),
    );
    #[cfg(target_os = "windows")]
    for relative in [
        "Google/Chrome/Application/chrome.exe",
        "Microsoft/Edge/Application/msedge.exe",
        "Chromium/Application/chrome.exe",
    ] {
        for root in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            if let Some(root) = std::env::var_os(root) {
                paths.push(PathBuf::from(root).join(relative));
            }
        }
    }
    paths
}
pub fn discover(settings: &Settings, exists: impl Fn(&Path) -> bool) -> Result<PathBuf, String> {
    candidates(settings).into_iter().find(|p| exists(p)).ok_or_else(|| "No browser found. Install Google Chrome, Microsoft Edge or Chromium, or set Settings → Browser → Browser binary.".into())
}
pub fn arguments(data: &Path, width: u32, height: u32) -> Vec<String> {
    vec![
        "--remote-debugging-port=0".into(),
        format!("--user-data-dir={}", profile(data).display()),
        "--headless=new".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-features=Translate".into(),
        format!("--window-size={width},{height}"),
        "about:blank".into(),
    ]
}
pub struct Process {
    pub child: Child,
    pub port: u16,
    pub websocket: String,
}
impl Process {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub async fn stop(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
pub async fn launch(
    data: &Path,
    settings: &Settings,
    width: u32,
    height: u32,
) -> Result<Process, String> {
    let binary = discover(settings, Path::is_file)?;
    let dir = profile(data);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let active = dir.join("DevToolsActivePort");
    match std::fs::remove_file(&active) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    let mut child = Command::new(binary)
        .args(arguments(data, width, height))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Cannot launch browser: {e}"))?;
    for _ in 0..150 {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "Browser exited ({status}). The taucode profile may already be in use."
            ));
        }
        if let Ok(text) = std::fs::read_to_string(&active) {
            if let Ok((port, websocket)) = parse_port(&text) {
                return Ok(Process {
                    child,
                    port,
                    websocket,
                });
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = child.kill().await;
    Err("Browser did not create DevToolsActivePort within 15 seconds".into())
}
