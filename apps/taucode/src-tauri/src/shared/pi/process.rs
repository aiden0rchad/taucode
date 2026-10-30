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
    cancelling: AtomicBool,
    pub extensions: Mutex<std::collections::HashSet<String>>,
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
    dialogs: Mutex<HashMap<String, Value>>,
    next_id: AtomicU64,
    touched: Mutex<Instant>,
    diagnostic_start: Mutex<Option<std::time::Instant>>,
    emit: Emit,
    graph: Option<CodegraphSetup>,
    omp: bool,
}
impl Process {
    pub async fn spawn(
        mut command: Command,
        mut translator: Translator,
        emit: Emit,
        graph: Option<CodegraphSetup>,
        idle: Duration,
        secrets: Vec<String>,
    ) -> Result<Arc<Self>, String> {
        let omp = command.as_std().get_envs().any(|(k,v)| k == "TAUCODE_OMP" && v.is_some());
        if omp {translator.diagnostic_engine="omp".into();}
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let diagnostic_start = taucode_diag_mcp::writer::enabled().then(std::time::Instant::now);
        let mut child = command
            .spawn()
            .map_err(|e| { taucode_diag_mcp::writer::record(|| {let mut event=taucode_diag_mcp::core::event("error","engine","engine.spawn_failed","Engine spawn failed");event.context=serde_json::json!({"engine":"pi"});event.detail=serde_json::json!({"errorClass":format!("{:?}",e.kind())});event});e.to_string() })?;
        let diagnostic_pid=child.id();
        taucode_diag_mcp::observe::child(diagnostic_pid,if omp {"omp"} else {"pi"});
        let stdout = child.stdout.take().ok_or("Pi stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Pi stderr unavailable")?;
        let stdin = child.stdin.take();
        let p = Arc::new(Self {
            translator: Mutex::new(translator),
            operation: Mutex::new(()),
            alive: AtomicBool::new(true),
            running: AtomicBool::new(false),
            cancelling: AtomicBool::new(false),
            extensions: Mutex::new(Default::default()),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            dialogs: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            touched: Mutex::new(Instant::now()),
            diagnostic_start: Mutex::new(diagnostic_start),
            emit,
            graph,
            omp,
        });
        let reader = p.clone();
        let output_secrets = secrets.clone();
        tokio::spawn(async move {
            // split(b'\n') is deliberately LF-only (including U+2028/U+2029 in strings).
            let mut lines = BufReader::new(stdout).split(b'\n');
            let reason = loop {
                match lines.next_segment().await {
                    Ok(Some(line)) if line.iter().all(u8::is_ascii_whitespace) => continue,
                    Ok(Some(line)) => match serde_json::from_str::<Value>(&redact(
                        String::from_utf8_lossy(&line).into_owned(),
                        &output_secrets,
                    )) {
                        Ok(m) => reader.receive(m).await,
                        Err(e) => {taucode_diag_mcp::writer::record(||taucode_diag_mcp::core::event("error","engine","engine.protocol_decode","Pi protocol decode failed"));break format!("Pi returned invalid JSON: {e}");},
                    },
                    Ok(None) => break "Pi process exited unexpectedly".into(),
                    Err(e) => break format!("Pi stdout failed: {e}"),
                }
            };
            if let Ok(Some(status)) = reader.child.lock().await.try_wait() {
                if taucode_diag_mcp::writer::enabled() { taucode_diag_mcp::observe::exited(diagnostic_pid, status); }
            }
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
                let line = redact(String::from_utf8_lossy(&line).into_owned(), &secrets);
                taucode_diag_mcp::observe::stderr(diagnostic_pid,&line);
                eprintln!("pi: {line}");
                stderr_emit(json!({"method":"codex/stderr","params":{"message":line}}));
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
    pub(super) async fn receive(self: &Arc<Self>, mut m: Value) {
        if self.omp && m["type"] == "agent_end" { m["type"] = json!("agent_settled"); }
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
            if method == "setStatus" && m["statusKey"] == "taucode-verification" {
                if let Some(record) = m["statusText"].as_str().and_then(|text| serde_json::from_str::<Value>(text).ok()) {
                    for event in self.translator.lock().await.apply(&json!({"type":"taucode_verification","record":record})) {
                        (self.emit)(event);
                    }
                }
                return;
            }
            if method == "setStatus" && m["statusText"] == "ready" {
                if let Some(key @ ("taucode-gate" | "taucode-subagents" | "taucode-work")) = m["statusKey"].as_str()
                {
                    self.extensions.lock().await.insert(key.into());
                    return;
                }
            }
            if translate::approval(&m).is_none() && translate::login_requested(&m) {
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
            if self.cancelling.load(Ordering::SeqCst) {
                let _ = self.write(json!({"type":"extension_ui_response","id":m["id"],"cancelled":true})).await;
                return;
            }
            let mut t = self.translator.lock().await;
            // tool_call approval can precede tool_execution_start in pi.
            if let Some(a) = translate::approval(&m) {
                for event in t.approval_item(&a) {
                    (self.emit)(event);
                }
            }
            let request = translate::ui_request(&m, &t.thread, &t.turn_id());
            drop(t);
            let id = request["id"].as_str().unwrap().to_owned();
            let mut dialogs = self.dialogs.lock().await;
            if self.cancelling.load(Ordering::SeqCst) {
                drop(dialogs);
                let _ = self.write(json!({"type":"extension_ui_response","id":m["id"],"cancelled":true})).await;
                return;
            }
            dialogs.insert(id.clone(), m.clone());
            (self.emit)(request);
            drop(dialogs);
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
        if value["type"] == "prompt" { self.cancelling.store(false, Ordering::SeqCst); }
        if value["type"] == "abort" {
            self.cancelling.store(true, Ordering::SeqCst);
            self.translator.lock().await.apply(&json!({"type":"abort_requested"}));
            // Pi waits for tool hooks to settle before acknowledging abort. An
            // unanswered extension dialog otherwise holds that wait forever.
            let dialogs = std::mem::take(&mut *self.dialogs.lock().await);
            let thread = self.translator.lock().await.thread.clone();
            for (id, request) in dialogs {
                self.write(json!({"type":"extension_ui_response","id":request["id"],"cancelled":true})).await?;
                (self.emit)(json!({"method":"serverRequest/resolved","params":{"threadId":thread,"requestId":id}}));
            }
        }
        let id = format!("tau-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        value["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        let initial_state = value["type"] == "get_state";
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
        if initial_state && result.is_ok() {
            taucode_diag_mcp::observe::ready(if self.omp {"omp"} else {"pi"}, self.diagnostic_start.lock().await.take());
        }
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

pub(super) fn redact(mut text: String, secrets: &[String]) -> String {
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        let escaped = serde_json::to_string(secret).unwrap();
        text = text
            .replace(&escaped[1..escaped.len() - 1], "[REDACTED]")
            .replace(secret, "[REDACTED]");
    }
    text
}
