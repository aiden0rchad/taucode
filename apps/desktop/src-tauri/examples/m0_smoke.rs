use std::{env, io::Write, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use desktop_lib::engine::{Engine, EngineEvent};
use serde_json::{Map, Value, json};
use tokio::{sync::broadcast, time::timeout};

struct Args {
    cwd: String,
    prompt: String,
    model: Option<String>,
    provider: Option<String>,
    auto_approve: bool,
    fork_provider: Option<String>,
    fork_model: Option<String>,
    fork_prompt: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    let engine = Engine::spawn().await?;
    let mut events = engine.subscribe();
    let first = start_thread(&engine, &args.cwd, args.model, args.provider).await?;
    print_thread(&first)?;
    let first_id = thread_id(&first)?.to_owned();
    run_turn(
        &engine,
        &mut events,
        &first_id,
        &args.prompt,
        args.auto_approve,
    )
    .await?;

    if args.fork_provider.is_some() || args.fork_model.is_some() || args.fork_prompt.is_some() {
        let prompt = args
            .fork_prompt
            .context("--fork-prompt is required when forking")?;
        let fork = fork_thread(&engine, &first_id, args.fork_model, args.fork_provider).await?;
        print_thread(&fork)?;
        let fork_id = thread_id(&fork)?.to_owned();
        run_turn(
            &engine,
            &mut events,
            &fork_id,
            &prompt,
            args.auto_approve,
        )
        .await?;
    }

    engine.shutdown().await?;
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut cwd = None;
    let mut prompt = None;
    let mut model = None;
    let mut provider = None;
    let mut auto_approve = false;
    let mut fork_provider = None;
    let mut fork_model = None;
    let mut fork_prompt = None;
    let mut args = env::args().skip(1);

    while let Some(flag) = args.next() {
        let target = match flag.as_str() {
            "--cwd" => &mut cwd,
            "--prompt" => &mut prompt,
            "--model" => &mut model,
            "--provider" => &mut provider,
            "--fork-provider" => &mut fork_provider,
            "--fork-model" => &mut fork_model,
            "--fork-prompt" => &mut fork_prompt,
            "--auto-approve" => {
                auto_approve = true;
                continue;
            }
            "-h" | "--help" => {
                println!(
                    "m0_smoke --cwd DIR --prompt TEXT [--model M] [--provider P] \
                     [--auto-approve] [--fork-provider P2 --fork-model M2 --fork-prompt T]"
                );
                std::process::exit(0);
            }
            _ => bail!("unknown argument: {flag}"),
        };
        *target = Some(
            args.next()
                .ok_or_else(|| anyhow!("{flag} requires a value"))?,
        );
    }

    Ok(Args {
        cwd: cwd.context("--cwd is required")?,
        prompt: prompt.context("--prompt is required")?,
        model,
        provider,
        auto_approve,
        fork_provider,
        fork_model,
        fork_prompt,
    })
}

async fn start_thread(
    engine: &Engine,
    cwd: &str,
    model: Option<String>,
    provider: Option<String>,
) -> Result<Value> {
    let mut params = Map::from_iter([("cwd".to_owned(), Value::String(cwd.to_owned()))]);
    insert(&mut params, "model", model);
    insert(&mut params, "modelProvider", provider);
    engine.request("thread/start", Value::Object(params)).await
}

async fn fork_thread(
    engine: &Engine,
    thread_id: &str,
    model: Option<String>,
    provider: Option<String>,
) -> Result<Value> {
    let mut params = Map::from_iter([(
        "threadId".to_owned(),
        Value::String(thread_id.to_owned()),
    )]);
    insert(&mut params, "model", model);
    insert(&mut params, "modelProvider", provider);
    engine.request("thread/fork", Value::Object(params)).await
}

async fn run_turn(
    engine: &Engine,
    events: &mut broadcast::Receiver<EngineEvent>,
    thread_id: &str,
    prompt: &str,
    auto_approve: bool,
) -> Result<()> {
    let response = engine
        .request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": prompt, "text_elements": [] }]
            }),
        )
        .await?;
    let turn_id = response["turn"]["id"]
        .as_str()
        .context("turn/start response has no turn id")?;
    println!("turn {turn_id}");

    timeout(Duration::from_secs(600), async {
        let mut inline_delta = false;
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    if inline_delta {
                        println!();
                        inline_delta = false;
                    }
                    println!("warning event stream lagged by {count} messages");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    bail!("engine event stream closed")
                }
            };
            match event {
                EngineEvent::Notification(message) => {
                    let method = message["method"].as_str().unwrap_or("unknown");
                    if method == "item/agentMessage/delta" {
                        print!("{}", message["params"]["delta"].as_str().unwrap_or_default());
                        std::io::stdout().flush()?;
                        inline_delta = true;
                    } else {
                        if inline_delta {
                            println!();
                            inline_delta = false;
                        }
                        println!("{method} {}", summary(&message));
                    }

                    if method == "turn/completed"
                        && message["params"]["turn"]["id"] == turn_id
                    {
                        let status = message["params"]["turn"]["status"]
                            .as_str()
                            .unwrap_or("unknown");
                        if status == "completed" {
                            return Ok(());
                        }
                        let error = message["params"]["turn"]["error"]["message"]
                            .as_str()
                            .unwrap_or("unknown error");
                        bail!("turn {turn_id} ended with status {status}: {error}");
                    }
                }
                EngineEvent::ServerRequest(message) => {
                    answer_server_request(engine, message, auto_approve).await?;
                }
                EngineEvent::Stderr(line) => eprintln!("stderr {line}"),
                EngineEvent::Exit { code } => bail!("app-server exited with code {code:?}"),
            }
        }
    })
    .await
    .context("turn timed out after 10 minutes")?
}

async fn answer_server_request(
    engine: &Engine,
    message: Value,
    auto_approve: bool,
) -> Result<()> {
    let method = message["method"].as_str().unwrap_or("unknown");
    let id = message["id"].clone();
    if matches!(
        method,
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
    ) {
        let decision = if auto_approve { "accept" } else { "decline" };
        println!(
            "{method} item={} {decision}",
            message["params"]["itemId"].as_str().unwrap_or("unknown")
        );
        engine
            .respond_to_server_request(id, Some(json!({ "decision": decision })), None)
            .await
    } else {
        println!("{method} unsupported");
        engine
            .respond_to_server_request(
                id,
                None,
                Some(json!({ "code": -32601, "message": "Method not found" })),
            )
            .await
    }
}

fn insert(params: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        params.insert(key.to_owned(), Value::String(value));
    }
}

fn thread_id(response: &Value) -> Result<&str> {
    response["thread"]["id"]
        .as_str()
        .context("thread response has no thread id")
}

fn print_thread(response: &Value) -> Result<()> {
    let id = thread_id(response)?;
    let provider = response["modelProvider"]
        .as_str()
        .or_else(|| response["thread"]["modelProvider"].as_str())
        .unwrap_or("unknown");
    println!("thread {id} modelProvider={provider}");
    Ok(())
}

fn summary(message: &Value) -> String {
    match message["method"].as_str() {
        Some("error") => format!(
            "{} (willRetry={})",
            message["params"]["error"]["message"]
                .as_str()
                .unwrap_or("unknown error"),
            message["params"]["willRetry"]
                .as_bool()
                .map_or_else(|| "unknown".to_owned(), |retry| retry.to_string())
        ),
        Some("warning") => message["params"]["message"]
            .as_str()
            .unwrap_or("unknown warning")
            .to_owned(),
        Some("item/started" | "item/completed") => message["params"]["item"]["type"]
            .as_str()
            .unwrap_or("item")
            .to_owned(),
        Some("item/commandExecution/outputDelta") => format!(
            "{} bytes",
            message["params"]["delta"]
                .as_str()
                .map_or(0, str::len)
        ),
        Some("turn/started") => message["params"]["turn"]["status"]
            .as_str()
            .unwrap_or("turn")
            .to_owned(),
        Some("turn/completed") => {
            let status = message["params"]["turn"]["status"]
                .as_str()
                .unwrap_or("turn");
            match message["params"]["turn"]["error"]["message"].as_str() {
                Some(error) => format!("{status}: {error}"),
                None => status.to_owned(),
            }
        }
        _ => String::new(),
    }
}
