pub mod epoch;

use std::{collections::HashMap, ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use rmcp::{
    ClientHandler, ErrorData, Peer, RoleClient, RoleServer, ServerHandler, ServiceExt,
    model::*,
    service::{
        NotificationContext, PeerRequestOptions, RequestContext, RunningService, RxJsonRpcMessage,
        ServiceError, TxJsonRpcMessage,
    },
    transport::{TokioChildProcess, Transport},
};
use tokio::{
    process::Command,
    sync::{Mutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore, mpsc},
    time::{Instant, timeout},
};

const MAX_THREADS: usize = 32;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
type Upstream = RunningService<RoleClient, UpstreamHandler>;

#[derive(Clone)]
struct OriginalMeta(Option<RequestMetaObject>);

// rmcp assigns its own progressToken after request options are applied. Restore
// the caller's metadata at the transport boundary, without replacing MCP framing.
struct PreserveMeta(TokioChildProcess);

impl Transport<RoleClient> for PreserveMeta {
    type Error = std::io::Error;

    fn send(
        &mut self,
        mut message: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        if let JsonRpcMessage::Request(message) = &mut message
            && let ClientRequest::CallToolRequest(request) = &mut message.request
            && let Some(meta) = request.extensions.remove::<OriginalMeta>()
        {
            request.params.meta = meta.0;
            request.extensions.remove::<RequestMetaObject>();
        }
        self.0.send(message)
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        self.0.receive().await
    }
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.0.close().await
    }
}

// Same cancellation-on-drop pattern as codex's rmcp-client/src/tool_request.rs.
struct CancelOnDrop(Option<(Peer<RoleClient>, RequestId)>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some((peer, id)) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = peer
                    .notify_cancelled(CancelledNotificationParam::new(
                        Some(id),
                        Some("tool call response future dropped".into()),
                    ))
                    .await;
            });
        }
    }
}

struct UpstreamHandler {
    changes: Option<mpsc::UnboundedSender<()>>,
}

impl ClientHandler for UpstreamHandler {
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        if let Some(changes) = &self.changes {
            let _ = changes.send(());
        }
    }
}

struct Session {
    service: Upstream,
    epoch: u64,
    // Held through child shutdown, including eviction, to bound live processes.
    _permit: Option<OwnedSemaphorePermit>,
}

struct Slot {
    session: Option<Session>,
    used: Instant,
}

impl Slot {
    fn empty() -> Self {
        Self {
            session: None,
            used: Instant::now(),
        }
    }

    async fn close(&mut self) {
        if let Some(mut session) = self.session.take() {
            let _ = session.service.close().await;
        }
    }
}

pub struct Proxy {
    command: Vec<OsString>,
    state_dir: Option<PathBuf>,
    default_key: String,
    info: ServerConfig,
    slots: Mutex<HashMap<String, Arc<Mutex<Slot>>>>,
    permits: Arc<Semaphore>,
    changes: mpsc::UnboundedSender<()>,
    change_rx: Mutex<Option<mpsc::UnboundedReceiver<()>>>,
}

impl Proxy {
    pub async fn new(command: Vec<OsString>, state_dir: Option<PathBuf>) -> Result<Self> {
        anyhow::ensure!(
            !command.is_empty(),
            "usage: tau-mcp-proxy -- <upstream cmd> [args...]"
        );
        let (changes, change_rx) = mpsc::unbounded_channel();
        let mut proxy = Self {
            default_key: std::env::var("TAUCODE_SESSION_KEY").unwrap_or_else(|_| "default".into()),
            command,
            state_dir,
            info: ServerConfig::default(),
            slots: Mutex::new(HashMap::new()),
            permits: Arc::new(Semaphore::new(MAX_THREADS)),
            changes,
            change_rx: Mutex::new(Some(change_rx)),
        };
        let session = proxy
            .spawn(&proxy.default_key, proxy.epoch(&proxy.default_key))
            .await?;
        let info = session
            .service
            .peer_info()
            .context("upstream sent no server info")?;
        proxy.info.protocol_version = info.protocol_version.clone();
        proxy.info.server_info = info
            .server_info
            .clone()
            .context("upstream sent no implementation info")?;
        proxy.info.instructions = info.instructions.clone();
        proxy.info.meta = info.meta.clone();
        proxy.info.capabilities.tools = info.capabilities.tools.clone();
        proxy.slots.get_mut().insert(
            proxy.default_key.clone(),
            Arc::new(Mutex::new(Slot {
                session: Some(session),
                used: Instant::now(),
            })),
        );
        Ok(proxy)
    }

    fn epoch(&self, key: &str) -> u64 {
        self.state_dir
            .as_ref()
            .map_or(0, |dir| epoch::read_epoch(dir, key))
    }

    async fn spawn(&self, key: &str, epoch: u64) -> Result<Session> {
        let permit = if key == self.default_key {
            None
        } else {
            Some(self.permits.clone().acquire_owned().await?)
        };
        let mut command = Command::new(&self.command[0]);
        command.args(&self.command[1..]).kill_on_drop(true);
        let service = timeout(
            Duration::from_secs(30),
            UpstreamHandler {
                changes: (key == self.default_key).then(|| self.changes.clone()),
            }
            .serve(PreserveMeta(TokioChildProcess::new(command)?)),
        )
        .await
        .context("upstream initialization timed out")??;
        Ok(Session {
            service,
            epoch,
            _permit: permit,
        })
    }

    async fn session(&self, key: &str) -> Result<OwnedMutexGuard<Slot>, ErrorData> {
        let (slot, evicted) = {
            let mut slots = self.slots.lock().await;
            let mut evicted = None;
            if !slots.contains_key(key) && slots.len() > MAX_THREADS {
                let oldest = slots
                    .iter()
                    .filter(|(key, slot)| {
                        key.as_str() != self.default_key && Arc::strong_count(slot) == 1
                    })
                    .filter_map(|(key, slot)| {
                        slot.try_lock().ok().map(|slot| (key.clone(), slot.used))
                    })
                    .min_by_key(|(_, used)| *used)
                    .map(|(key, _)| key);
                let oldest = oldest.ok_or_else(|| {
                    ErrorData::internal_error("all upstream sessions are busy", None)
                })?;
                evicted = slots.remove(&oldest);
            }
            let slot = slots
                .entry(key.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(Slot::empty())))
                .clone();
            (slot, evicted)
        };
        if let Some(evicted) = evicted {
            evicted.lock().await.close().await;
        }
        let mut slot = slot.lock_owned().await;
        let epoch = self.epoch(key);
        if slot
            .session
            .as_ref()
            .is_some_and(|session| session.epoch != epoch)
            || slot.used.elapsed() >= IDLE_TIMEOUT
        {
            slot.close().await;
        }
        slot.used = Instant::now();
        if slot.session.is_none() {
            slot.session = Some(
                self.spawn(key, epoch)
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
            );
        }
        Ok(slot)
    }

    pub async fn reap_idle(&self) {
        let mut retired = Vec::new();
        {
            let mut slots = self.slots.lock().await;
            slots.retain(|key, slot| {
                if Arc::strong_count(slot) == 1
                    && let Ok(mut slot) = slot.try_lock()
                    && slot.used.elapsed() >= IDLE_TIMEOUT
                {
                    retired.extend(slot.session.take());
                    return key == &self.default_key;
                }
                true
            });
        }
        for mut session in retired {
            let _ = session.service.close().await;
        }
    }

    pub async fn shutdown(&self) {
        let slots = std::mem::take(&mut *self.slots.lock().await);
        let mut closing = tokio::task::JoinSet::new();
        for slot in slots.into_values() {
            closing.spawn(async move {
                slot.lock().await.close().await;
            });
        }
        while closing.join_next().await.is_some() {}
    }

    async fn finish<T>(slot: &mut Slot, result: Result<T, ServiceError>) -> Result<T, ErrorData> {
        slot.used = Instant::now();
        match result {
            Ok(value) => Ok(value),
            Err(ServiceError::McpError(error)) => Err(error),
            Err(error) => {
                slot.close().await;
                Err(ErrorData::internal_error(error.to_string(), None))
            }
        }
    }

    async fn forward_changes(&self, peer: Peer<RoleServer>) {
        if let Some(mut changes) = self.change_rx.lock().await.take() {
            tokio::spawn(async move {
                while changes.recv().await.is_some() {
                    if peer.notify_tool_list_changed().await.is_err() {
                        break;
                    }
                }
            });
        }
    }
}

impl ServerHandler for Proxy {
    fn get_info(&self) -> ServerConfig {
        self.info.clone()
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        context.peer.set_peer_info(request);
        Ok(self.info.clone())
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        self.forward_changes(context.peer).await;
    }

    async fn list_tools(
        &self,
        mut request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if !context.meta.is_empty() {
            request.get_or_insert_with(Default::default).meta = Some(context.meta);
        }
        let mut slot = self.session(&self.default_key).await?;
        let result = slot
            .session
            .as_ref()
            .expect("initialized session")
            .service
            // The SDK's list_tools helper caches responses; a proxy must forward each list.
            .send_request(ClientRequest::ListToolsRequest(ListToolsRequest {
                method: Default::default(),
                params: request,
                extensions: Default::default(),
            }))
            .await
            .and_then(|result| match result {
                ServerResult::ListToolsResult(result) => Ok(result),
                _ => Err(ServiceError::UnexpectedResponse),
            });
        Self::finish(&mut slot, result).await
    }

    async fn call_tool(
        &self,
        mut request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let key = context
            .meta
            .get("threadId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&self.default_key)
            .to_owned();
        if !context.meta.is_empty() {
            request.meta = Some(context.meta);
        }
        let mut slot = self.session(&key).await?;
        let mut request = CallToolRequest::new(request);
        request
            .extensions
            .insert(OriginalMeta(request.params.meta.clone()));
        let service = &slot.session.as_ref().expect("initialized session").service;
        let result = async {
            let handle = service
                .send_request_with_option(
                    ClientRequest::CallToolRequest(request),
                    PeerRequestOptions::no_options(),
                )
                .await?;
            let mut cancellation = CancelOnDrop(Some((service.peer().clone(), handle.id.clone())));
            let result = handle.await_response().await;
            cancellation.0 = None;
            match result? {
                ServerResult::CallToolResult(result) => Ok(CallToolResponse::Complete(result)),
                ServerResult::InputRequiredResult(result) => {
                    Ok(CallToolResponse::InputRequired(result))
                }
                ServerResult::CreateTaskResult(result) => Ok(CallToolResponse::Task(result)),
                _ => Err(ServiceError::UnexpectedResponse),
            }
        }
        .await;
        Self::finish(&mut slot, result).await
    }
}

/// Run the stdio proxy with its own runtime, before host initialization.
pub fn run_cli(args: Vec<OsString>) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut args = args.into_iter();
            anyhow::ensure!(
                args.next().as_deref() == Some(std::ffi::OsStr::new("--")),
                "usage: tau-mcp-proxy -- <upstream cmd> [args...]"
            );
            let proxy = Arc::new(
                Proxy::new(
                    args.collect(),
                    std::env::var_os("TAUCODE_STATE_DIR").map(PathBuf::from),
                )
                .await?,
            );
            let result = async {
                let service = proxy.clone().serve(rmcp::transport::stdio()).await?;
                let waiting = service.waiting();
                tokio::pin!(waiting);
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    tokio::select! {
                        result = &mut waiting => { result?; break; }
                        _ = interval.tick() => proxy.reap_idle().await,
                    }
                }
                anyhow::Ok(())
            }
            .await;
            proxy.shutdown().await;
            result
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reaps_idle_slots_but_keeps_busy_and_default_slots() {
        let (changes, change_rx) = mpsc::unbounded_channel();
        let expired = || {
            Arc::new(Mutex::new(Slot {
                session: None,
                used: Instant::now() - IDLE_TIMEOUT,
            }))
        };
        let busy = expired();
        let proxy = Proxy {
            default_key: "default".into(),
            command: vec![],
            state_dir: None,
            info: ServerConfig::default(),
            slots: Mutex::new(HashMap::from([
                ("default".into(), expired()),
                ("idle".into(), expired()),
                ("busy".into(), busy.clone()),
                ("recent".into(), Arc::new(Mutex::new(Slot::empty()))),
            ])),
            permits: Arc::new(Semaphore::new(MAX_THREADS)),
            changes,
            change_rx: Mutex::new(Some(change_rx)),
        };
        proxy.reap_idle().await;
        let slots = proxy.slots.lock().await;
        assert_eq!(slots.len(), 3);
        assert!(!slots.contains_key("idle"));
        assert!(slots.contains_key("default"));
        assert!(slots.contains_key("busy"));
        assert!(slots.contains_key("recent"));
    }
}
