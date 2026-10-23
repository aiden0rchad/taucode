use std::{sync::Arc, time::Duration};

use anyhow::Result;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{broadcast, mpsc},
    time::sleep,
};

use super::{Pending, ServerRequests};

#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    Notification(Value),
    ServerRequest(Value),
    Stderr(String),
    Exit { code: Option<i32> },
}

pub(super) async fn write_messages<W>(
    mut writer: W,
    mut messages: mpsc::Receiver<Value>,
    events: broadcast::Sender<EngineEvent>,
) where
    W: AsyncWrite + Unpin,
{
    while let Some(message) = messages.recv().await {
        let result: Result<()> = async {
            let bytes = serde_json::to_vec(&message)?;
            writer.write_all(&bytes).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            let _ = events.send(EngineEvent::Stderr(format!("protocol write error: {error}")));
            break;
        }
    }
}

pub(super) async fn read_stdout<R>(
    reader: R,
    pending: Pending,
    server_requests: ServerRequests,
    writer: mpsc::Sender<Value>,
    events: broadcast::Sender<EngineEvent>,
    server_request_timeout: Duration,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => match serde_json::from_str::<Value>(&line) {
                Ok(message) => route_message(
                    message,
                    &pending,
                    &server_requests,
                    &writer,
                    &events,
                    server_request_timeout,
                ),
                Err(error) => {
                    let _ = events.send(EngineEvent::Stderr(format!(
                        "protocol parse error: {error}: {line}"
                    )));
                }
            },
            Ok(None) => break,
            Err(error) => {
                let _ = events.send(EngineEvent::Stderr(format!(
                    "protocol read error: {error}"
                )));
                break;
            }
        }
    }
}

pub(super) async fn read_stderr<R>(reader: R, events: broadcast::Sender<EngineEvent>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let _ = events.send(EngineEvent::Stderr(line));
            }
            Ok(None) => break,
            Err(error) => {
                let _ = events.send(EngineEvent::Stderr(format!("stderr read error: {error}")));
                break;
            }
        }
    }
}

fn route_message(
    message: Value,
    pending: &Pending,
    server_requests: &ServerRequests,
    writer: &mpsc::Sender<Value>,
    events: &broadcast::Sender<EngineEvent>,
    server_request_timeout: Duration,
) {
    let Some(object) = message.as_object() else {
        protocol_error(events, "non-object protocol message");
        return;
    };

    if object.contains_key("method") {
        if object.get("method").and_then(Value::as_str).is_none() {
            protocol_error(events, "protocol method is not a string");
        } else if let Some(id) = object.get("id") {
            let key = id_key(id);
            server_requests
                .lock()
                .expect("server request mutex poisoned")
                .insert(key.clone());
            let _ = events.send(EngineEvent::ServerRequest(message.clone()));
            expire_server_request(
                id.clone(),
                key,
                Arc::clone(server_requests),
                writer.clone(),
                server_request_timeout,
            );
        } else {
            let _ = events.send(EngineEvent::Notification(message));
        }
        return;
    }

    let Some(id) = object.get("id").and_then(Value::as_u64) else {
        protocol_error(events, "response has no numeric id");
        return;
    };
    let Some(sender) = pending
        .lock()
        .expect("pending request mutex poisoned")
        .remove(&id)
    else {
        protocol_error(events, &format!("response for unknown request {id}"));
        return;
    };
    let reply = if let Some(result) = object.get("result") {
        Ok(result.clone())
    } else if let Some(error) = object.get("error") {
        Err(format_rpc_error(error))
    } else {
        Err("malformed response: expected result or error".to_owned())
    };
    let _ = sender.send(reply);
}

fn expire_server_request(
    id: Value,
    key: String,
    server_requests: ServerRequests,
    writer: mpsc::Sender<Value>,
    duration: Duration,
) {
    tokio::spawn(async move {
        sleep(duration).await;
        let expired = server_requests
            .lock()
            .expect("server request mutex poisoned")
            .remove(&key);
        if expired {
            let _ = writer
                .send(json!({
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                }))
                .await;
        }
    });
}

pub(super) fn id_key(id: &Value) -> String {
    serde_json::to_string(id).expect("serializing a JSON value cannot fail")
}

fn protocol_error(events: &broadcast::Sender<EngineEvent>, message: &str) {
    let _ = events.send(EngineEvent::Stderr(format!("protocol error: {message}")));
}

fn format_rpc_error(error: &Value) -> String {
    let code = error.get("code").map_or_else(
        || "unknown".to_owned(),
        |value| value.to_string(),
    );
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown RPC error");
    format!("RPC error {code}: {message}")
}
