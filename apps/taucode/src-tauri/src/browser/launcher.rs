use super::{Browser, Settings};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, Write},
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub fn arguments(cli: &Path, data: &Path, port: u16, settings: &Settings) -> Vec<String> {
    let mut args = vec![
        cli.to_string_lossy().into(),
        "--cdp-endpoint".into(),
        format!("http://127.0.0.1:{port}"),
        "--output-dir".into(),
        data.join("browser/output").to_string_lossy().into(),
    ];
    for (flag, origins) in [
        ("--allowed-origins", &settings.allowed_origins),
        ("--blocked-origins", &settings.blocked_origins),
    ] {
        if !origins.is_empty() {
            args.extend([flag.into(), origins.join(";")]);
        }
    }
    args
}
pub fn current_url(result: &Value) -> Option<String> {
    result["content"]
        .as_array()?
        .iter()
        .filter_map(|c| c["text"].as_str())
        .flat_map(str::lines)
        .find_map(|line| {
            if !line.strip_prefix("- ").is_some_and(|s| {
                s.split_once(": (current) [")
                    .is_some_and(|(index, _)| index.parse::<usize>().is_ok())
            }) {
                return None;
            }
            let (_, url) = line.rsplit_once("](")?;
            Some(url.strip_suffix(')')?.into())
        })
}
fn selection(result: &Value, id: &str) -> Option<Value> {
    let index = result["content"]
        .as_array()?
        .iter()
        .filter_map(|c| c["text"].as_str())
        .flat_map(str::lines)
        .find_map(|line| {
            line.strip_prefix("- ")?
                .split_once(": (current) [")?
                .0
                .parse::<usize>()
                .ok()
        })?;
    Some(
        json!({"jsonrpc":"2.0","id":format!("{id}-select"),"method":"tools/call","params":{"name":"browser_tabs","arguments":{"action":"select","index":index}}}),
    )
}
pub async fn run(data: PathBuf) -> Result<(), String> {
    let mut signal = super::signal::Shutdown::new()?;
    let browser = Browser::new(data);
    browser.serve().await?;
    let result = proxy(&browser, &mut signal).await;
    // Only stops a browser owned by this launcher (daemon-less mode).
    browser.shutdown().await;
    result
}
async fn proxy(
    browser: &std::sync::Arc<Browser>,
    signal: &mut super::signal::Shutdown,
) -> Result<(), String> {
    let settings = browser.settings()?;
    super::validate(&settings)?;
    let cli = match std::env::var_os("TAUCODE_BROWSER_CLI") {
        Some(cli) => PathBuf::from(cli),
        None => crate::shared::engine_runtime::prepare(&browser.data, "browser", None).await?
            .join("node_modules/@playwright/mcp/cli.js"),
    };
    if !cli.is_file() {
        return Err(
            "Playwright MCP runtime missing. Install engines/browser (@playwright/mcp@0.0.83)."
                .into(),
        );
    }
    // Playwright lists its unchanged schemas without connecting to CDP. Recreate
    // the MCP session with the real endpoint only on the first browser tool.
    let mut child = spawn(&cli, &browser.data, 1, &settings)?;
    let mut port = None;
    let mut initialize = None;
    let mut input = child.stdin.take().ok_or("Missing MCP input")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("Missing MCP output")?).lines();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if tx.blocking_send(line).is_err() {
                break;
            }
        }
    });
    let mut calls: HashSet<String> = HashSet::new();
    let tab_id = format!("tau-browser-tabs-{}", uuid::Uuid::new_v4());
    let mut pending = HashMap::new();
    let mut sequence = 0;
    let result = async {
        loop {
            tokio::select! {
                _ = signal.wait() => break,
                line = rx.recv() => {
                    let Some(line) = line else { break; };
                    let line = line.map_err(|e| e.to_string())?;
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if v["method"] == "initialize" { initialize = Some(v.clone()); }
                        if v["method"] == "notifications/cancelled" {
                            for id in cancel_pending(&mut pending, &v["params"]["requestId"]) {
                                let cancel = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id}});
                                input.write_all(format!("{cancel}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                            }
                            calls.remove(&v["params"]["requestId"].to_string());
                            let _ = browser.request(json!({"action":"activity","active":!calls.is_empty()})).await;
                        }
                        if v["method"] == "tools/call" {
                            let endpoint = match browser.request(json!({"action":"ensure"})).await {
                                Ok(endpoint) => endpoint,
                                Err(error) => {
                                    emit(&json!({"jsonrpc":"2.0","id":v["id"],"result":{"isError":true,"content":[{"type":"text","text":error}]}}).to_string())?;
                                    continue;
                                }
                            };
                            let current_port = endpoint["port"].as_u64().ok_or("Missing browser debugging port")? as u16;
                            if port != Some(current_port) {
                                let mut init = initialize.clone().ok_or("Initialize MCP before calling browser tools")?;
                                let _ = child.kill().await; let _ = child.wait().await;
                                for id in calls.drain() {
                                    let id: Value = serde_json::from_str(&id).map_err(|e| e.to_string())?;
                                    emit(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":"Browser restarted; retry the tool call"}}).to_string())?;
                                }
                                pending.clear();
                                child = spawn(&cli,&browser.data,current_port,&settings)?;
                                input = child.stdin.take().ok_or("Missing MCP input")?;
                                output = BufReader::new(child.stdout.take().ok_or("Missing MCP output")?).lines();
                                let init_id = format!("{tab_id}-initialize");
                                init["id"] = json!(init_id);
                                input.write_all(format!("{init}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                                tokio::time::timeout(std::time::Duration::from_secs(30), async {
                                    loop {
                                        let line = output.next_line().await.map_err(|e| e.to_string())?.ok_or("Playwright MCP exited during initialization")?;
                                        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                                        if response["id"] == init_id {
                                            if response.get("error").is_some() { return Err(response["error"].to_string()); }
                                            return Ok(());
                                        }
                                    }
                                }).await.map_err(|_| "Playwright MCP initialization timed out")??;
                                input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await.map_err(|e| e.to_string())?;
                                port = Some(current_port);
                            }
                            calls.insert(v["id"].to_string());
                            let _ = browser.request(json!({"action":"activity","active":true})).await;
                            sequence += 1;
                            let id = format!("{tab_id}-{sequence}");
                            let query = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"browser_tabs","arguments":{"action":"list"}}});
                            pending.insert(id, line);
                            input.write_all(format!("{query}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                            continue;
                        }
                    }
                    input.write_all(format!("{line}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                }
                line = output.next_line() => {
                    let Some(line) = line.map_err(|e| e.to_string())? else { break; };
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if let Some(original) = v["id"].as_str().and_then(|id| pending.remove(id)) {
                            let id = v["id"].as_str().unwrap();
                            if !id.ends_with("-select") {
                                if let Some(query) = selection(&v["result"], id) {
                                    pending.insert(format!("{id}-select"), original);
                                    input.write_all(format!("{query}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                                    continue;
                                }
                            }
                            if let Some(url) = current_url(&v["result"]) { let _ = browser.request(json!({"action":"agentTab","url":url})).await; }
                            input.write_all(format!("{original}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                            continue;
                        }
                        if v["id"] == tab_id {
                            if let Some(query) = selection(&v["result"], &tab_id) {
                                input.write_all(format!("{query}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                            }
                            continue;
                        }
                        if v["id"] == format!("{tab_id}-select") {
                            if let Some(url) = current_url(&v["result"]) { let _ = browser.request(json!({"action":"agentTab","url":url})).await; }
                            continue;
                        }
                        if v["id"].as_str().is_some_and(|id| id.starts_with(&tab_id)) { continue; }
                        if calls.remove(&v["id"].to_string()) {
                            let query = json!({"jsonrpc":"2.0","id":tab_id,"method":"tools/call","params":{"name":"browser_tabs","arguments":{"action":"list"}}});
                            input.write_all(format!("{query}\n").as_bytes()).await.map_err(|e| e.to_string())?;
                            let _ = browser.request(json!({"action":"activity","active":!calls.is_empty()})).await;
                        }
                    }
                    emit(&line)?;
                }
            }
        }
        Ok(())
    }.await;
    if port.is_some() {
        let _ = browser
            .request(json!({"action":"activity","active":false}))
            .await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

fn emit(line: &str) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")
        .and_then(|_| stdout.flush())
        .map_err(|e| e.to_string())
}

pub(super) fn cancel_pending(
    pending: &mut HashMap<String, String>,
    request: &Value,
) -> Vec<String> {
    let ids: Vec<_> = pending
        .iter()
        .filter(|(_, line)| serde_json::from_str::<Value>(line).is_ok_and(|v| v["id"] == *request))
        .map(|(id, _)| id.clone())
        .collect();
    for id in &ids {
        pending.remove(id);
    }
    ids
}
pub(super) fn command(
    cli: &Path,
    data: &Path,
    port: u16,
    settings: &Settings,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(
        crate::shared::engine_runtime::node_path(),
    );
    command
        .args(arguments(cli, data, port, settings))
        .current_dir(data.join("browser/output"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}
pub(super) fn spawn(
    cli: &Path,
    data: &Path,
    port: u16,
    settings: &Settings,
) -> Result<tokio::process::Child, String> {
    std::fs::create_dir_all(data.join("browser/output")).map_err(|e| e.to_string())?;
    command(cli, data, port, settings)
        .spawn()
        .map_err(|e| format!("Cannot start Playwright MCP: {e}"))
}
