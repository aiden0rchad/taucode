use super::*;
use futures_util::StreamExt;
use rmcp::{
    service::{RoleClient, RunningService},
    transport::TokioChildProcess,
    ServiceExt,
};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn list(client: RunningService<RoleClient, ()>) -> Result<Value, String> {
    let result = async {
        let tools = client
            .list_all_tools()
            .await
            .map_err(|_| "MCP tools/list failed")?;
        Ok(json!(tools
            .into_iter()
            .map(|t| json!({"name":t.name,"description":t.description}))
            .collect::<Vec<_>>()))
    }
    .await;
    let _ = client.cancel().await;
    result
}

pub(crate) async fn test_connection(
    entry: &McpServerEntry,
    keys: &dyn KeyStore,
    cwd: &Path,
) -> Value {
    let start = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let transport = entry
            .transport
            .as_ref()
            .ok_or("Built-in connector is unavailable; install CodeGraph and restart")?;
        match transport {
            Transport::Stdio {
                command,
                args,
                cwd: directory,
                env,
            } => {
                let mut child = tokio::process::Command::new(command);
                child
                    .args(args)
                    .current_dir(
                        directory
                            .as_deref()
                            .map(Path::new)
                            .map(|p| cwd.join(p))
                            .unwrap_or_else(|| cwd.into()),
                    )
                    .kill_on_drop(true);
                child.stderr(std::process::Stdio::null());
                for (name, value) in env {
                    child.env(name, resolve(entry, name, value, keys)?);
                }
                // The probe is a separate proxy session; it must not reuse a live thread.
                child.env(
                    "TAUCODE_SESSION_KEY",
                    format!("test-{}", uuid::Uuid::new_v4()),
                );
                let transport = TokioChildProcess::new(child)
                    .map_err(|_| "Cannot start connector; check its command and directory")?;
                let client = ()
                    .serve(transport)
                    .await
                    .map_err(|_| "MCP initialization failed; check the server command")?;
                list(client).await
            }
            Transport::Http {
                url,
                headers,
                bearer,
            } => {
                let mut resolved = reqwest::header::HeaderMap::new();
                for (name, value) in headers {
                    resolved.insert(
                        reqwest::header::HeaderName::from_bytes(name.as_bytes())
                            .map_err(|_| "Invalid HTTP header name")?,
                        reqwest::header::HeaderValue::from_str(&resolve(entry, name, value, keys)?)
                            .map_err(|_| "Invalid HTTP header value")?,
                    );
                }
                if let Some(value) = bearer {
                    resolved.insert(
                        reqwest::header::AUTHORIZATION,
                        reqwest::header::HeaderValue::from_str(&format!(
                            "Bearer {}",
                            resolve(entry, "bearer", value, keys)?
                        ))
                        .map_err(|_| "Invalid bearer token")?,
                    );
                }
                // Reuse rmcp for handshake/version negotiation/pagination, with the already
                // linked reqwest transport. No OAuth provider or credential store exists.
                let (client_io, bridge_io) = tokio::io::duplex(64 * 1024);
                let url = url.clone();
                let has_bearer = bearer.is_some();
                let bridge = tokio::spawn(async move {
                    http_bridge(bridge_io, &url, resolved, has_bearer).await
                });
                let _abort = AbortOnDrop(bridge.abort_handle());
                let result = match ().serve(client_io).await {
                    Ok(client) => list(client).await,
                    Err(_) => Err("MCP HTTP initialization failed".into()),
                };
                // A failed HTTP request closes the duplex and wakes rmcp; expose only our
                // sanitized error, never a response body, URL query, or supplied header.
                if let Ok(Ok(Err(error))) =
                    tokio::time::timeout(Duration::from_secs(1), bridge).await
                {
                    return Err(error);
                }
                result
            }
        }
    })
    .await
    .unwrap_or_else(|_| Err("Connection timed out after 15 seconds".into()));
    match result {
        Ok(tools) => json!({"ok":true,"latencyMs":start.elapsed().as_millis(),"tools":tools}),
        Err(error) => json!({"ok":false,"latencyMs":start.elapsed().as_millis(),"error":error}),
    }
}

fn resolve(
    s: &McpServerEntry,
    name: &str,
    value: &Credential,
    keys: &dyn KeyStore,
) -> Result<String, String> {
    match value {
        Credential::Value { value } => Ok(value.clone()),
        Credential::Secret { .. } => keys
            .get(&account(&s.id, name)?)?
            .ok_or_else(|| format!("Save {name} in Keychain first")),
    }
}
fn status_error(status: u16, challenge: &str, has_bearer: bool) -> Option<String> {
    if status == 401
        && challenge
            .to_ascii_lowercase()
            .split(|c: char| c.is_whitespace() || c == ',')
            .any(|v| v == "bearer")
        && !has_bearer
    {
        Some(OAUTH_REQUIRED.into())
    } else if !(200..300).contains(&status) {
        Some(format!(
            "Connector returned HTTP {status}; check the endpoint and credentials"
        ))
    } else {
        None
    }
}

async fn http_bridge(
    io: tokio::io::DuplexStream,
    url: &str,
    mut headers: reqwest::header::HeaderMap,
    has_bearer: bool,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Cannot create HTTP client")?;
    let (read, mut write) = tokio::io::split(io);
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|_| "MCP transport closed")?
    {
        let request: Value = serde_json::from_str(&line).map_err(|_| "Invalid MCP request")?;
        let response = client
            .post(url)
            .headers(headers.clone())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(line)
            .send()
            .await
            .map_err(|_| "Cannot connect; check the endpoint")?;
        if let Some(error) = status_error(
            response.status().as_u16(),
            response
                .headers()
                .get("www-authenticate")
                .and_then(|h| h.to_str().ok())
                .unwrap_or(""),
            has_bearer,
        ) {
            return Err(error);
        }
        if let Some(session) = response.headers().get("mcp-session-id") {
            headers.insert("mcp-session-id", session.clone());
        }
        if request.get("id").is_none() {
            continue;
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"));
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        let mut completed = false;
        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(&chunk.map_err(|_| "Cannot read MCP response")?);
            if buffer.len() > 16 * 1024 * 1024 {
                return Err("MCP response is too large".into());
            }
            let payloads = if sse {
                let text = String::from_utf8_lossy(&buffer).replace("\r\n", "\n");
                let Some(end) = text.rfind("\n\n") else {
                    continue;
                };
                let payloads = text[..end]
                    .split("\n\n")
                    .filter_map(|event| {
                        let data = event
                            .lines()
                            .filter_map(|l| {
                                l.strip_prefix("data:")
                                    .map(|s| s.strip_prefix(' ').unwrap_or(s))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        (!data.is_empty()).then_some(data)
                    })
                    .collect::<Vec<_>>();
                buffer = text[end + 2..].as_bytes().to_vec();
                payloads
            } else {
                match serde_json::from_slice::<Value>(&buffer) {
                    Ok(v) => vec![v.to_string()],
                    Err(_) => continue,
                }
            };
            for payload in payloads {
                let message: Value =
                    serde_json::from_str(&payload).map_err(|_| "Invalid MCP response")?;
                if message.get("id") == request.get("id")
                    && (message.get("result").is_some() || message.get("error").is_some())
                {
                    completed = true;
                    if let Some(version) = message
                        .pointer("/result/protocolVersion")
                        .and_then(Value::as_str)
                    {
                        headers.insert(
                            "mcp-protocol-version",
                            version.parse().map_err(|_| "Invalid MCP version")?,
                        );
                    }
                }
                write
                    .write_all(format!("{payload}\n").as_bytes())
                    .await
                    .map_err(|_| "MCP transport closed")?;
            }
            if completed {
                break;
            }
        }
        if !completed {
            return Err("Server closed without an MCP response".into());
        }
    }
    if headers.contains_key("mcp-session-id") {
        let _ = client.delete(url).headers(headers).send().await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake;
    impl rmcp::ServerHandler for Fake {
        fn get_info(&self) -> rmcp::model::ServerInfo {
            let mut info = rmcp::model::ServerInfo::default();
            info.capabilities = rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build();
            info
        }
        async fn list_tools(
            &self,
            _: Option<rmcp::model::PaginatedRequestParams>,
            _: rmcp::service::RequestContext<rmcp::RoleServer>,
        ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
            let mut result = rmcp::model::ListToolsResult::default();
            result.tools.push(serde_json::from_value(json!({"name":"read","description":"Read a document","inputSchema":{"type":"object"}})).unwrap());
            Ok(result)
        }
    }
    #[tokio::test]
    async fn handshake_and_tools_use_in_memory_transport() {
        let (client, server) = tokio::io::duplex(4096);
        let server =
            tokio::spawn(async move { Fake.serve(server).await.unwrap().waiting().await.unwrap() });
        let tools = list(().serve(client).await.unwrap()).await.unwrap();
        assert_eq!(
            tools,
            json!([{"name":"read","description":"Read a document"}])
        );
        server.await.unwrap();
    }
    #[test]
    fn oauth_and_auth_errors_are_actionable() {
        assert_eq!(
            status_error(401, "Bearer realm=\"mcp\"", false).as_deref(),
            Some(OAUTH_REQUIRED)
        );
        assert!(!status_error(401, "Bearer", true).unwrap().contains("OAuth"));
        assert!(status_error(200, "", false).is_none());
    }
}
