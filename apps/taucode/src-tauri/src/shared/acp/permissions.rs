use crate::types::{McpApproval, McpServerEntry};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Decision {
    Allow,
    Ask,
    Reject,
}

pub(super) fn tool_name(tool: &Value) -> &str {
    tool.get("name")
        .and_then(Value::as_str)
        .or_else(|| tool.pointer("/rawInput/tool_name").and_then(Value::as_str))
        .or_else(|| {
            tool.pointer("/_meta/claudeCode/toolName")
                .and_then(Value::as_str)
        })
        .or_else(|| tool["title"].as_str())
        .unwrap_or("")
}
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn connector_decision(entries: &[McpServerEntry], tool: &str, mode: &str) -> Decision {
    let Some(server) = entries
        .iter()
        .filter(|s| tool.starts_with(&format!("mcp__{}__", s.id.replace('-', "_"))))
        .max_by_key(|s| s.id.len())
    else {
        return Decision::Reject;
    };
    if !server.enabled {
        return Decision::Reject;
    }
    let name = &tool[format!("mcp__{}__", server.id.replace('-', "_")).len()..];
    if server
        .tool_denylist
        .iter()
        .flatten()
        .any(|t| name == normalize(t) || name.starts_with(&format!("{}_", normalize(t))))
        || server
            .tool_allowlist
            .as_ref()
            .is_some_and(|list| !list.iter().any(|t| normalize(t) == name))
    {
        return Decision::Reject;
    }
    if server.id == "taucode-browser" && crate::shared::browser::READ_TOOLS.contains(&name)
        || server.id == "taucode-computer" && name == "computer_screenshot"
    {
        return Decision::Allow;
    }
    if mode == "read-only" {
        return Decision::Reject;
    }
    // Computer actions have their own approval gate in the connector, as in pi.
    if server.id == "taucode-computer" {
        return Decision::Allow;
    }
    if server.approval == McpApproval::Ask {
        Decision::Ask
    } else {
        Decision::Allow
    }
}
fn within(cwd: &str, path: &str) -> bool {
    let root = Path::new(cwd);
    let candidate = root.join(path);
    // Resolve symlinks, including the existing parent of a newly created file.
    let mut existing = candidate.as_path();
    while !existing.exists() {
        if existing
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return false;
        }
        let Some(parent) = existing.parent() else {
            return false;
        };
        existing = parent;
    }
    root.canonicalize()
        .ok()
        .zip(existing.canonicalize().ok())
        .is_some_and(|(root, path)| path.starts_with(root))
}
pub(super) fn decide(mode: &str, cwd: &str, entries: &[McpServerEntry], tool: &Value) -> Decision {
    let name = tool_name(tool);
    if name.starts_with("mcp__") {
        let decision = connector_decision(entries, name, mode);
        return if mode == "ask" && decision != Decision::Reject {
            Decision::Ask
        } else {
            decision
        };
    }
    let read = matches!(
        tool["kind"].as_str(),
        Some("read" | "search" | "fetch" | "think")
    );
    if mode == "read-only" {
        return if read {
            Decision::Allow
        } else {
            Decision::Reject
        };
    }
    if mode != "full-access" {
        return Decision::Ask;
    }
    if read {
        return Decision::Allow;
    }
    if matches!(tool["kind"].as_str(), Some("edit" | "execute")) {
        let input = &tool["rawInput"];
        let paths = [
            input["file_path"].as_str(),
            input["path"].as_str(),
            input["cwd"].as_str(),
        ];
        if paths.into_iter().flatten().any(|p| !within(cwd, p)) {
            return Decision::Ask;
        }
        if tool["locations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["path"].as_str())
            .any(|p| !within(cwd, p))
        {
            return Decision::Ask;
        }
        if tool["kind"] == "execute"
            || paths.into_iter().flatten().next().is_some()
            || tool["locations"].as_array().is_some_and(|v| !v.is_empty())
        {
            return Decision::Allow;
        }
    }
    Decision::Ask
}
pub(super) fn outcome(request: &Value, decision: &str) -> Value {
    let kind = match decision {
        "accept" => "allow_once",
        // Keep session grants in taucode: adapter allow_always may persist localSettings.
        "acceptForSession" => "allow_once",
        _ => "reject_once",
    };
    let options = request["options"].as_array();
    let option = options.into_iter().flatten().find(|o| o["kind"] == kind);
    option
        .map(|o| json!({"outcome":{"outcome":"selected","optionId":o["optionId"]}}))
        .unwrap_or_else(|| json!({"outcome":{"outcome":"cancelled"}}))
}
pub(super) fn approval(id: &str, thread: &str, turn: &str, cwd: &str, request: &Value) -> Value {
    let tool = &request["toolCall"];
    let mut decisions = vec!["accept"];
    if !tool_name(tool).starts_with("mcp__")
        && request["options"]
            .as_array()
            .is_some_and(|options| options.iter().any(|o| o["kind"] == "allow_always"))
    {
        decisions.push("acceptForSession");
    }
    decisions.extend(["decline", "cancel"]);
    json!({"id":id,"method":if tool["kind"] == "edit" {"item/fileChange/requestApproval"} else {"item/commandExecution/requestApproval"},"params":{"threadId":thread,"turnId":turn,"itemId":tool["toolCallId"],"approvalId":id,"command":tool["rawInput"]["command"].as_str().unwrap_or(tool_name(tool)),"cwd":cwd,"reason":format!("Claude Code requests permission: {}",tool_name(tool)),"availableDecisions":decisions,"grantRoot":null}})
}

pub(super) fn grant_key(tool: &Value) -> String {
    format!("{}:{}", tool_name(tool), tool["rawInput"])
}
