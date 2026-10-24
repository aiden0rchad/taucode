//! JSON-RPC 2.0 over ACP's LF-delimited stdio transport.
use crate::shared::providers::BoxFuture;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{oneshot, Mutex},
    time::Instant,
};

pub(super) const AUTH_ERROR: &str = "Claude Code rejected the API key (no claude.ai login is used)";
type Handler = Arc<dyn Fn(Value) -> BoxFuture<'static, Option<Value>> + Send + Sync>;
#[derive(Clone, Debug)]
pub(super) struct RpcError {
    pub message: String,
    pub error_kind: Option<Value>,
}
impl From<String> for RpcError {
    fn from(message: String) -> Self {
        Self { message, error_kind: None }
    }
}
impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(kind) = &self.error_kind { write!(f, " (errorKind: {kind})")?; }
        Ok(())
    }
}
type Response = oneshot::Receiver<Result<Value, RpcError>>;
pub(super) struct Client {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>,
    next: AtomicU64,
    pub alive: AtomicBool,
    pub running: AtomicUsize,
    touched: Mutex<Instant>,
    pub operation: Mutex<()>,
    pub capabilities: Mutex<Value>,
    pub id: String,
}
pub(super) fn redact(mut text: String, secrets: &[String]) -> String {
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        text = text.replace(secret, "[REDACTED]");
    }
    text
}
fn redact_value(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => *text = redact(std::mem::take(text), secrets),
        Value::Array(values) => values.iter_mut().for_each(|v| redact_value(v, secrets)),
        Value::Object(values) => values.values_mut().for_each(|v| redact_value(v, secrets)),
        _ => (),
    }
}
impl Client {
    pub async fn spawn(
        mut command: Command,
        handler: Handler,
        secrets: Vec<String>,
    ) -> Result<Arc<Self>, String> {
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot start Claude Code: {e}"))?;
        let stdout = child.stdout.take().ok_or("ACP stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("ACP stderr unavailable")?;
        let stdin = child.stdin.take().ok_or("ACP stdin unavailable")?;
        let client = Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(Some(stdin)),
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            alive: AtomicBool::new(true),
            running: AtomicUsize::new(0),
            touched: Mutex::new(Instant::now()),
            operation: Mutex::new(()),
            capabilities: Mutex::new(Value::Null),
            id: uuid::Uuid::new_v4().to_string(),
        });
        let reader = client.clone();
        let output_secrets = secrets.clone();
        let stderr_handler = handler.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).split(b'\n');
            let error = loop {
                let line = match lines.next_segment().await {
                    Ok(Some(line)) => line,
                    Ok(None) => break "Claude Code process exited".to_string(),
                    Err(e) => break format!("ACP stdout failed: {e}"),
                };
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let mut message: Value = match serde_json::from_slice(&line) {
                    Ok(v) => v,
                    Err(_) => break "ACP returned invalid JSON".into(),
                };
                redact_value(&mut message, &output_secrets);
                *reader.touched.lock().await = Instant::now();
                if message.get("method").is_some() {
                    let id = message.get("id").cloned();
                    if let Some(result) = handler(message).await {
                        if let Some(id) = id {
                            let _ = reader.respond(id, result).await;
                        }
                    }
                } else if let Some(id) = message["id"].as_u64() {
                    if let Some(sender) = reader.pending.lock().await.remove(&id) {
                        let result = if message.get("error").is_some() {
                            Err(RpcError { message: if message["error"]["code"] == -32000 {
                                AUTH_ERROR.into()
                            } else {
                                message["error"]["message"]
                                    .as_str()
                                    .unwrap_or("ACP request failed")
                                    .to_string()
                            }, error_kind: message["error"]["data"].get("errorKind").cloned() })
                        } else {
                            Ok(message["result"].clone())
                        };
                        let _ = sender.send(result);
                    }
                }
            };
            reader.alive.store(false, Ordering::SeqCst);
            for (_, sender) in reader.pending.lock().await.drain() {
                let _ = sender.send(Err(error.clone().into()));
            }
            handler(json!({"method":"acp/disconnected","params":{"message":error}})).await;
        });
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).split(b'\n');
            while let Ok(Some(line)) = lines.next_segment().await {
                stderr_handler(json!({"method":"codex/stderr","params":{"message":redact(String::from_utf8_lossy(&line).into_owned(), &secrets)}})).await;
            }
        });
        let weak = Arc::downgrade(&client);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let Some(c) = weak.upgrade() else {
                    break;
                };
                if !c.alive.load(Ordering::SeqCst) {
                    break;
                }
                let _operation = c.operation.lock().await;
                if c.running.load(Ordering::SeqCst) == 0
                    && c.pending.lock().await.is_empty()
                    && c.touched.lock().await.elapsed() >= Duration::from_secs(600)
                {
                    let _ = c.shutdown().await;
                    break;
                }
            }
        });
        Ok(client)
    }
    async fn write(&self, message: Value) -> Result<(), String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err("Claude Code process is not running".into());
        }
        *self.touched.lock().await = Instant::now();
        let mut bytes = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or("ACP stdin is closed")?
            .write_all(&bytes)
            .await
            .map_err(|e| e.to_string())
    }
    pub async fn begin(&self, method: &str, params: Value) -> Result<(u64, Response), String> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        if let Err(e) = self
            .write(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(e);
        }
        Ok((id, rx))
    }
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let (id, rx) = self.begin(method, params).await?;
        // Prompts can wait indefinitely for the user's approval; other RPCs are bounded.
        if method == "session/prompt" {
            return rx.await.map_err(|_| "ACP response channel closed")?.map_err(|e| e.to_string());
        }
        match tokio::time::timeout(Duration::from_secs(120), rx).await {
            Ok(result) => result.map_err(|_| "ACP response channel closed")?.map_err(|e| e.to_string()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(format!("ACP {method} timed out"))
            }
        }
    }
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await
    }
    pub async fn respond(&self, id: Value, result: Value) -> Result<(), String> {
        self.write(json!({"jsonrpc":"2.0","id":id,"result":result}))
            .await
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        self.alive.store(false, Ordering::SeqCst);
        // EOF invokes the adapter's dispose(), closing native Claude and MCP children.
        self.stdin.lock().await.take();
        let mut child = self.child.lock().await;
        if tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .is_err()
        {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                // The process group was created by this client at spawn.
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            crate::shared::process_core::kill_child_process_tree(&mut child).await;
        }
        Ok(())
    }
}
