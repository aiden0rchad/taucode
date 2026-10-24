use super::translate::text;
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) fn capped(messages: impl IntoIterator<Item = Value>) -> Value {
    let mut kept = std::collections::VecDeque::new();
    let mut bytes = 64;
    let mut truncated = false;
    for message in messages {
        let size = message.to_string().len() + 1;
        bytes += size;
        kept.push_back((message, size));
        while kept.len() > 500 || bytes > 2 * 1024 * 1024 {
            bytes -= kept.pop_front().unwrap().1;
            truncated = true;
        }
    }
    json!({"messages":kept.into_iter().map(|(m,_)|m).collect::<Vec<_>>(),"truncated":truncated})
}

pub(super) fn tree(
    prefix: &str,
    results: &[Value],
    previous: &Value,
    done: bool,
    transcripts: &mut HashMap<String, Value>,
) -> Value {
    let now = chrono::Utc::now().timestamp_millis();
    json!(results.iter().enumerate().map(|(i, r)| {
        let id = format!("{prefix}:agent:{i}");
        let old = &previous[i];
        let status = if r["stopReason"] == "aborted" || r["status"] == "aborted" { "aborted" }
            else if r["status"] == "failed" || r["stopReason"] == "error" || r["exitCode"].as_i64().is_some_and(|n| n > 0) { "failed" }
            else if r["status"] == "pending" && done { "aborted" }
            else if r["status"] == "completed" || done || r["stopReason"] == "stop" { "completed" }
            else if r["status"] == "pending" { "pending" } else { "running" };
        let messages = r["messages"].as_array().cloned().unwrap_or_default();
        let mut nested = Vec::new();
        let mut calls = Vec::<(&str, &Value)>::new();
        for message in &messages {
            if let (Some(call), Some(_)) = (message["toolCallId"].as_str(), message["details"]["results"].as_array()) {
                if let Some(existing) = calls.iter_mut().find(|(id,_)| *id == call) { existing.1 = message; }
                else { calls.push((call, message)); }
            }
        }
        for (call, message) in calls {
            let child_prefix = format!("{id}/{call}");
            let prior = json!(old["children"].as_array().into_iter().flatten().filter(|n| n["id"].as_str().is_some_and(|s| s.starts_with(&format!("{child_prefix}:agent:")))).cloned().collect::<Vec<_>>());
            let mut children = tree(&child_prefix, message["details"]["results"].as_array().unwrap(), &prior, done && !matches!(status, "aborted" | "failed"), transcripts);
            if matches!(status, "aborted" | "failed") { settle(&mut children, status); }
            nested.extend(children.as_array().unwrap().iter().cloned());
        }
        let last = messages.iter().rev().find(|m| m["role"] == "assistant")
            .map(|m|text(&m["content"])).unwrap_or_default();
        transcripts.insert(id.clone(), capped(messages.into_iter().map(|mut m| {
            // Nested transcripts belong to their own nodes; do not duplicate them.
            if let Some(details) = m.get_mut("details").and_then(Value::as_object_mut) { details.remove("results"); }
            m
        })));
        let mut usage = r["usage"].as_object().cloned().unwrap_or_default();
        for key in ["input", "output", "cacheRead", "cacheWrite"] {
            usage.insert(key.into(), json!(r["usage"][key].as_u64().unwrap_or(0) + nested.iter().map(|n|n["usage"][key].as_u64().unwrap_or(0)).sum::<u64>()));
        }
        usage.insert("cost".into(), json!(r["usage"]["cost"].as_f64().unwrap_or(0.0) + nested.iter().map(|n|n["usage"]["cost"].as_f64().unwrap_or(0.0)).sum::<f64>()));
        json!({"id":id,"agent":r["agent"],"model":r["model"],"task":r["task"],"status":status,
            "startedAt":if status == "pending" {Value::Null} else {json!(old["startedAt"].as_i64().or(r["startedAt"].as_i64()).unwrap_or(now))},
            "endedAt":if matches!(status,"running"|"pending") {Value::Null} else {json!(old["endedAt"].as_i64().unwrap_or(now))},
            "usage":usage,"lastMessage":last.chars().take(300).collect::<String>(),
            "errorMessage":r["stderr"].as_str().filter(|s|!s.is_empty()).or(r["errorMessage"].as_str()),"children":nested})
    }).collect::<Vec<_>>())
}

pub(super) fn settle(nodes: &mut Value, status: &str) {
    for node in nodes.as_array_mut().into_iter().flatten() {
        if node["status"] == "running" || node["status"] == "pending" {
            node["status"] = json!(status);
            node["endedAt"] = json!(chrono::Utc::now().timestamp_millis());
        }
        settle(&mut node["children"], status);
    }
}
