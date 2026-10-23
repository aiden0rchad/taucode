mod rpc;

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::{
    process::Command,
    sync::{broadcast, mpsc, oneshot},
    time::timeout,
};

pub use rpc::EngineEvent;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;
type ServerRequests = Arc<Mutex<HashSet<String>>>;

const EVENT_CAPACITY: usize = 1_024;
const WRITER_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub request_timeout: Duration,
    pub server_request_timeout: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            program: std::env::var_os("TAUCODE_CODEX_BIN")
                .unwrap_or_else(|| OsString::from("codex")),
            args: vec![OsString::from("app-server")],
            request_timeout: Duration::from_secs(120),
            server_request_timeout: Duration::from_secs(600),
        }
    }
}

#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

struct Inner {
    writer: mpsc::Sender<Value>,
    process: mpsc::Sender<ProcessCommand>,
    pending: Pending,
    server_requests: ServerRequests,
    events: broadcast::Sender<EngineEvent>,
    initial_events: Mutex<Option<broadcast::Receiver<EngineEvent>>>,
    next_id: AtomicU64,
    request_timeout: Duration,
    running: Arc<AtomicBool>,
}

struct ProcessCommand {
    done: oneshot::Sender<Result<(), String>>,
}

impl Engine {
    pub async fn spawn() -> Result<Self> {
        Self::spawn_with_config(EngineConfig::default()).await
    }

    pub async fn spawn_with_config(config: EngineConfig) -> Result<Self> {
        let mut child = Command::new(&config.program)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to start {:?}", config.program))?;

        let stdin = child.stdin.take().context("app-server stdin unavailable")?;
        let stdout = child.stdout.take().context("app-server stdout unavailable")?;
        let stderr = child.stderr.take().context("app-server stderr unavailable")?;
        let (writer, writer_rx) = mpsc::channel(WRITER_CAPACITY);
        let (events, initial_events) = broadcast::channel(EVENT_CAPACITY);
        let (process, process_rx) = mpsc::channel(1);
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let server_requests = Arc::new(Mutex::new(HashSet::new()));
        let inner = Arc::new(Inner {
            writer: writer.clone(),
            process,
            pending: Arc::clone(&pending),
            server_requests: Arc::clone(&server_requests),
            events: events.clone(),
            initial_events: Mutex::new(Some(initial_events)),
            next_id: AtomicU64::new(1),
            request_timeout: config.request_timeout,
            running: Arc::new(AtomicBool::new(true)),
        });

        tokio::spawn(rpc::write_messages(stdin, writer_rx, events.clone()));
        tokio::spawn(rpc::read_stdout(
            stdout,
            pending.clone(),
            server_requests.clone(),
            writer,
            events.clone(),
            config.server_request_timeout,
        ));
        tokio::spawn(rpc::read_stderr(stderr, events.clone()));
        tokio::spawn(supervise(
            child,
            process_rx,
            pending,
            server_requests,
            events,
            Arc::clone(&inner.running),
        ));

        let engine = Self { inner };
        if let Err(error) = engine
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "taucode",
                        "title": "taucode",
                        "version": "0.0.1"
                    }
                }),
            )
            .await
        {
            let _ = engine.shutdown().await;
            return Err(error).context("app-server initialization failed");
        }
        engine.notify("initialized", None).await?;
        Ok(engine)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner
            .initial_events
            .lock()
            .expect("initial event receiver mutex poisoned")
            .take()
            .unwrap_or_else(|| self.inner.events.subscribe())
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        if !self.is_running() {
            bail!("app-server is not running");
        }

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .expect("pending request mutex poisoned")
            .insert(id, reply_tx);
        if !self.is_running() {
            self.remove_pending(id);
            bail!("app-server is not running");
        }

        if self
            .inner
            .writer
            .send(json!({ "id": id, "method": method, "params": params }))
            .await
            .is_err()
        {
            self.remove_pending(id);
            bail!("app-server writer stopped");
        }

        match timeout(self.inner.request_timeout, reply_rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(error))) => Err(anyhow!(error)),
            Ok(Err(_)) => Err(anyhow!("app-server stopped before responding")),
            Err(_) => {
                self.remove_pending(id);
                Err(anyhow!(
                    "request {id} ({method}) timed out after {:?}",
                    self.inner.request_timeout
                ))
            }
        }
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        if !self.is_running() {
            bail!("app-server is not running");
        }
        let mut message = serde_json::Map::from_iter([
            ("method".to_owned(), Value::String(method.to_owned())),
        ]);
        if let Some(params) = params {
            message.insert("params".to_owned(), params);
        }
        self.inner
            .writer
            .send(Value::Object(message))
            .await
            .map_err(|_| anyhow!("app-server writer stopped"))
    }

    pub async fn respond_to_server_request(
        &self,
        id: Value,
        result: Option<Value>,
        error: Option<Value>,
    ) -> Result<()> {
        if result.is_some() == error.is_some() {
            bail!("provide exactly one of result or error");
        }
        let key = rpc::id_key(&id);
        if !self
            .inner
            .server_requests
            .lock()
            .expect("server request mutex poisoned")
            .remove(&key)
        {
            bail!("server request {id} is unknown or already answered");
        }

        let message = match (result, error) {
            (Some(result), None) => json!({ "id": id, "result": result }),
            (None, Some(error)) => json!({ "id": id, "error": error }),
            _ => unreachable!(),
        };
        self.inner
            .writer
            .send(message)
            .await
            .map_err(|_| anyhow!("app-server writer stopped"))
    }

    pub async fn shutdown(&self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        let (done, wait) = oneshot::channel();
        if self
            .inner
            .process
            .send(ProcessCommand { done })
            .await
            .is_err()
        {
            return if self.is_running() {
                Err(anyhow!("app-server supervisor stopped"))
            } else {
                Ok(())
            };
        }
        wait.await
            .map_err(|_| anyhow!("app-server supervisor stopped"))?
            .map_err(anyhow::Error::msg)
    }

    fn remove_pending(&self, id: u64) {
        self.inner
            .pending
            .lock()
            .expect("pending request mutex poisoned")
            .remove(&id);
    }
}

async fn supervise(
    mut child: tokio::process::Child,
    mut commands: mpsc::Receiver<ProcessCommand>,
    pending: Pending,
    server_requests: ServerRequests,
    events: broadcast::Sender<EngineEvent>,
    running: Arc<AtomicBool>,
) {
    let mut shutdown = None;
    let status = tokio::select! {
        status = child.wait() => status,
        command = commands.recv() => {
            if let Some(command) = command {
                shutdown = Some(command.done);
            }
            let _ = child.start_kill();
            child.wait().await
        }
    };

    running.store(false, Ordering::Release);
    let code = status.as_ref().ok().and_then(std::process::ExitStatus::code);
    let reason = match &status {
        Ok(status) => format!("app-server exited with {status}"),
        Err(error) => format!("failed waiting for app-server: {error}"),
    };
    for sender in pending
        .lock()
        .expect("pending request mutex poisoned")
        .drain()
        .map(|(_, sender)| sender)
    {
        let _ = sender.send(Err(reason.clone()));
    }
    server_requests
        .lock()
        .expect("server request mutex poisoned")
        .clear();
    let _ = events.send(EngineEvent::Exit { code });
    if let Some(done) = shutdown {
        let result = status.map(|_| ()).map_err(|error| error.to_string());
        let _ = done.send(result);
    }
}
