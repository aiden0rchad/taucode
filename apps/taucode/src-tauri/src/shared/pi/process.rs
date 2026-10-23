use super::translate::{self, Translator};
use crate::codegraph::CodegraphSetup;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{oneshot, Mutex},
    time::{timeout, Instant},
};

pub(super) type Emit = Arc<dyn Fn(Value) + Send + Sync>;
pub(super) struct Process {
    pub translator: Mutex<Translator>,
    pub operation: Mutex<()>,
    pub alive: AtomicBool,
    pub running: AtomicBool,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
    dialogs: Mutex<HashMap<String, Value>>,
    next_id: AtomicU64,
    touched: Mutex<Instant>,
    emit: Emit,
    graph: Option<CodegraphSetup>,
}
impl Process {
    pub async fn spawn(
        mut command: Command,
        translator: Translator,
        emit: Emit,
        graph: Option<CodegraphSetup>,
        idle: Duration,
    ) -> Result<Arc<Self>, String> {
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot start pi: {e}"))?;
        let stdout = child.stdout.take().ok_or("Pi stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Pi stderr unavailable")?;
        let stdin = child.stdin.take();
        let p = Arc::new(Self {
            translator: Mutex::new(translator),
            operation: Mutex::new(()),
            alive: AtomicBool::new(true),
            running: AtomicBool::new(false),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            dialogs: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            touched: Mutex::new(Instant::now()),
            emit,
            graph,
        });
        let reader = p.clone();
        tokio::spawn(async move {
            // split(b'\n') is deliberately LF-only (including U+2028/U+2029 in strings).
            let mut lines = BufReader::new(stdout).split(b'\n');
            let reason = loop {
                match lines.next_segment().await {
                    Ok(Some(line)) if line.iter().all(u8::is_ascii_whitespace) => continue,
                    Ok(Some(line)) => match serde_json::from_slice::<Value>(&line) {
                        Ok(m) => reader.receive(m).await,
                        Err(e) => break format!("Pi returned invalid JSON: {e}"),
                    },
                    Ok(None) => break "Pi process exited unexpectedly".into(),
                    Err(e) => break format!("Pi stdout failed: {e}"),
                }
            };
            reader.alive.store(false, Ordering::SeqCst);
            let running = reader.running.swap(false, Ordering::SeqCst);
            let mut t = reader.translator.lock().await;
            if running && !t.active {
                for e in t.apply(&json!({"type":"agent_start"})) {
                    (reader.emit)(e);
                }
            }
            if t.active {
                for e in t.fail(&reason) {
                    (reader.emit)(e);
                }
            }
            drop(t);
            for (_, tx) in reader.pending.lock().await.drain() {
                let _ = tx.send(Err(reason.clone()));
            }
            let thread = reader.translator.lock().await.thread.clone();
            for (id, _) in reader.dialogs.lock().await.drain() {
                (reader.emit)(
                    json!({"method":"serverRequest/resolved","params":{"threadId":thread,"requestId":id}}),
                );
            }
        });
        let stderr_emit = p.emit.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).split(b'\n');
            while let Ok(Some(line)) = lines.next_segment().await {
                eprintln!("pi: {}", String::from_utf8_lossy(&line));
                stderr_emit(
                    json!({"method":"codex/stderr","params":{"message":String::from_utf8_lossy(&line)}}),
                );
            }
        });
        let weak = Arc::downgrade(&p);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(idle.min(Duration::from_secs(30))).await;
                let Some(p) = weak.upgrade() else {
                    break;
                };
                if !p.alive.load(Ordering::SeqCst) {
                    break;
                }
                let _operation = p.operation.lock().await;
                if !p.running.load(Ordering::SeqCst)
                    && !p.translator.lock().await.active
                    && p.pending.lock().await.is_empty()
                    && p.touched.lock().await.elapsed() >= idle
                {
                    let _ = p.shutdown().await;
                    break;
                }
            }
        });
        Ok(p)
    }
    pub(super) async fn receive(self: &Arc<Self>, m: Value) {
        *self.touched.lock().await = Instant::now();
        if m["type"] == "response" {
            if let Some(id) = m["id"].as_str() {
                if let Some(tx) = self.pending.lock().await.remove(id) {
                    let _ = tx.send(if m["success"] == true {
                        Ok(m["data"].clone())
                    } else {
                        Err(m["error"].as_str().unwrap_or("Pi command failed").into())
                    });
                }
            }
            return;
        }
        if m["type"] == "extension_ui_request" {
            let method = m["method"].as_str().unwrap_or("");
            if translate::login_requested(&m) {
                for event in self.translator.lock().await.fail(
                    "Accounts and logins are disabled. Configure an API key provider in Settings.",
                ) {
                    (self.emit)(event);
                }
                let _ = self
                    .write(json!({"type":"extension_ui_response","id":m["id"],"cancelled":true}))
                    .await;
                return;
            }
            if !matches!(method, "select" | "confirm" | "input" | "editor") {
                eprintln!("pi extension UI: {m}");
                (self.emit)(
                    json!({"method":"codex/stderr","params":{"message":format!("pi extension UI: {m}")}}),
                );
                return;
            }
            let t = self.translator.lock().await;
            let request = translate::ui_request(&m, &t.thread, &t.turn_id());
            drop(t);
            let id = request["id"].as_str().unwrap().to_owned();
            self.dialogs.lock().await.insert(id.clone(), m.clone());
            (self.emit)(request);
            if let Some(ms) = m["timeout"].as_u64() {
                let weak = Arc::downgrade(self);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    if let Some(p) = weak.upgrade() {
                        let _ = p.answer(&id, json!({})).await;
                    }
                });
            }
            return;
        }
        let mut t = self.translator.lock().await;
        for e in t.apply(&m) {
            (self.emit)(e);
        }
        if m["type"] == "agent_start" {
            self.running.store(true, Ordering::SeqCst);
        }
        if m["type"] == "agent_settled" {
            self.running.store(false, Ordering::SeqCst);
        }
        if m["type"] == "compaction_end"
            && m["errorMessage"].is_null()
            && m["aborted"] != true
            && m["result"].is_object()
        {
            if let Some(g) = &self.graph {
                g.bump_epoch(&t.thread);
            }
        }
    }
    pub async fn write(&self, value: Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or("Pi process is closed")?
            .write_all(&bytes)
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn request(&self, mut value: Value) -> Result<Value, String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err("Pi process exited; retry to resume the session".into());
        }
        *self.touched.lock().await = Instant::now();
        let id = format!("tau-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        value["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        if let Err(e) = self.write(value).await {
            self.pending.lock().await.remove(&id);
            return Err(e);
        }
        let result = match timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => Err("Pi request cancelled".into()),
            Err(_) => Err("Pi request timed out after 300 seconds".into()),
        };
        self.pending.lock().await.remove(&id);
        result
    }
    pub async fn answer(&self, id: &str, result: Value) -> Result<(), String> {
        let request = self
            .dialogs
            .lock()
            .await
            .remove(id)
            .ok_or("Pi question expired or was already answered")?;
        self.write(translate::ui_response(&request, &result))
            .await?;
        let thread = self.translator.lock().await.thread.clone();
        (self.emit)(
            json!({"method":"serverRequest/resolved","params":{"threadId":thread,"requestId":id}}),
        );
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        // Drain stdout while waiting: pi may still be finishing a write on EOF.
        self.stdin.lock().await.take();
        let mut child = self.child.lock().await;
        if timeout(Duration::from_secs(2), child.wait()).await.is_err() {
            crate::shared::process_core::kill_child_process_tree(&mut child).await;
            let _ = child.wait().await;
        }
        self.alive.store(false, Ordering::SeqCst);
        Ok(())
    }
}

pub(super) async fn check_node(node: &str) -> Result<(), String> {
    static VERSION: tokio::sync::OnceCell<Result<(), String>> = tokio::sync::OnceCell::const_new();
    VERSION.get_or_init(||async {
        let output=Command::new(node).arg("--version").output().await.map_err(|e|format!("Pi requires Node >=22.19. Install Node or set TAUCODE_NODE: {e}"))?;
        let version=String::from_utf8_lossy(&output.stdout);let parts:Vec<_>=version.trim().trim_start_matches('v').split('.').filter_map(|s|s.parse::<u64>().ok()).collect();
        if !output.status.success() || parts.len()<2 || (parts[0],parts[1])<(22,19){Err(format!("Pi requires Node >=22.19; found {}. Install a supported Node or set TAUCODE_NODE.",version.trim()))}else{Ok(())}
    }).await.clone()
}
