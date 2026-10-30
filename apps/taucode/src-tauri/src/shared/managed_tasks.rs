//! Commands launched by the user or an approved Work tool. Never adopts external processes.
use std::{collections::HashMap, path::{Path, PathBuf}, process::Stdio, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::{io::{AsyncRead, AsyncReadExt}, process::Child, sync::{oneshot, watch, Mutex}};
use crate::{backend::app_server::WorkspaceSession, types::{AppSettings, WorkspaceEntry}};
use super::process_core::tokio_command;
#[cfg(windows)]
use super::process_core::kill_child_process_tree;

const OUTPUT_LIMIT: usize = 128 * 1024;
const TASK_LIMIT: usize = 128;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedTask {
    id: String,
    workspace_id: String,
    thread_id: String,
    command: String,
    label: String,
    cwd: String,
    pid: Option<u32>,
    status: String,
    started_at: i64,
    ended_at: Option<i64>,
    exit_code: Option<i32>,
    output: String,
    output_truncated: bool,
    urls: Vec<String>,
}

#[derive(Clone, Default)]
pub(crate) struct TaskRuntime {
    pub guard: Option<(PathBuf, PathBuf)>,
    pub env: Option<Vec<(String, String)>>,
}

struct Task {
    runtime: TaskRuntime,
    value: Mutex<ManagedTask>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    finished: watch::Sender<bool>,
    runner: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Default)]
pub(crate) struct ManagedTasks {
    tasks: Mutex<HashMap<String, Arc<Task>>>,
    closing: AtomicBool,
    shutdown_lock: Mutex<()>,
}

impl Drop for ManagedTasks {
    fn drop(&mut self) {
        if let Ok(tasks) = self.tasks.try_lock() {
            for task in tasks.values() {
                if let Ok(mut sender) = task.stop.try_lock() {
                    if let Some(sender) = sender.take() { let _ = sender.send(()); }
                }
            }
        }
    }
}

impl ManagedTasks {
    pub(crate) fn new() -> Arc<Self> { Arc::new(Self::default()) }

    /// Stop every owned command before the host exits; runtime drop is only a fallback.
    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        let _shutdown = self.shutdown_lock.lock().await;
        self.closing.store(true, Ordering::Release);
        let tasks: Vec<_> = self.tasks.lock().await.values().cloned().collect();
        let mut runners = Vec::new();
        for task in tasks {
            if let Some(stop) = task.stop.lock().await.take() { let _ = stop.send(()); }
            if let Some(runner) = task.runner.lock().unwrap().take() { runners.push(runner); }
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
        let mut cancelled = Vec::new();
        for mut runner in runners {
            if tokio::time::timeout_at(deadline, &mut runner).await.is_err() {
                // OwnedChild drops synchronously when the cancelled actor is joined.
                runner.abort();
                cancelled.push(runner);
            }
        }
        let deadline = deadline + Duration::from_secs(1);
        for runner in cancelled {
            let _ = tokio::time::timeout_at(deadline, runner).await.map_err(|_| "Background task cleanup timed out")?;
        }
        Ok(())
    }


    pub(crate) async fn request(
        &self,
        app_settings: &Mutex<AppSettings>,
        workspaces: &Mutex<HashMap<String, WorkspaceEntry>>,
        sessions: &Mutex<HashMap<String, Arc<WorkspaceSession>>>,
        workspace_id: String,
        thread_id: String,
        action: String,
        params: Value,
    ) -> Result<Value, String> {
        if workspace_id.trim().is_empty() || thread_id.trim().is_empty() {
            return Err("A workspace and session are required".into());
        }
        let path = workspaces.lock().await.get(&workspace_id)
            .map(|w| PathBuf::from(&w.path)).ok_or("Workspace not found")?;
        let cwd = if matches!(action.as_str(), "start" | "restart") {
            if !app_settings.lock().await.features().terminal { return Err("Enable Terminal in Settings → Features".into()); }
            let cwd = tokio::fs::canonicalize(&path).await.map_err(|e| format!("Workspace is unavailable: {e}"))?;
            let session = sessions.lock().await.get(&workspace_id).cloned().ok_or("Workspace not connected")?;
            let response = tokio::time::timeout(Duration::from_secs(15), session.send_request_for_workspace(
                &workspace_id, "thread/read", json!({"threadId":thread_id,"excludeTurns":true})
            )).await.map_err(|_| "Session validation timed out")??;
            let thread = response.pointer("/result/thread").or_else(|| response.get("thread")).ok_or("Session not found")?;
            validate_session(&cwd, &thread_id, thread).await?
        } else { path };
        self.request_owned(&workspace_id, &thread_id, &cwd, &action, params, TaskRuntime::default()).await
    }

    /// Caller must resolve the owned session and enforce its current access policy.
    pub(crate) async fn request_owned(&self, workspace: &str, thread: &str, cwd: &Path, action: &str, params: Value, runtime: TaskRuntime) -> Result<Value, String> {
        if workspace.is_empty() || thread.is_empty() { return Err("A workspace and session are required".into()); }
        match action {
            "list" => (),
            "start" | "restart" => {
                let (command, label, runtime) = if action == "restart" {
                    let task = self.find(workspace, thread, &params).await?;
                    let value = task.value.lock().await;
                    if value.status == "running" { return Err("Stop the task before restarting it".into()); }
                    (value.command.clone(), value.label.clone(), task.runtime.clone())
                } else {
                    (params["command"].as_str().ok_or("Command is required")?.to_string(), params["label"].as_str().unwrap_or("").to_string(), runtime)
                };
                if label.len() > 256 { return Err("Task name is too long".into()); }
                let cwd = tokio::fs::canonicalize(cwd).await.map_err(|e| format!("Workspace is unavailable: {e}"))?;
                if !cwd.is_dir() { return Err("Workspace is not a directory".into()); }
                self.start_with_runtime(workspace, thread, &cwd, command, label, runtime).await?;
            }
            "stop" => { let task = self.find(workspace, thread, &params).await?; stop(&task).await?; }
            "clearFinished" => {
                let mut tasks = self.tasks.lock().await;
                let mut remove = Vec::new();
                for (id, task) in tasks.iter() {
                    let value = task.value.lock().await;
                    if value.workspace_id == workspace && value.thread_id == thread && value.status != "running" { remove.push(id.clone()); }
                }
                for id in remove { tasks.remove(&id); }
            }
            _ => return Err("Unknown managed task action".into()),
        }
        serde_json::to_value(self.list(workspace, thread).await).map_err(|e| e.to_string())
    }

    pub(crate) async fn stop_session(&self, workspace: &str, thread: &str) -> Result<(), String> {
        let tasks: Vec<_> = self.tasks.lock().await.values().cloned().collect();
        for task in tasks {
            let value = task.value.lock().await;
            let owned = value.workspace_id == workspace && value.thread_id == thread;
            drop(value);
            if owned { stop(&task).await?; }
        }
        Ok(())
    }

    async fn find(&self, workspace: &str, thread: &str, params: &Value) -> Result<Arc<Task>, String> {
        let id = params["id"].as_str().ok_or("Task id is required")?;
        let task = self.tasks.lock().await.get(id).cloned().ok_or("Task not found")?;
        let value = task.value.lock().await;
        if value.workspace_id != workspace || value.thread_id != thread { return Err("Task not found in this session".into()); }
        drop(value);
        Ok(task)
    }

    async fn list(&self, workspace: &str, thread: &str) -> Vec<ManagedTask> {
        let tasks: Vec<_> = self.tasks.lock().await.values().cloned().collect();
        let mut values = Vec::new();
        for task in tasks {
            let value = task.value.lock().await;
            if value.workspace_id == workspace && value.thread_id == thread {
                let mut snapshot = value.clone();
                snapshot.output = strip_ansi(&snapshot.output);
                values.push(snapshot);
            }
        }
        values.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| a.id.cmp(&b.id)));
        values
    }

    #[cfg(test)]
    async fn start(&self, workspace: &str, thread: &str, cwd: &Path, command: String, label: String) -> Result<Arc<Task>, String> {
        self.start_with_runtime(workspace, thread, cwd, command, label, TaskRuntime::default()).await
    }
    async fn start_with_runtime(&self, workspace: &str, thread: &str, cwd: &Path, command: String, label: String, runtime: TaskRuntime) -> Result<Arc<Task>, String> {
        validate_command(&command)?;
        let mut tasks = self.tasks.lock().await;
        if self.closing.load(Ordering::Acquire) { return Err("Background tasks are shutting down".into()); }
        if tasks.len() >= TASK_LIMIT { return Err("Clear finished tasks before starting more".into()); }
        let mut running = 0;
        for task in tasks.values() { if task.value.lock().await.status == "running" { running += 1; } }
        if running >= 16 { return Err("At most 16 background tasks can run at once".into()); }
        #[cfg(unix)]
        let mut process = {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            let mut process = if let Some((node, script)) = &runtime.guard {
                let mut process = tokio_command(node);
                process.arg(script).arg(std::process::id().to_string()).arg(&shell).arg(&command).arg("--shared-group");
                process
            } else {
                let mut process = tokio_command(shell);
                process.arg("-lc").arg(&command);
                process
            };
            process.process_group(0);
            process
        };
        #[cfg(windows)]
        let mut process = { let mut process = tokio_command("cmd.exe"); process.args(["/D", "/S", "/C", &command]); process };
        if let Some(env) = &runtime.env { process.env_clear().envs(env.iter().cloned()); }
        process.current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = process.spawn().map_err(|e| format!("Could not start task: {e}"))?;
        let (stop_tx, stop_rx) = oneshot::channel();
        let (finished, _) = watch::channel(false);
        let id = uuid::Uuid::new_v4().to_string();
        let task = Arc::new(Task {
            runtime,
            value: Mutex::new(ManagedTask {
                id: id.clone(), workspace_id: workspace.into(), thread_id: thread.into(),
                label: if label.trim().is_empty() { command.lines().next().unwrap_or(&command).chars().take(100).collect() } else { label.trim().into() },
                command, cwd: cwd.to_string_lossy().into(), pid: child.id(), status: "running".into(),
                started_at: now(), ended_at: None, exit_code: None, output: String::new(), output_truncated: false, urls: Vec::new(),
            }), stop: Mutex::new(Some(stop_tx)), finished, runner: std::sync::Mutex::new(None),
        });
        tasks.insert(id, task.clone());
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let runner = tokio::spawn(run(task.clone(), OwnedChild(child), stop_rx, stdout, stderr));
        *task.runner.lock().unwrap() = Some(runner);
        Ok(task)
    }
}

async fn stop(task: &Task) -> Result<(), String> {
    let mut finished = task.finished.subscribe();
    if let Some(stop) = task.stop.lock().await.take() { let _ = stop.send(()); }
    if !*finished.borrow() {
        tokio::time::timeout(Duration::from_secs(10), finished.wait_for(|done| *done))
            .await.map_err(|_| "Task is still stopping")?.map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn validate_session(workspace: &Path, thread_id: &str, thread: &Value) -> Result<PathBuf, String> {
    if thread["id"].as_str() != Some(thread_id) { return Err("Session id does not match".into()); }
    let cwd = thread["cwd"].as_str().ok_or("Session has no workspace")?;
    let cwd = tokio::fs::canonicalize(cwd).await.map_err(|_| "Session workspace is unavailable")?;
    if !cwd.starts_with(workspace) { return Err("Session belongs to another workspace".into()); }
    Ok(cwd)
}

fn now() -> i64 { chrono::Utc::now().timestamp_millis() }
fn validate_command(command: &str) -> Result<(), String> {
    if command.trim().is_empty() || command.len() > 32 * 1024 || command.contains('\0') {
        return Err("Enter a command of at most 32768 bytes, without NUL characters".into());
    }
    Ok(())
}

// Own the unreaped child, not a saved PID: Drop runs before Child can reap it.
// This also covers cancellation before the spawned future is first polled.
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(pid) = self.0.id() {
            #[cfg(unix)]
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL); }
            #[cfg(windows)]
            { let _ = super::process_core::std_command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
        }
    }
}

async fn run(task: Arc<Task>, mut owned: OwnedChild, mut stop: oneshot::Receiver<()>, stdout: impl AsyncRead + Unpin + Send + 'static, stderr: impl AsyncRead + Unpin + Send + 'static) {
    let child = &mut owned.0;
    let out = tokio::spawn(read_output(task.clone(), stdout));
    let err = tokio::spawn(read_output(task.clone(), stderr));
    let mut stopped = false;
    // On Unix observe exit without reaping first, keeping the group leader's PID
    // reserved until remaining descendants have been terminated.
    let status = loop {
        tokio::select! {
            _ = &mut stop, if !stopped => {
                stopped = true;
                terminate_group(child).await;
            }
            _ = tokio::time::sleep(Duration::from_millis(40)) => {
                #[cfg(unix)]
                if let Some(pid) = child.id() {
                    let exited = {
                        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                        let result = unsafe { libc::waitid(libc::P_PID, pid, &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT) };
                        if result != 0 { break Err(std::io::Error::last_os_error()); }
                        unsafe { info.si_pid() != 0 }
                    };
                    if !exited { continue; }
                    terminate_group(child).await;
                }
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => (),
                    Err(error) => break Err(error),
                }
            }
        }
    };
    // Group termination closes its inherited pipes; cap draining even for a command
    // that deliberately detached a descendant into a different process group.
    for mut reader in [out, err] {
        if tokio::time::timeout(Duration::from_secs(1), &mut reader).await.is_err() { reader.abort(); }
    }
    let mut value = task.value.lock().await;
    value.ended_at = Some(now());
    value.exit_code = status.as_ref().ok().and_then(|status| status.code());
    value.status = if stopped { "stopped" } else if status.as_ref().is_ok_and(|status| status.success()) { "completed" } else { "failed" }.into();
    if let Err(error) = status { append_output(&mut value, &format!("\nCould not wait for task: {error}")); }
    task.finished.send_replace(true);
}

async fn terminate_group(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() { unsafe { libc::kill(-(pid as i32), libc::SIGKILL); } }
    #[cfg(windows)]
    kill_child_process_tree(child).await;
    #[cfg(not(windows))]
    let _ = child.start_kill();
}

async fn read_output(task: Arc<Task>, mut stream: impl AsyncRead + Unpin) {
    let mut buffer = [0; 4096];
    let mut pending = Vec::new();
    let mut url_pending = String::new();
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(n) => pending.extend_from_slice(&buffer[..n]),
        }
        let valid = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => pending.len(),
        };
        if valid > 0 {
            let text = String::from_utf8_lossy(&pending[..valid]);
            let mut value = task.value.lock().await;
            append_output(&mut value, &text);
            url_pending.push_str(&text);
            if let Some(end) = url_pending.rfind(url_delimiter) {
                let end = end + url_pending[end..].chars().next().unwrap().len_utf8();
                scan_urls(&mut value, &url_pending[..end]);
                url_pending.drain(..end);
            }
            if url_pending.len() > OUTPUT_LIMIT { url_pending.clear(); }
            pending.drain(..valid);
        }
    }
    let mut value = task.value.lock().await;
    if !pending.is_empty() {
        let text = String::from_utf8_lossy(&pending);
        append_output(&mut value, &text);
        url_pending.push_str(&text);
    }
    scan_urls(&mut value, &url_pending);
}

fn append_output(value: &mut ManagedTask, chunk: &str) {
    value.output.push_str(chunk);
    if value.output.len() > OUTPUT_LIMIT {
        let mut boundary = value.output.len() - OUTPUT_LIMIT;
        while !value.output.is_char_boundary(boundary) { boundary += 1; }
        value.output.drain(..boundary);
        value.output_truncated = true;
    }
}

fn url_delimiter(c: char) -> bool { c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>') }

fn scan_urls(value: &mut ManagedTask, text: &str) {
    let plain = strip_ansi(text);
    for word in plain.split(url_delimiter) {
        let word = word.trim_start_matches('(').trim_end_matches([',', ';', ')', '.']);
        let Ok(mut url) = reqwest::Url::parse(word) else { continue; };
        if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() { continue; }
        let host = url.host_str().unwrap_or("").trim_matches(['[', ']']);
        let local = host == "localhost" || host.ends_with(".localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
            std::net::IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_unspecified(),
            std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified() || (ip.segments()[0] & 0xfe00) == 0xfc00,
        });
        if !local { continue; }
        if host == "0.0.0.0" { let _ = url.set_host(Some("localhost")); }
        else if host == "::" { let _ = url.set_host(Some("[::1]")); }
        let url = url.to_string();
        if !value.urls.contains(&url) && value.urls.len() < 16 { value.urls.push(url); }
    }
}

fn strip_ansi(value: &str) -> String {
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() == Some('[') { for c in chars.by_ref() { if ('@'..='~').contains(&c) { break; } } }
        } else { result.push(c); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{WorkspaceKind, WorkspaceSettings};

    fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap() }
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!("taucode-managed-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn workspace(path: &Path) -> Mutex<HashMap<String, WorkspaceEntry>> {
        Mutex::new(HashMap::from([("workspace".into(), WorkspaceEntry {
            id: "workspace".into(), name: "Test".into(), path: path.to_string_lossy().into(),
            kind: WorkspaceKind::Main, parent_id: None, worktree: None, settings: WorkspaceSettings::default(),
        })]))
    }
    async fn finished(manager: &ManagedTasks, thread: &str) -> ManagedTask {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let tasks = manager.list("workspace", thread).await;
                if let Some(task) = tasks.into_iter().find(|t| t.status != "running") { return task; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("task should exit")
    }

    #[test]
    #[cfg(unix)]
    fn captures_real_output_exit_timestamps_and_scoped_history() { runtime().block_on(async {
        let path = directory();
        let manager = ManagedTasks::new();
        manager.start("workspace", "thread", &path, "printf 'hello ✓\\nhttp://localhost:5173/\\n'; printf 'error' >&2; exit 7".into(), "Dev".into()).await.unwrap();
        let task = finished(&manager, "thread").await;
        assert_eq!(task.status, "failed");
        assert_eq!(task.exit_code, Some(7));
        assert!(task.ended_at.unwrap() >= task.started_at);
        assert!(task.output.contains("hello ✓"));
        assert!(task.output.contains("error"));
        assert_eq!(task.urls, ["http://localhost:5173/"]);
        assert!(manager.list("workspace", "other").await.is_empty());
        let workspaces = workspace(&path);
        let sessions = Mutex::new(HashMap::new());
        assert!(manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "other".into(), "stop".into(), json!({"id":task.id})).await.is_err());
        manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "other".into(), "clearFinished".into(), json!({})).await.unwrap();
        assert_eq!(manager.list("workspace", "thread").await.len(), 1);
        manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "thread".into(), "clearFinished".into(), json!({})).await.unwrap();
        assert!(manager.list("workspace", "thread").await.is_empty());
        std::fs::remove_dir_all(path).unwrap();
    }); }

    #[test]
    #[cfg(unix)]
    fn stop_kills_only_owned_group_and_clear_preserves_running_tasks() { runtime().block_on(async {
        let path = directory();
        let manager = ManagedTasks::new();
        manager.start("workspace", "thread", &path, "sleep 30 & printf '%s' $! > child.pid; wait".into(), "Sleep".into()).await.unwrap();
        manager.start("workspace", "other", &path, "sleep 30".into(), "Other".into()).await.unwrap();
        let child_pid: i32 = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(pid) = std::fs::read_to_string(path.join("child.pid")) { if let Ok(pid) = pid.parse() { break pid; } }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        let workspaces = workspace(&path);
        let sessions = Mutex::new(HashMap::new());
        manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "thread".into(), "clearFinished".into(), json!({})).await.unwrap();
        let task = manager.list("workspace", "thread").await.pop().unwrap();
        manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "thread".into(), "stop".into(), json!({"id":task.id})).await.unwrap();
        assert_eq!(manager.list("workspace", "thread").await[0].status, "stopped");
        assert_eq!(manager.list("workspace", "other").await[0].status, "running");
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let state = std::process::Command::new("ps").args(["-o", "stat=", "-p", &child_pid.to_string()]).output().unwrap();
                let state = String::from_utf8_lossy(&state.stdout);
                if state.trim().is_empty() || state.trim_start().starts_with('Z') { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("owned descendant should stop");
        let other = manager.list("workspace", "other").await.pop().unwrap();
        manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, "workspace".into(), "other".into(), "stop".into(), json!({"id":other.id})).await.unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }); }

    #[test]
    fn bounded_unicode_output_and_only_actual_local_http_addresses() {
        let mut task = ManagedTask { id: "id".into(), workspace_id: "w".into(), thread_id: "t".into(), command: "dev".into(), label: "Dev".into(), cwd: "/".into(), pid: None, status: "running".into(), started_at: now(), ended_at: None, exit_code: None, output: String::new(), output_truncated: false, urls: vec![] };
        append_output(&mut task, &"✓".repeat(OUTPUT_LIMIT));
        assert!(task.output.len() <= OUTPUT_LIMIT);
        assert!(task.output_truncated);
        append_output(&mut task, "\n\u{1b}[32mhttp://local");
        append_output(&mut task, "host:5173/\u{1b}[0m\nhttp://0.0.0.0:8000/ http://[::1]:8080/ https://docs.example.com http://user:secret@localhost:99/ javascript:alert(1)");
        scan_urls(&mut task, "\u{1b}[32mhttp://localhost:5173/\u{1b}[0m http://0.0.0.0:8000/ http://[::1]:8080/ https://docs.example.com http://user:secret@localhost:99/ javascript:alert(1)");
        assert_eq!(task.urls, ["http://localhost:5173/", "http://localhost:8000/", "http://[::1]:8080/"]);
        scan_urls(&mut task, "Serving HTTP on 127.0.0.1 port 52822 (http://127.0.0.1:52822/) ... (https://docs.example.com) (http://user:secret@localhost:99/)");
        assert_eq!(task.urls.last().unwrap(), "http://127.0.0.1:52822/");
        assert_eq!(task.urls.len(), 4);
        assert!(validate_command(" ").is_err());
        assert!(validate_command("echo\0bad").is_err());
    }

    #[test]
    fn rejects_unknown_workspace_and_unvalidated_session_before_spawn() { runtime().block_on(async {
        let manager = ManagedTasks::new();
        let path = directory();
        let workspaces = workspace(&path);
        let sessions = Mutex::new(HashMap::new());
        for (workspace, thread) in [("unknown", "thread"), ("workspace", ""), ("workspace", "thread")] {
            assert!(manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions, workspace.into(), thread.into(), "start".into(), json!({"command":"echo must-not-run > created"})).await.is_err());
        }
        assert!(manager.tasks.lock().await.is_empty());
        assert!(!path.join("created").exists());
        std::fs::remove_dir_all(path).unwrap();
    }); }
    #[test]
    #[cfg(unix)]
    fn canonical_session_cwd_is_enforced_before_spawn_and_used_for_command() { runtime().block_on(async {
        use crate::backend::app_server::protocol_tests::make_session;
        let path = directory();
        let outside = directory();
        let inside = path.join("subproject");
        std::fs::create_dir(&inside).unwrap();
        std::os::unix::fs::symlink(&outside, path.join("escape")).unwrap();
        let workspaces = workspace(&path);
        let canonical = std::fs::canonicalize(&path).unwrap();
        assert!(validate_session(&canonical, "thread", &json!({"id":"wrong","cwd":inside})).await.is_err());
        assert!(validate_session(&canonical, "thread", &json!({"id":"thread","cwd":path.join("escape")})).await.is_err());
        for (cwd, allowed) in [(&outside, false), (&inside, true)] {
            let manager = ManagedTasks::new();
            let session = make_session();
            session.owners.lock().await.insert("thread".into(), "codex".into());
            let sessions = Mutex::new(HashMap::from([("workspace".into(), session.clone())]));
            let response = json!({"result":{"thread":{"id":"thread","cwd":cwd}}});
            let pending = session.clone();
            let responder = tokio::spawn(async move {
                tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        let request = pending.pending.lock().await.drain().next();
                        if let Some((_, sender)) = request { sender.send(response).unwrap(); break; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.unwrap();
            });
            let result = manager.request(&Mutex::new(AppSettings::default()), &workspaces, &sessions,
                "workspace".into(), "thread".into(), "start".into(), json!({"command":"pwd"})).await;
            responder.await.unwrap();
            if allowed {
                result.unwrap();
                let task = finished(&manager, "thread").await;
                assert_eq!(task.status, "completed");
                assert_eq!(PathBuf::from(task.cwd), std::fs::canonicalize(cwd).unwrap());
                assert_eq!(PathBuf::from(task.output.trim()), std::fs::canonicalize(cwd).unwrap());
            } else {
                assert!(result.unwrap_err().contains("another workspace"));
                assert!(manager.tasks.lock().await.is_empty());
            }
            session.shutdown().await;
        }
        std::fs::remove_dir_all(path).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }); }

    #[test]
    fn terminal_feature_blocks_starts_and_restarts_but_allows_cleanup() { runtime().block_on(async {
        let manager = ManagedTasks::new();
        let path = directory();
        let workspaces = workspace(&path);
        let sessions = Mutex::new(HashMap::new());
        let mut settings = AppSettings::default();
        settings.features.terminal = false;
        let settings = Mutex::new(settings);
        for action in ["start", "restart"] {
            let error = manager.request(&settings, &workspaces, &sessions, "workspace".into(), "thread".into(), action.into(), json!({"command":"echo no"})).await.unwrap_err();
            assert!(error.contains("Enable Terminal"));
        }
        assert!(manager.tasks.lock().await.is_empty());
        assert_eq!(manager.request(&settings, &workspaces, &sessions, "workspace".into(), "thread".into(), "clearFinished".into(), json!({})).await.unwrap(), json!([]));
        std::fs::remove_dir_all(path).unwrap();
    }); }

    #[test]
    #[cfg(unix)]
    fn cancellation_synchronously_terminates_owned_group() {
        let path = directory();
        let executor = runtime();
        let (actor, pid, child_pid) = executor.block_on(async {
            let manager = ManagedTasks::new();
            let actor = manager.start("workspace", "thread", &path, "sleep 30 & printf '%s' $! > child.pid; wait".into(), "Sleep".into()).await.unwrap();
            let pid = manager.list("workspace", "thread").await[0].pid.unwrap();
            let child_pid: i32 = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(value) = std::fs::read_to_string(path.join("child.pid")) { if let Ok(pid) = value.parse() { break pid; } }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }).await.unwrap();
            // Retain the registry so this checks actor cancellation, not its stop channel.
            (actor, pid, (child_pid, manager))
        });
        let (child_pid, manager) = child_pid;
        executor.block_on(async {
            let runner = actor.runner.lock().unwrap().take().unwrap();
            runner.abort();
            assert!(runner.await.unwrap_err().is_cancelled());
        });
        // Cleanup cannot depend on another runtime tick or a pending stop message.
        for process in [pid as i32, child_pid] {
            let status = std::process::Command::new("ps").args(["-o", "stat=", "-p", &process.to_string()]).output().unwrap();
            let state = String::from_utf8_lossy(&status.stdout);
            assert!(state.trim().is_empty() || state.trim_start().starts_with('Z'), "process {process} still runs: {state}");
        }
        drop(manager);
        drop(executor);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn split_output_never_publishes_a_partial_server_address() { runtime().block_on(async {
        let path = directory();
        let manager = ManagedTasks::new();
        manager.start("workspace", "thread", &path, "printf 'http://localhost:5'; sleep 0.2; printf '173/\\342\\200\\203'".into(), "Dev".into()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let values = manager.list("workspace", "thread").await;
                if values[0].output == "http://localhost:5" {
                    assert!(values[0].urls.is_empty());
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        let task = finished(&manager, "thread").await;
        assert_eq!(task.urls, ["http://localhost:5173/"]);
        std::fs::remove_dir_all(path).unwrap();
    }); }

    #[test]
    #[cfg(unix)]
    fn shutdown_joins_owned_groups_preserves_external_process_and_rejects_starts() { runtime().block_on(async {
        let path = directory();
        let manager = ManagedTasks::new();
        let mut external = tokio_command("/bin/sh").args(["-c", "sleep 30"]).kill_on_drop(true).spawn().unwrap();
        manager.start("workspace", "first", &path, "sleep 30 & printf '%s' $! > child.pid; wait".into(), "Server".into()).await.unwrap();
        manager.start("another-workspace", "second", &path, "sleep 30".into(), "Watcher".into()).await.unwrap();
        let descendant: u32 = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(value) = std::fs::read_to_string(path.join("child.pid")) { if let Ok(pid) = value.parse() { break pid; } }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        let (first, second) = tokio::join!(manager.shutdown(), manager.shutdown());
        first.unwrap(); second.unwrap();
        for task in manager.tasks.lock().await.values() {
            let value = task.value.lock().await;
            assert_eq!(value.status, "stopped");
            assert!(value.ended_at.is_some());
            assert!(task.runner.lock().unwrap().is_none());
            assert_eq!(unsafe { libc::kill(value.pid.unwrap() as i32, 0) }, -1, "owned leader should be reaped");
        }
        assert!(external.try_wait().unwrap().is_none(), "unowned process must keep running");
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let status = std::process::Command::new("ps").args(["-o", "stat=", "-p", &descendant.to_string()]).output().unwrap();
                let state = String::from_utf8_lossy(&status.stdout);
                if state.trim().is_empty() || state.trim_start().starts_with('Z') { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("owned descendant should be terminated before shutdown returns");
        assert!(manager.start("workspace", "new", &path, "echo must-not-run".into(), "New".into()).await.err().unwrap().contains("shutting down"));
        external.kill().await.unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }); }

}
