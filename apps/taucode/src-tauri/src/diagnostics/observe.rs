use crate::{
    core::{self, signals::Turn},
    writer,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};
#[derive(Default)]
struct Tracker {
    turns: BTreeMap<String, (Turn, Value)>,
    history: BTreeMap<String, Vec<u64>>,
    contexts: BTreeMap<String, Value>,
    responses: BTreeMap<String, Value>,
    totals: BTreeMap<String, Value>,
    baselines: BTreeMap<String, Value>,
}
static TRACKER: OnceLock<Mutex<Tracker>> = OnceLock::new();
pub fn observe(message: &Value) {
    if !writer::enabled() {
        return;
    }
    let method = message["method"].as_str().unwrap_or("");
    let p = &message["params"];
    let thread = p["threadId"].as_str().unwrap_or("");
    let mut tracker = TRACKER.get_or_init(Default::default).lock().unwrap();
    if method == "turn/started" {
        let turn = &p["turn"];
        let mut context = tracker
            .contexts
            .get(thread)
            .cloned()
            .unwrap_or_else(|| json!({}));
        context["threadId"] = json!(thread);
        context["turnId"] = turn["id"].clone();
        for key in ["engine", "model", "providerKind", "codegraph", "zeroCost"] {
            if !p[key].is_null() {
                context[key] = p[key].clone();
            }
        }
        let baseline = tracker
            .totals
            .get(thread)
            .cloned()
            .unwrap_or_else(|| json!({}));
        tracker.baselines.insert(thread.into(), baseline);
        tracker.turns.insert(
            thread.into(),
            (
                Turn {
                    context,
                    started: Some(Instant::now()),
                    ..Default::default()
                },
                json!({}),
            ),
        );
        return;
    }
    if method == "turn/completed" {
        tracker.baselines.remove(thread);
        if let Some((turn, metrics)) = tracker.turns.remove(thread) {
            if p["turn"]["status"] == "interrupted" {
                writer::record(|| {
                    let mut e = core::event("warning", "agent", "agent.aborted", "Turn aborted");
                    e.context = turn.context.clone();
                    e
                });
            }
            for e in turn.finish(metrics, &mut tracker.history) {
                writer::record(|| e);
            }
        }
        return;
    }
    if method == "error" {
        writer::record(|| {
            let mut e = core::event(
                "error",
                "engine",
                "engine.turn_failed",
                "Engine turn failed",
            );
            e.context = json!({"threadId":thread,"turnId":p["turnId"]});
            e
        });
        if p["willRetry"] == true {
            writer::metadata("warning", "provider", "provider.retry", json!({}));
        }
    }
    if method == "item/completed" && p["item"]["type"] == "agentMessage" {
        if let Some(id) = p["item"]["id"].as_str() {
            if let Some((turn, _)) = tracker.turns.get_mut(thread) {
                turn.context["responseId"] = json!(id);
                let context = turn.context.clone();
                if tracker.responses.len() >= 512 {
                    if let Some(first) = tracker.responses.keys().next().cloned() {
                        tracker.responses.remove(&first);
                    }
                }
                tracker.responses.insert(id.into(), context);
            }
        }
    }
    let mut usage = None;
    if method == "thread/tokenUsage/updated" {
        let total = &p["tokenUsage"]["total"];
        let last = &p["tokenUsage"]["last"];
        let baseline = tracker
            .baselines
            .get(thread)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut value = json!({});
        for (dest, source) in [
            ("tokensIn", "inputTokens"),
            ("tokensOut", "outputTokens"),
            ("cachedTokens", "cachedInputTokens"),
            ("costUsd", "cost"),
        ] {
            let mut amount = total[source]
                .as_f64()
                .map(|n| (n - baseline[source].as_f64().unwrap_or(0.0)).max(0.0))
                .or(last[source].as_f64());
            if dest == "costUsd" && last.get(source).is_some_and(Value::is_null) {
                amount = None;
            }
            value[dest] = if dest == "costUsd" {
                let zero_cost = tracker.turns.get(thread).is_some_and(|(turn, _)| turn.context["zeroCost"] == true);
                json!(amount.filter(|cost| *cost > 0.0 || (*cost == 0.0 && zero_cost)))
            } else {
                json!(amount.unwrap_or(0.0) as u64)
            };
        }
        tracker.totals.insert(thread.into(), total.clone());
        usage = Some(value);
    }
    if let Some((turn, metrics)) = tracker.turns.get_mut(thread) {
        if let Some(usage) = usage {
            *metrics = usage;
        }
        if method == "item/agentMessage/delta" && !turn.first_token {
            turn.first_token = true;
            writer::record(|| {
                let mut e = core::event("perf", "engine", "engine.first_token", "First token");
                e.context = turn.context.clone();
                e.metrics = json!({"durationMs":turn.started.map(|s|s.elapsed().as_millis() as u64).unwrap_or(0)});
                e
            });
        }
        if method == "item/started" {
            let item = &p["item"];
            let kind = item["type"].as_str().unwrap_or("");
            if kind == "contextCompaction" {
                turn.compacted = true;
            }
            if [
                "mcpToolCall",
                "dynamicToolCall",
                "commandExecution",
                "fileChange",
            ]
            .contains(&kind)
            {
                turn.tool(
                    item["tool"].as_str().unwrap_or(kind),
                    item.get("arguments")
                        .or(item.get("command"))
                        .or(item.get("changes"))
                        .unwrap_or(&Value::Null),
                    false,
                );
            }
        }
        if method == "item/completed"
            && (p["item"]["status"] == "failed"
                || p["item"]["success"] == false
                || !p["item"]["error"].is_null())
        {
            turn.failed_tool(
                p["item"]["tool"]
                    .as_str()
                    .or(p["item"]["type"].as_str())
                    .unwrap_or("unknown"),
            );
            writer::record(|| {
                let mut e = core::event("error", "agent", "agent.tool_failed", "Tool failed");
                e.context = turn.context.clone();
                e.detail = json!({"tool":p["item"]["tool"],"errorClass":"ToolError"});
                e
            });
        }
    }
}
pub fn ready(engine: &str, start: Option<Instant>) {
    if let Some(start) = start {
        writer::record(|| {
            let duration = start.elapsed().as_millis() as u64;
            let mut e = core::event(
                "perf",
                "engine",
                "engine.ready",
                "Engine ready",
            );
            e.context = json!({"engine":engine});
            e.metrics = json!({"durationMs":duration});
            e
        });
    }
}

pub fn clear() {
    if let Some(children) = CHILDREN.get() {
        if let Ok(mut children) = children.lock() {
            children.clear();
        }
    }
    if let Some(state) = TRACKER.get() {
        if let Ok(mut state) = state.lock() {
            *state = Tracker::default();
        }
    }
}

#[derive(Default)]
struct Child {
    engine: String,
    tail: std::collections::VecDeque<String>,
}
static CHILDREN: OnceLock<Mutex<BTreeMap<u32, Child>>> = OnceLock::new();
pub fn child(pid: Option<u32>, engine: &str) {
    if !writer::enabled() {
        return;
    }
    if let Some(pid) = pid {
        let mut children = CHILDREN.get_or_init(Default::default).lock().unwrap();
        if children.len() < 64 {
            children.insert(
                pid,
                Child {
                    engine: engine.into(),
                    ..Default::default()
                },
            );
        }
    }
}
pub fn stderr(pid: Option<u32>, line: &str) {
    if !writer::snippets() {
        return;
    }
    if let Some(pid) = pid {
        if let Some(child) = CHILDREN
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .get_mut(&pid)
        {
            if child.tail.len() == 40 {
                child.tail.pop_front();
            }
            child.tail.push_back(line.chars().take(300).collect());
        }
    }
}
pub fn exited(pid: Option<u32>, status: std::process::ExitStatus) {
    if !writer::enabled() {
        return;
    }
    let child = pid.and_then(|pid| {
        CHILDREN
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .remove(&pid)
    });
    if !status.success() {
        writer::record(|| {
            let mut e = core::event(
                "error",
                "engine",
                "engine.exit_nonzero",
                "Engine exited unsuccessfully",
            );
            e.detail = json!({"status":status.code()});
            if let Some(child) = child {
                e.context = json!({"engine":child.engine});
                if writer::snippets() {
                    e.detail["stderrTail"] =
                        json!(child.tail.into_iter().collect::<Vec<_>>().join("\n"));
                }
            }
            e
        });
    }
}
pub fn sample() -> Vec<core::Event> {
    if !writer::enabled() {
        return vec![];
    }
    let Some(children) = CHILDREN.get() else {
        return vec![];
    };
    let mut children = children.lock().unwrap();
    let mut events = vec![];
    children.retain(|pid, child| {
        let Ok(output) = std::process::Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "rss=,pcpu="])
            .output()
        else {
            return true;
        };
        if !output.status.success() {
            return false;
        }
        let values = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter_map(|v| v.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if values.len() == 2 {
            let mut e = core::event(
                "perf",
                "engine",
                "engine.process_usage",
                "Child process sample",
            );
            e.context = json!({"engine":child.engine});
            e.metrics = json!({"rssKb":values[0],"cpuPercent":values[1]});
            events.push(e);
        }
        true
    });
    events
}

pub fn thread_context(thread: &str, context: Value) {
    if !writer::enabled() {
        return;
    }
    let mut state = TRACKER.get_or_init(Default::default).lock().unwrap();
    if state.contexts.len() < 512 || state.contexts.contains_key(thread) {
        state.contexts.insert(thread.into(), context);
    }
}
pub fn response_context(id: &str) -> Option<Value> {
    TRACKER.get()?.lock().ok()?.responses.get(id).cloned()
}
