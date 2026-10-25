pub(crate) mod bridge;
mod launcher;
#[cfg(unix)]
mod lifecycle;
#[cfg(unix)]
mod paths;
#[cfg(windows)]
mod pipe;
mod process;
pub(crate) mod signal;
#[cfg(test)]
mod tests;

use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch, Mutex};

pub(crate) use crate::types::BrowserSettings as Settings;
pub const READ_TOOLS: &[&str] = &[
    "browser_snapshot",
    "browser_take_screenshot",
    "browser_console_messages",
    "browser_network_requests",
    "browser_tabs",
    "browser_navigate",
    "browser_navigate_back",
    "browser_wait_for",
];

pub fn validate(settings: &Settings) -> Result<(), String> {
    if settings.binary.as_ref().is_some_and(|s| s.contains('\0')) {
        return Err("Invalid browser binary path".into());
    }
    if !settings.search_url.contains("{query}")
        || !matches!(
            reqwest::Url::parse(&settings.search_url.replace("{query}", "test"))
                .map(|u| u.scheme().to_string())
                .as_deref(),
            Ok("https" | "http")
        )
    {
        return Err("Browser search URL must be an HTTP(S) URL containing {query}".into());
    }
    if settings
        .allowed_origins
        .iter()
        .chain(&settings.blocked_origins)
        .any(|s| s.is_empty() || s.contains(char::is_whitespace) || s.contains('\0'))
    {
        return Err("Enter one origin pattern per line, without spaces".into());
    }
    Ok(())
}
struct Running {
    process: process::Process,
    requests: mpsc::Sender<bridge::Request>,
    task: tokio::task::JoinHandle<()>,
}
trait Managed {
    fn healthy(&mut self) -> bool;
    fn stop(&mut self) -> impl std::future::Future<Output = ()> + Send;
}
impl Managed for Running {
    fn healthy(&mut self) -> bool {
        self.process.alive() && !self.task.is_finished()
    }
    async fn stop(&mut self) {
        self.task.abort();
        self.process.stop().await;
    }
}
async fn ensure_running<R: Managed, F: std::future::Future<Output = Result<R, String>>>(
    running: &mut Option<R>,
    start: impl FnOnce() -> F,
) -> Result<(), String> {
    if running.as_mut().is_some_and(Managed::healthy) {
        return Ok(());
    }
    if let Some(mut old) = running.take() {
        old.stop().await;
    }
    *running = Some(start().await?);
    Ok(())
}
pub struct Browser {
    shutting_down: std::sync::atomic::AtomicBool,
    data: PathBuf,
    running: Mutex<Option<Running>>,
    pub state: watch::Sender<Value>,
    server: Mutex<Option<tokio::task::JoinHandle<()>>>,
    #[cfg(unix)]
    owner_lock: Mutex<Option<std::fs::File>>,
}
impl Browser {
    pub fn new(data: PathBuf) -> Arc<Self> {
        let (state, _) = watch::channel(json!({"tabs":[],"active":"","frame":null,"agent":false}));
        Arc::new(Self {
            shutting_down: std::sync::atomic::AtomicBool::new(false),
            data,
            running: Mutex::new(None),
            state,
            server: Mutex::new(None),
            #[cfg(unix)]
            owner_lock: Mutex::new(None),
        })
    }
    pub fn settings(&self) -> Result<Settings, String> {
        Ok(crate::storage::read_settings(&self.data.join("settings.json"))?.browser)
    }
    async fn ensure(&self, running: &mut Option<Running>, p: &Value) -> Result<(), String> {
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Browser host is shutting down".into());
        }
        if !crate::shared::engine_registry::features(Some(&self.data)).browser { return Err("Enable Browser in Settings → Features".into()); }
        ensure_running(running, || async {
            let settings = self.settings()?;
            validate(&settings)?;
            let mut process = process::launch(
                &self.data,
                &settings,
                p["width"].as_u64().unwrap_or(1000).clamp(1, 7680) as u32,
                p["height"].as_u64().unwrap_or(700).clamp(1, 4320) as u32,
            )
            .await?;
            let connected = async {
                let socket = bridge::Socket::connect(&process.websocket).await?;
                self.state
                    .send_replace(json!({"tabs":[],"active":"","frame":null,"agent":false}));
                let mut bridge = bridge::Bridge::new(socket, self.state.clone());
                let downloads = std::env::var_os("HOME")
                    .or_else(|| std::env::var_os("USERPROFILE"))
                    .map(PathBuf::from)
                    .ok_or("Cannot locate Downloads directory")?
                    .join("Downloads");
                bridge.initialize(&downloads.to_string_lossy()).await?;
                Ok::<_, String>(bridge)
            }
            .await;
            let bridge = match connected {
                Ok(bridge) => bridge,
                Err(error) => {
                    process.stop().await;
                    return Err(error);
                }
            };
            let (requests, receiver) = mpsc::channel(128);
            let task = tokio::spawn(bridge.run(receiver));
            Ok(Running {
                process,
                requests,
                task,
            })
        })
        .await
    }
    pub async fn stop(&self) {
        if let Some(mut running) = self.running.lock().await.take() {
            running.task.abort();
            running.process.stop().await;
        }
        self.state
            .send_replace(json!({"tabs":[],"active":"","frame":null,"agent":false}));
    }
    pub async fn shutdown(&self) {
        self.shutting_down
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.stop().await;
        if let Some(server) = self.server.lock().await.take() {
            server.abort();
            #[cfg(unix)]
            {
                paths::cleanup(&self.data);
                self.owner_lock.lock().await.take();
            }
        }
    }
    pub async fn request(&self, p: Value) -> Result<Value, String> {
        #[cfg(any(unix, windows))]
        if self.server.lock().await.is_none() {
            return ipc(&self.data, p).await;
        }
        self.local_request(p).await
    }
    async fn local_request(&self, p: Value) -> Result<Value, String> {
        match p["action"].as_str() {
            Some("clear") => {
                if p["confirmed"] != true {
                    return Err("Confirm clearing browsing data first".into());
                }
                // Serialize profile deletion with process startup.
                let mut running = self.running.lock().await;
                if let Some(mut old) = running.take() {
                    old.task.abort();
                    old.process.stop().await;
                }
                let dir = process::profile(&self.data);
                if dir.exists() {
                    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
                }
                self.state
                    .send_replace(json!({"tabs":[],"active":"","frame":null,"agent":false}));
                return Ok(Value::Null);
            }
            Some("activity") => {
                self.state
                    .send_modify(|s| s["agent"] = json!(p["active"] == true));
                return Ok(Value::Null);
            }
            Some("poll") => {
                let mut running = self.running.lock().await;
                if running
                    .as_mut()
                    .is_some_and(|r| !r.process.alive() || r.task.is_finished())
                {
                    let previous = self.state.borrow().clone();
                    self.ensure(&mut running, &previous).await?;
                    if previous["streaming"] == true {
                        let (reply, receiver) = oneshot::channel();
                        running.as_ref().unwrap().requests.send((json!({"action":"attach","width":previous["width"],"height":previous["height"],"dpr":previous["dpr"]}), reply)).await.map_err(|_| "Browser disconnected")?;
                        receiver.await.map_err(|_| "Browser disconnected")??;
                    }
                }
                return Ok(self.state.borrow().clone());
            }
            _ => (),
        }
        let mut running = self.running.lock().await;
        if p["action"] == "detach" && running.is_none() {
            return Ok(Value::Null);
        }
        self.ensure(&mut running, &p).await?;
        let running = running.as_ref().unwrap();
        if p["action"] == "ensure" {
            return Ok(json!({"port":running.process.port}));
        }
        let (reply, receiver) = oneshot::channel();
        running
            .requests
            .send((p, reply))
            .await
            .map_err(|_| "Browser disconnected")?;
        tokio::time::timeout(Duration::from_secs(30), receiver)
            .await
            .map_err(|_| "Browser request timed out")?
            .map_err(|_| "Browser disconnected")?
    }
    pub async fn serve(self: &Arc<Self>) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::PermissionsExt;
            let mut server = self.server.lock().await;
            if server.is_some() {
                return Ok(());
            }
            let dir = self.data.join("browser");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(dir.join("owner.lock"))
                .map_err(|e| e.to_string())?;
            // Keep the lock for the owner's lifetime, preventing stale-socket races.
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error.to_string());
            }
            let listener = paths::bind(&self.data).await?;
            *self.owner_lock.lock().await = Some(lock);
            let browser = Arc::downgrade(self);
            *server = Some(tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    if let Err(error) = crate::shared::local_socket::validate_peer(&socket) {
                        eprintln!("Rejected browser control peer: {error}");
                        continue;
                    }
                    let Some(browser) = browser.upgrade() else {
                        break;
                    };
                    tokio::spawn(serve_client(browser, socket));
                }
            }));
        }
        #[cfg(windows)]
        {
            let mut server = self.server.lock().await;
            if server.is_some() {
                return Ok(());
            }
            let name = pipe_name(&self.data);
            let mut pipe = match pipe::create(std::ffi::OsStr::new(&name), true) {
                Ok(pipe) => pipe,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
                Err(e) => return Err(e.to_string()),
            };
            let browser = Arc::downgrade(self);
            *server = Some(tokio::spawn(async move {
                loop {
                    if pipe.connect().await.is_err() {
                        break;
                    }
                    let next = match pipe::create(std::ffi::OsStr::new(&name), false) {
                        Ok(next) => next,
                        Err(_) => break,
                    };
                    let Some(browser) = browser.upgrade() else {
                        break;
                    };
                    tokio::spawn(serve_client(browser, pipe));
                    pipe = next;
                }
            }));
        }
        Ok(())
    }
}
#[cfg(any(unix, windows))]
async fn ipc(data: &Path, p: Value) -> Result<Value, String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    tokio::time::timeout(Duration::from_secs(45), async {
        // Another owner can hold the lock while it is still binding its endpoint.
        #[cfg(unix)]
        let mut socket = {
            let mut attempts = 0;
            loop {
                match paths::connect(data).await {
                    Ok(socket) => break socket,
                    Err(e)
                        if attempts < 50
                            && matches!(
                                e.kind(),
                                std::io::ErrorKind::NotFound
                                    | std::io::ErrorKind::ConnectionRefused
                            ) =>
                    {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => return Err(format!("Browser host unavailable: {e}")),
                }
            }
        };
        #[cfg(windows)]
        let mut socket = {
            let mut attempts = 0;
            loop {
                match tokio::net::windows::named_pipe::ClientOptions::new().open(pipe_name(data)) {
                    Ok(socket) => break socket,
                    Err(e)
                        if attempts < 50
                            && (e.raw_os_error() == Some(231)
                                || e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => return Err(format!("Browser host unavailable: {e}")),
                }
            }
        };
        socket
            .write_all(format!("{p}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(socket.take(32 * 1024 * 1024))
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if let Some(error) = response["error"].as_str() {
            Err(error.into())
        } else {
            Ok(response["result"].clone())
        }
    })
    .await
    .map_err(|_| "Browser host timed out".to_string())?
}

pub fn mcp_cli(args: Vec<std::ffi::OsString>) -> Result<(), String> {
    let data = args
        .first()
        .map(PathBuf::from)
        .ok_or("Missing browser app data directory")?;
    if args.len() != 1 {
        return Err("Usage: --tau-browser-mcp <app-data-dir>".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(launcher::run(data))
}

#[cfg(windows)]
fn pipe_name(data: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hash);
    format!(r"\\.\pipe\taucode-browser-{:016x}", hash.finish())
}

async fn serve_client(
    browser: Arc<Browser>,
    socket: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
) {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let (read, mut write) = tokio::io::split(socket);
    let mut line = String::new();
    let read = tokio::time::timeout(
        Duration::from_secs(30),
        BufReader::new(read.take(1024 * 1024)).read_line(&mut line),
    )
    .await;
    if !matches!(read, Ok(Ok(n)) if n > 0) {
        return;
    }
    let result = match serde_json::from_str(&line) {
        Ok(p) => browser.local_request(p).await,
        Err(e) => Err(e.to_string()),
    };
    let response = match result {
        Ok(v) => json!({"result":v}),
        Err(e) => json!({"error":e}),
    };
    let _ = write.write_all(format!("{response}\n").as_bytes()).await;
}
