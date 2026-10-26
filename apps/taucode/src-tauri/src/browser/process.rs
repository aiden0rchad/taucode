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
    #[cfg(unix)]
    data: PathBuf,
    #[cfg(unix)]
    group: Option<u32>,
}
impl Process {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub async fn stop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.group {
            if let Err(e) = super::lifecycle::stop(&self.data, pid).await {
                eprintln!("Browser cleanup: {e}");
                super::lifecycle::kill_group(pid);
            }
        }
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        #[cfg(unix)]
        {
            self.group = None;
        }
    }
}
#[cfg(unix)]
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(pid) = self.group {
            super::lifecycle::kill_group(pid);
        }
    }
}
pub async fn launch(
    data: &Path,
    settings: &Settings,
    width: u32,
    height: u32,
) -> Result<Process, String> {
    let binary = match discover(settings, Path::is_file) {
        Ok(binary) => binary,
        Err(error) if settings.binary.as_ref().is_some_and(|p| !p.is_empty()) => return Err(error),
        Err(_) => managed_chromium(data).await?,
    };
    let dir = profile(data);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    super::lifecycle::cleanup(data)
        .await
        .map_err(|e| format!("Cannot stop stale browser: {e}"))?;
    let active = dir.join("DevToolsActivePort");
    match std::fs::remove_file(&active) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    let mut command = Command::new(binary);
    command
        .args(arguments(data, width, height))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .spawn()
        .map_err(|e| format!("Cannot launch browser: {e}"))?;
    let mut process = Process {
        #[cfg(unix)]
        group: child.id(),
        child,
        port: 0,
        websocket: String::new(),
        #[cfg(unix)]
        data: data.to_path_buf(),
    };
    #[cfg(unix)]
    if let Err(e) = super::lifecycle::record(
        data,
        process.child.id().ok_or("Browser exited during launch")?,
    ) {
        process.stop().await;
        return Err(format!("Cannot record browser process: {e}"));
    }
    for _ in 0..150 {
        if let Some(status) = process.child.try_wait().map_err(|e| e.to_string())? {
            process.stop().await;
            return Err(format!(
                "Browser exited ({status}). The taucode profile may already be in use."
            ));
        }
        if let Ok(text) = std::fs::read_to_string(&active) {
            if let Ok((port, websocket)) = parse_port(&text) {
                process.port = port;
                process.websocket = websocket;
                return Ok(process);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    process.stop().await;
    Err("Browser did not create DevToolsActivePort within 15 seconds".into())
}

async fn managed_chromium(data: &Path) -> Result<PathBuf, String> {
    let runtime = crate::shared::engine_runtime::prepare(data, "browser", None).await?;
    let node = crate::shared::engine_runtime::node_path();
    // Keep downloaded browsers outside the source-synchronized engine directory.
    let browsers = data.join("browser/runtimes");
    let mut query = Command::new(&node);
    query.current_dir(&runtime).env("PLAYWRIGHT_BROWSERS_PATH", &browsers)
        .args(["-e", "process.stdout.write(require('./node_modules/playwright').chromium.executablePath())"])
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(15), query.output()).await
        .map_err(|_| "Browser path lookup timed out")?.map_err(|e| e.to_string())?;
    if !output.status.success() { return Err("Cannot locate managed Chromium".into()); }
    let binary = PathBuf::from(String::from_utf8(output.stdout).map_err(|e| e.to_string())?);
    if !binary.is_absolute() || !binary.starts_with(&browsers) { return Err("Invalid managed Chromium path".into()); }
    if !binary.is_file() {
        let mut install = Command::new(&node);
        install.current_dir(&runtime).env("PLAYWRIGHT_BROWSERS_PATH", &browsers)
            .env("PATH", crate::shared::engine_runtime::node_path_env(&node)?)
            .arg(runtime.join("node_modules/playwright/cli.js")).args(["install", "chromium"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
        let status = tokio::time::timeout(std::time::Duration::from_secs(600), install.status()).await
            .map_err(|_| "Chromium installation timed out; retry Browser setup")?.map_err(|e| e.to_string())?;
        if !status.success() || !binary.is_file() { return Err("Chromium installation failed. Check network access and retry.".into()); }
    }
    Ok(binary)
}
