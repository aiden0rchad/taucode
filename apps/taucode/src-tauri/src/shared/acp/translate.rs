use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Translator {
    pub thread: String,
    pub cwd: String,
    pub turns: Vec<Value>,
    pub active: bool,
    tools: HashMap<String, Value>,
    completed: HashSet<String>,
    text_item: Option<(String, String)>,
    user_message: Option<String>,
}
impl Translator {
    pub fn new(thread: String, cwd: String, turns: Vec<Value>) -> Self {
        Self {
            thread,
            cwd,
            turns,
            ..Default::default()
        }
    }
    pub fn turn_id(&self) -> &str {
        self.turns
            .last()
            .and_then(|t| t["id"].as_str())
            .unwrap_or("")
    }
    pub fn event(&self, method: &str, mut params: Value) -> Value {
        params["threadId"] = json!(self.thread);
        json!({"method":method,"params":params})
    }
    pub fn start(&mut self, input: Value) -> Vec<Value> {
        self.active = true;
        self.tools.clear();
        self.completed.clear();
        self.text_item = None;
        self.user_message = None;
        let turn = json!({"id":format!("{}:turn:{}",self.thread,self.turns.len()+1),"items":[],"status":"inProgress","error":null});
        self.turns.push(turn.clone());
        let mut events = vec![
            self.event("turn/started", json!({"turn":turn})),
            self.event(
                "thread/status/changed",
                json!({"status":{"type":"active","activeFlags":[]}}),
            ),
        ];
        if !input.is_null() {
            self.item(
                json!({"id":uuid::Uuid::new_v4().to_string(),"type":"userMessage","content":input}),
                true,
                &mut events,
            );
        }
        events
    }
    fn item(&mut self, item: Value, done: bool, out: &mut Vec<Value>) {
        let Some(turn) = self.turns.last_mut() else {
            return;
        };
        let items = turn["items"].as_array_mut().unwrap();
        if let Some(old) = items.iter_mut().find(|v| v["id"] == item["id"]) {
            *old = item.clone();
        } else {
            items.push(item.clone());
            out.push(self.event("item/started",json!({"turnId":self.turn_id(),"item":item,"startedAtMs":chrono::Utc::now().timestamp_millis()})));
        }
        if done
            && self
                .completed
                .insert(item["id"].as_str().unwrap_or("").into())
        {
            out.push(self.event("item/completed",json!({"turnId":self.turn_id(),"item":item,"completedAtMs":chrono::Utc::now().timestamp_millis()})));
        }
    }
    pub fn finish(&mut self, reason: &str, error: Option<&str>) -> Vec<Value> {
        if !self.active {
            return vec![];
        }
        let mut out = vec![];
        let items = self.turns.last().unwrap()["items"]
            .as_array()
            .unwrap()
            .clone();
        for mut item in items {
            if !self.completed.contains(item["id"].as_str().unwrap_or("")) {
                if item.get("status").is_some() {
                    item["status"] = json!(if error.is_some() || reason == "cancelled" {
                        "failed"
                    } else {
                        "completed"
                    });
                }
                for node in item["agentsTree"].as_array_mut().into_iter().flatten() {
                    node["status"] = json!(if reason == "cancelled" { "aborted" } else if error.is_some() { "failed" } else { "completed" });
                }
                for state in item["agentsStates"].as_object_mut().into_iter().flat_map(|s| s.values_mut()) {
                    state["status"] = json!(if reason == "cancelled" { "aborted" } else if error.is_some() { "failed" } else { "completed" });
                }
                self.item(item, true, &mut out);
            }
        }
        let turn = self.turns.last_mut().unwrap();
        turn["status"] = json!(if error.is_some() || reason == "refusal" {
            "failed"
        } else if reason == "cancelled" {
            "interrupted"
        } else {
            "completed"
        });
        turn["stopReason"] = json!(reason);
        turn["error"] = error
            .map(|e| json!({"message":e,"codexErrorInfo":null,"additionalDetails":null}))
            .unwrap_or(Value::Null);
        let turn = turn.clone();
        self.active = false;
        out.push(self.event("turn/completed", json!({"turn":turn})));
        out.push(self.event("thread/status/changed", json!({"status":{"type":"idle"}})));
        out
    }
    pub fn fail_prompt(&mut self, mut error: super::client::RpcError) -> Vec<Value> {
        if let Some(items) = self.turns.last().and_then(|t| t["items"].as_array()) {
            for text in items.iter().filter(|i| i["type"] == "agentMessage")
                .filter_map(|i| i["text"].as_str()).filter(|s| s.starts_with("API Error:"))
            {
                if !error.message.contains(text) {
                    error.message.push('\n');
                    error.message.push_str(text);
                }
            }
        }
        let mut events = self.finish("error", Some(&error.message));
        if let Some(kind) = error.error_kind {
            if let Some(turn) = self.turns.last_mut() { turn["error"]["errorKind"] = kind.clone(); }
            for event in events.iter_mut().filter(|e| e["method"] == "turn/completed") {
                event["params"]["turn"]["error"]["errorKind"] = kind.clone();
            }
        }
        events
    }
    pub fn update(&mut self, update: &Value) -> Vec<Value> {
        let kind = update["sessionUpdate"].as_str().unwrap_or("");
        let mut out = vec![];
        if kind == "user_message_chunk" {
            let message = update["messageId"].as_str().map(str::to_string);
            let content = if update["content"]["type"] == "image" {
                json!({"type":"image","url":format!("data:{};base64,{}",update["content"]["mimeType"].as_str().unwrap_or("image/png"),update["content"]["data"].as_str().unwrap_or(""))})
            } else {
                update["content"].clone()
            };
            if self.active && message.is_some() && self.user_message == message {
                if let Some(item) = self.turns.last_mut().unwrap()["items"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|i| i["type"] == "userMessage")
                {
                    item["content"].as_array_mut().unwrap().push(content);
                }
                return out;
            }
            if self.active {
                out.extend(self.finish("end_turn", None));
            }
            out.extend(self.start(json!([content])));
            self.user_message = message;
            return out;
        }
        if !matches!(
            kind,
            "agent_message_chunk"
                | "agent_thought_chunk"
                | "tool_call"
                | "tool_call_update"
                | "plan"
                | "usage_update"
        ) {
            return out;
        }
        if !self.active {
            out.extend(self.start(Value::Null));
        }
        match kind {
            "agent_message_chunk" | "agent_thought_chunk" => {
                let reasoning = kind == "agent_thought_chunk";
                let ty = if reasoning {
                    "reasoning"
                } else {
                    "agentMessage"
                };
                let delta = update["content"]["text"].as_str().unwrap_or("");
                let id = match &self.text_item {
                    Some((old, id)) if old == ty => id.clone(),
                    _ => {
                        let id = uuid::Uuid::new_v4().to_string();
                        self.text_item = Some((ty.into(), id.clone()));
                        id
                    }
                };
                let old = self.turns.last().unwrap()["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|i| i["id"] == id);
                let text = format!(
                    "{}{delta}",
                    old.and_then(|v| if reasoning {
                        v["content"][0].as_str()
                    } else {
                        v["text"].as_str()
                    })
                    .unwrap_or("")
                );
                let item = if reasoning {
                    json!({"id":id,"type":ty,"summary":[],"content":[text]})
                } else {
                    json!({"id":id,"type":ty,"text":text,"phase":"final_answer"})
                };
                self.item(item, false, &mut out);
                // The delta carries this chunk; starting with it too would duplicate text in the UI.
                for event in out
                    .iter_mut()
                    .filter(|e| e["method"] == "item/started" && e["params"]["item"]["id"] == id)
                {
                    if reasoning {
                        event["params"]["item"]["content"] = json!([""]);
                    } else {
                        event["params"]["item"]["text"] = json!("");
                    }
                }
                out.push(self.event(
                    if reasoning {
                        "item/reasoning/textDelta"
                    } else {
                        "item/agentMessage/delta"
                    },
                    json!({"turnId":self.turn_id(),"itemId":id,"delta":delta,"contentIndex":0}),
                ));
            }
            "tool_call" | "tool_call_update" => {
                self.text_item = None;
                let id = update["toolCallId"].as_str().unwrap_or("");
                let tool = self.tools.entry(id.into()).or_insert_with(|| json!({}));
                let diffs: Vec<_> = tool["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|c| c["type"] == "diff")
                    .cloned()
                    .collect();
                for (key, value) in update.as_object().into_iter().flatten() {
                    tool[key] = value.clone();
                }
                if let Some(delta) = update
                    .pointer("/_meta/terminal_output_delta/data")
                    .and_then(Value::as_str)
                {
                    tool["_output"] =
                        json!(format!("{}{delta}", tool["_output"].as_str().unwrap_or("")));
                } else if let Some(output) = update
                    .pointer("/_meta/terminal_output/data")
                    .and_then(Value::as_str)
                {
                    tool["_output"] = json!(output);
                }
                if tool["kind"] == "edit"
                    && !diffs.is_empty()
                    && !tool["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|c| c["type"] == "diff")
                {
                    tool["content"].as_array_mut().unwrap().extend(diffs);
                }
                let tool = tool.clone();
                let mut item = tool_item(&tool, &self.thread, &self.cwd);
                if item["type"] == "collabAgentToolCall" {
                    let previous = self.turns.last().unwrap()["items"].as_array().unwrap().iter().find(|i| i["id"] == id);
                    let updates = previous.into_iter().flat_map(|i| i["agentUpdates"].as_array().into_iter().flatten()).cloned()
                        .chain(std::iter::once(json!({"role":"assistant","content":item["agentsTree"][0]["lastMessage"].as_str().filter(|s| !s.is_empty()).unwrap_or(tool["status"].as_str().unwrap_or("pending"))})));
                    item["agentUpdates"] = super::super::pi::agents::capped(updates)["messages"].clone();
                }
                let old_output = self.turns.last().unwrap()["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|i| i["id"] == id)
                    .and_then(|i| i["aggregatedOutput"].as_str())
                    .unwrap_or("")
                    .to_string();
                let done = matches!(tool["status"].as_str(), Some("completed" | "failed"));
                self.item(item.clone(), done, &mut out);
                if item["type"] == "commandExecution" && !done {
                    let output = item["aggregatedOutput"].as_str().unwrap_or("");
                    if let Some(delta) = output
                        .strip_prefix(&old_output)
                        .filter(|delta| !delta.is_empty())
                    {
                        out.push(self.event(
                            "item/commandExecution/outputDelta",
                            json!({"turnId":self.turn_id(),"itemId":id,"delta":delta}),
                        ));
                    }
                }
                if item["type"] == "fileChange" {
                    let diff = item["changes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v["diff"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push(self.event(
                        "turn/diff/updated",
                        json!({"turnId":self.turn_id(),"diff":diff}),
                    ));
                }
            }
            "plan" => {
                let plan: Vec<_> = update["entries"].as_array().into_iter().flatten().map(|e| json!({"step":e["content"],"status":match e["status"].as_str(){Some("in_progress")=>"inProgress",Some("completed")=>"completed",_=>"pending"}})).collect();
                self.item(json!({"id":format!("{}:plan",self.turn_id()),"type":"plan","text":update["entries"].as_array().into_iter().flatten().filter_map(|e|e["content"].as_str()).collect::<Vec<_>>().join("\n")}), false, &mut out);
                out.push(self.event(
                    "turn/plan/updated",
                    json!({"turnId":self.turn_id(),"explanation":null,"plan":plan}),
                ));
            }
            "usage_update" => {
                let usage = json!({"totalTokens":update["used"],"inputTokens":0,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0});
                out.push(self.event("thread/tokenUsage/updated",json!({"turnId":self.turn_id(),"tokenUsage":{"last":usage,"total":usage,"modelContextWindow":update["size"]}})));
            }
            _ => (),
        }
        out
    }
}
pub(super) fn tool_item(tool: &Value, thread: &str, cwd: &str) -> Value {
    let id = &tool["toolCallId"];
    let name = super::permissions::tool_name(tool);
    let input = &tool["rawInput"];
    let status = match tool["status"].as_str() {
        Some("completed") => "completed",
        Some("failed") => "failed",
        _ => "inProgress",
    };
    let text = tool["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["content"]["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if matches!(name.split_whitespace().next(), Some("Task" | "Agent"))
        || tool["kind"] == "task"
        || tool.pointer("/_meta/subagent").is_some()
    {
        let receiver = tool
            .pointer("/_meta/subagent/sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{thread}:agent:{}", id.as_str().unwrap_or("")));
        let agent_status = if status == "inProgress" { "running" } else { status };
        return json!({"id":id,"type":"collabAgentToolCall","tool":"spawnAgent","status":status,"senderThreadId":thread,"receiverThreadIds":[receiver],"prompt":input["prompt"],"model":input["model"],"reasoningEffort":null,"agentsStates":{&receiver:{"status":agent_status,"message":text}},"agentsTree":[{"id":receiver,"agent":input["subagent_type"].as_str().unwrap_or("Agent"),"task":input["description"].as_str().or(input["prompt"].as_str()),"model":input["model"],"status":agent_status,"lastMessage":text,"children":[]}]});
    }
    match tool["kind"].as_str() {
        Some("execute") => {
            json!({"id":id,"type":"commandExecution","command":input["command"].as_str().unwrap_or(name),"cwd":cwd,"status":status,"commandActions":[],"aggregatedOutput":tool["_output"].as_str().unwrap_or(if text.is_empty(){tool["rawOutput"].as_str().unwrap_or("")}else{&text}),"exitCode":tool.pointer("/_meta/terminal_exit/exit_code").unwrap_or(&tool["rawOutput"]["exitCode"]),"durationMs":null,"processId":null,"source":"agent"})
        }
        Some("edit") => {
            let changes: Vec<_> = tool["content"].as_array().into_iter().flatten().filter(|c|c["type"]=="diff").map(|c| {
                let old = c["oldText"].as_str().unwrap_or("");
                let new = c["newText"].as_str().unwrap_or("");
                let path = c["path"].as_str().unwrap_or("");
                let lines = |text: &str, prefix: char| {
                    let mut lines = text.lines().map(|l|format!("{prefix}{l}\n")).collect::<String>();
                    if !text.is_empty() && !text.ends_with('\n') { lines.push_str("\\ No newline at end of file\n"); }
                    lines
                };
                let diff = format!("--- {}\n+++ {}\n@@ -{},{} +{},{} @@\n{}{}",if c["oldText"].is_null(){"/dev/null"}else{path},path,usize::from(!old.is_empty()),old.lines().count(),usize::from(!new.is_empty()),new.lines().count(),lines(old,'-'),lines(new,'+'));
                json!({"path":path,"kind":{"type":if c["oldText"].is_null(){"add"}else{"update"}},"diff":diff})
            }).collect();
            json!({"id":id,"type":"fileChange","status":status,"changes":changes})
        }
        _ if name.starts_with("mcp__") => {
            let (server, tool_name) = name
                .trim_start_matches("mcp__")
                .split_once("__")
                .unwrap_or(("", name));
            json!({"id":id,"type":"mcpToolCall","server":server,"tool":tool_name,"arguments":input,"status":status,"result":if status=="completed"{json!({"content":[{"type":"text","text":text}]})}else{Value::Null},"error":if status=="failed"{json!({"message":text})}else{Value::Null},"durationMs":null})
        }
        _ => {
            json!({"id":id,"type":"dynamicToolCall","tool":name,"namespace":null,"arguments":input,"status":status,"contentItems":[{"type":"inputText","text":text}],"success":if status=="inProgress"{Value::Null}else{json!(status=="completed")},"durationMs":null})
        }
    }
}
