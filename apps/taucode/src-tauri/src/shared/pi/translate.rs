//! Pi has many model turns per prompt. Only agent_start/agent_settled delimit app turns.
//! Live messages and persisted messages use the same item constructors and finalizers.
use serde_json::{json, Value};
use std::collections::HashMap;

pub(super) fn text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.into();
    }
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn error(message: &str) -> Value {
    json!({"message":message,"codexErrorInfo":null,"additionalDetails":null,"misalignment":null})
}
fn input(content: &Value) -> Value {
    if content.is_string() {
        return json!([{"type":"text","text":text(content),"text_elements":[]}]);
    }
    json!(content.as_array().into_iter().flatten().filter_map(|p|match p["type"].as_str() {
        Some("text") => Some(json!({"type":"text","text":p["text"],"text_elements":[]})),
        Some("image") => Some(json!({"type":"image","url":format!("data:{};base64,{}",p["mimeType"].as_str().unwrap_or("image/png"),p["data"].as_str().unwrap_or(""))})),
        _ => None,
    }).collect::<Vec<_>>())
}

pub(super) struct Translator {
    pub thread: String,
    pub cwd: String,
    pub turns: Vec<Value>,
    pub active: bool,
    pub model: Value,
    pub provider: String,
    pub diagnostic_engine: String,
    pub diagnostic_codegraph: bool,
    pub diagnostic_zero_cost: bool,
    pub diagnostic_provider_kind: String,
    pub connectors: HashMap<String, String>,
    pub access_mode: String,
    pub total: Value,
    pub cost: f64,
    unknown_costs: std::collections::HashSet<String>,
    pub queue: Value,
    pub transcripts: HashMap<String, Value>,
    message: String,
    message_count: usize,
    completed: std::collections::HashSet<String>,
    tools: HashMap<String, Value>,
    files: HashMap<String, String>,
    failure: Option<String>,
    aborted: bool,
    compaction_only: bool,
    diff: String,
}
impl Translator {
    pub fn new(thread: &str, cwd: &str) -> Self {
        Self {
            thread: thread.into(),
            cwd: cwd.into(),
            turns: vec![],
            active: false,
            model: Value::Null,
            provider: String::new(),
            diagnostic_engine: "pi".into(),
            diagnostic_codegraph: false,
            diagnostic_zero_cost: false,
            diagnostic_provider_kind: String::new(),
            connectors: HashMap::new(),
            access_mode: "ask".into(),
            total: json!({"totalTokens":0,"inputTokens":0,"cachedInputTokens":0,"cacheWriteInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0}),
            cost: 0.0,
            unknown_costs: Default::default(),
            queue: json!({}),
            transcripts: HashMap::new(),
            message: String::new(),
            message_count: 0,
            completed: Default::default(),
            tools: HashMap::new(),
            files: HashMap::new(),
            failure: None,
            aborted: false,
            compaction_only: false,
            diff: String::new(),
        }
    }
    pub fn turn_id(&self) -> String {
        self.turns
            .last()
            .and_then(|t| t["id"].as_str())
            .unwrap_or("")
            .into()
    }
    fn event(&self, method: &str, mut params: Value) -> Value {
        params["threadId"] = json!(self.thread);
        if taucode_diag_mcp::writer::enabled() && method=="turn/started" {params["engine"]=json!(self.diagnostic_engine);params["model"]=self.model["id"].clone();params["providerKind"]=json!(self.diagnostic_provider_kind);params["codegraph"]=json!(self.diagnostic_codegraph);params["zeroCost"]=json!(self.diagnostic_zero_cost);}
        json!({"method":method,"params":params})
    }
    fn item_event(&self, method: &str, item: &Value) -> Value {
        let mut params = json!({"turnId":self.turn_id(),"item":item});
        params[if method == "item/started" {
            "startedAtMs"
        } else {
            "completedAtMs"
        }] = json!(chrono::Utc::now().timestamp_millis());
        self.event(method, params)
    }
    fn item(&mut self, item: Value, done: bool, out: &mut Vec<Value>) {
        let id = item["id"].as_str().unwrap().to_owned();
        let Some(turn) = self.turns.last_mut() else {
            return;
        };
        let items = turn["items"].as_array_mut().unwrap();
        if let Some(old) = items.iter_mut().find(|i| i["id"] == id) {
            *old = item.clone();
        } else {
            items.push(item.clone());
            out.push(self.item_event("item/started", &item));
        }
        if done && self.completed.insert(id) {
            out.push(self.item_event("item/completed", &item));
        }
    }
    fn find(&self, id: &str) -> Option<Value> {
        self.turns.last()?["items"]
            .as_array()?
            .iter()
            .find(|i| i["id"] == id)
            .cloned()
    }
    fn start(&mut self, out: &mut Vec<Value>) {
        if self.active {
            return;
        }
        self.active = true;
        self.failure = None;
        self.aborted = false;
        self.diff.clear();
        let turn = json!({"id":format!("{}:turn:{}",self.thread,self.turns.len()+1),"rootTurnId":null,"items":[],"itemsView":"full","status":"inProgress","error":null,"startedAt":null,"completedAt":null,"durationMs":null});
        self.turns.push(turn.clone());
        out.push(self.event("turn/started", json!({"turn":turn})));
        out.push(self.event(
            "thread/status/changed",
            json!({"status":{"type":"active","activeFlags":[]}}),
        ));
    }
    fn settle(&mut self, aborted: bool, out: &mut Vec<Value>) {
        if !self.active {
            return;
        }
        let open = self.turns.last().unwrap()["items"]
            .as_array()
            .unwrap()
            .clone();
        for mut item in open {
            if !self.completed.contains(item["id"].as_str().unwrap()) {
                if item.get("agentsTree").is_some() {
                    super::agents::settle(&mut item["agentsTree"], if aborted { "aborted" } else if self.failure.is_some() { "failed" } else { "completed" }, Some(chrono::Utc::now().timestamp_millis()));
                }
                if item.get("status").is_some() {
                    item["status"] = json!(if aborted || self.failure.is_some() {
                        "failed"
                    } else {
                        "completed"
                    });
                }
                self.item(item, true, out);
            }
        }
        let turn = self.turns.last_mut().unwrap();
        turn["status"] = json!(if aborted {
            "interrupted"
        } else if self.failure.is_some() {
            "failed"
        } else {
            "completed"
        });
        turn["error"] = self.failure.as_deref().map(error).unwrap_or(Value::Null);
        let turn = turn.clone();
        self.active = false;
        out.push(self.event("turn/completed", json!({"turn":turn})));
        out.push(self.event("thread/status/changed", json!({"status":{"type":"idle"}})));
    }
    pub fn fail(&mut self, message: &str) -> Vec<Value> {
        self.failure = Some(message.into());
        let mut out = vec![self.event(
            "error",
            json!({"turnId":self.turn_id(),"error":error(message),"willRetry":false}),
        )];
        self.settle(false, &mut out);
        out
    }
    fn stream_item(&self, index: usize, thinking: bool) -> Value {
        let id = format!("{}:{index}", self.message);
        self.find(&id).unwrap_or_else(||if thinking {json!({"type":"reasoning","id":id,"summary":[],"content":[""]})}else{json!({"type":"agentMessage","id":id,"text":"","phase":null,"memoryCitation":null,"delivery":null,"questions":null})})
    }
    fn usage(&mut self, u: &Value, source: &str, out: &mut Vec<Value>) {
        let mut last = json!({});
        for (pi, canonical) in [
            ("totalTokens", "totalTokens"),
            ("input", "inputTokens"),
            ("cacheRead", "cachedInputTokens"),
            ("cacheWrite", "cacheWriteInputTokens"),
            ("output", "outputTokens"),
            ("reasoning", "reasoningOutputTokens"),
        ] {
            let n = u[pi].as_u64().unwrap_or(0);
            last[canonical] = json!(n);
            self.total[canonical] = json!(self.total[canonical].as_u64().unwrap_or(0) + n);
        }
        // Raw runtime catalogs default unknown prices to zero. Numeric costs here
        // are normalized delegation deltas; their explicit zero is trustworthy.
        let cost = u["cost"].as_f64().filter(|n| *n >= 0.0)
            .or_else(|| u["cost"]["total"].as_f64().filter(|n| *n > 0.0));
        self.cost += u["knownCost"].as_f64().filter(|n| *n >= 0.0).or(cost).unwrap_or(0.0);
        if cost.is_some() { self.unknown_costs.remove(source); } else { self.unknown_costs.insert(source.into()); }
        self.total["cost"] = if self.unknown_costs.is_empty() { json!(self.cost) } else { Value::Null };
        self.total["knownCost"] = json!(self.cost);
        last["cost"] = json!(cost);
        out.push(self.event("thread/tokenUsage/updated",json!({"turnId":self.turn_id(),"tokenUsage":{"last":last,"total":self.total,"modelContextWindow":self.model["contextWindow"]}})));
    }
    fn message_end(&mut self, m: &Value, out: &mut Vec<Value>) {
        match m["role"].as_str() {
            Some("user") => {
                let id = format!("{}:user:{}", self.thread, m["timestamp"]);
                self.item(json!({"type":"userMessage","id":id,"clientId":null,"content":input(&m["content"])}),true,out);
            }
            Some("assistant") => {
                for (index, p) in m["content"].as_array().into_iter().flatten().enumerate() {
                    match p["type"].as_str() {
                        Some("text" | "thinking") => {
                            let thinking = p["type"] == "thinking";
                            let mut item = self.stream_item(index, thinking);
                            if thinking {
                                item["content"] = json!([p["thinking"].as_str().unwrap_or("")]);
                            } else {
                                item["text"] = p["text"].clone();
                            }
                            self.item(item, true, out);
                            if !thinking {
                                let plan = markdown_plan(p["text"].as_str().unwrap_or(""));
                                if !plan.is_empty() {
                                    self.item(json!({"type":"plan","id":format!("{}:plan:{}",self.turn_id(),self.message),"text":p["text"],"plan":plan}), true, out);
                                }
                            }
                        }
                        Some("toolCall") => {
                            self.tools
                                .insert(p["id"].as_str().unwrap_or("").into(), p.clone());
                        }
                        _ => (),
                    }
                }
                if m["stopReason"] == "error" {
                    self.failure = Some(
                        m["errorMessage"]
                            .as_str()
                            .unwrap_or("Pi provider failed")
                            .into(),
                    );
                }
                if m["stopReason"] == "aborted" {
                    self.aborted = true;
                }
                self.usage(&m["usage"], &self.message.clone(), out);
            }
            _ => (),
        }
    }
    fn tool_start(&mut self, m: &Value, out: &mut Vec<Value>) {
        let id = m["toolCallId"].as_str().unwrap_or("");
        let name = m["toolName"].as_str().unwrap_or("");
        let a = &m["args"];
        let virtual_name = matches!(name, "write" | "edit")
            .then(|| a["path"].as_str().and_then(|path| path.strip_prefix("xd://")))
            .flatten().filter(|name| !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte)));
        let name = virtual_name.unwrap_or(name);
        let virtual_args = virtual_name.and_then(|_| a["content"].as_str())
            .and_then(|content| serde_json::from_str::<Value>(content).ok());
        let a = virtual_args.as_ref().unwrap_or(a);
        let requested_command = (name == "bash" && a["command"].as_str().is_some_and(|command| command.contains("/lib/commandGuard.ts' ")))
            .then(|| self.tools.get(id).and_then(|tool| tool["arguments"]["command"].as_str()).map(str::to_owned)).flatten();
        self.tools
            .insert(id.into(), json!({"name":name,"arguments":a}));
        let item = match name {
            "subagent" => {
                let tasks = a["tasks"]
                    .as_array()
                    .filter(|tasks| !tasks.is_empty())
                    .or_else(|| a["chain"].as_array().filter(|tasks| !tasks.is_empty()))
                    .cloned()
                    .unwrap_or_else(|| vec![a.clone()]);
                let states: serde_json::Map<String, Value> = tasks.iter().enumerate().map(|(i, task)| (format!("{id}:agent:{i}"), json!({"status":"running","message":task["task"],"task":task["task"],"agent":task["agent"],"model":task["model"],"usage":null}))).collect();
                json!({"type":"collabAgentToolCall","id":id,"tool":"spawnAgent","status":"inProgress","senderThreadId":self.thread,"receiverThreadIds":states.keys().collect::<Vec<_>>(),"prompt":tasks.iter().filter_map(|t|t["task"].as_str()).collect::<Vec<_>>().join("\n\n"),"model":a["model"],"reasoningEffort":null,"agentsTree":super::agents::tree(id, &tasks, &Value::Null, false, (m["replay"] != true).then(|| chrono::Utc::now().timestamp_millis()), &mut self.transcripts),"agentsStates":states})
            }
            "bash" => {
                json!({"type":"commandExecution","id":id,"pluginId":null,"scriptPath":null,"command":requested_command.as_deref().or_else(|| a["command"].as_str()).unwrap_or(""),"cwd":self.cwd,"processId":null,"source":"agent","status":"inProgress","commandActions":[],"aggregatedOutput":"","exitCode":null,"durationMs":null})
            }
            "write" | "edit" => {
                let path = a["path"].as_str().unwrap_or("");
                let mut diff = format!("--- a/{path}\n+++ b/{path}\n");
                if name == "write" {
                    let content = a["content"].as_str().unwrap_or("");
                    let previous = self.files.get(path).map(String::as_str).unwrap_or("");
                    diff.push_str(&format!(
                        "@@ -{},{} +1,{} @@\n",
                        if previous.is_empty() { 0 } else { 1 },
                        previous.lines().count(),
                        content.lines().count()
                    ));
                    for line in previous.lines() {
                        diff.push_str(&format!("-{line}\n"));
                    }
                    for line in content.lines() {
                        diff.push_str(&format!("+{line}\n"));
                    }
                } else {
                    let edits = a["edits"]
                        .as_array()
                        .cloned()
                        .unwrap_or_else(|| vec![a.clone()]);
                    for edit in edits {
                        let old = edit["oldText"].as_str().unwrap_or("");
                        let new = edit["newText"].as_str().unwrap_or("");
                        diff.push_str(&format!(
                            "@@ -1,{} +1,{} @@\n",
                            old.lines().count(),
                            new.lines().count()
                        ));
                        for (prefix, content) in [("-", old), ("+", new)] {
                            for line in content.lines() {
                                diff.push_str(&format!("{prefix}{line}\n"));
                            }
                        }
                    }
                }
                json!({"type":"fileChange","id":id,"changes":[{"path":path,"kind":if name=="write" && !self.files.contains_key(path){json!({"type":"add"})}else{json!({"type":"update","move_path":null})},"diff":diff}],"status":"inProgress"})
            }
            _ if name.starts_with("mcp__") => {
                let (server, tool) = self
                    .connectors
                    .keys()
                    .filter(|id| name.starts_with(&format!("mcp__{id}__")) || name.starts_with(&format!("mcp__{id}_")))
                    .max_by_key(|id| id.len())
                    .map(|id| (id.as_str(), name[6 + id.len()..].strip_prefix('_').unwrap_or(&name[6 + id.len()..])))
                    .unwrap_or_else(|| connector_parts(&name[5..]).unwrap_or(("", &name[5..])));
                let server = self
                    .connectors
                    .get(server)
                    .map(String::as_str)
                    .unwrap_or(server);
                json!({"type":"mcpToolCall","id":id,"server":server,"tool":tool,"arguments":a,"status":"inProgress","appContext":null,"mcpAppUi":null,"pluginId":null,"readOnlyHint":null,"result":null,"error":null,"durationMs":null})
            }
            _ => {
                json!({"type":"dynamicToolCall","id":id,"namespace":null,"tool":name,"arguments":a,"status":"inProgress","contentItems":null,"success":null,"durationMs":null})
            }
        };
        self.item(item, false, out);
    }
    pub fn approval_item(&mut self, a: &Value) -> Vec<Value> {
        let mut out = vec![];
        let id = a["toolCallId"].as_str().unwrap_or("");
        if self.find(id).is_none() {
            let args = self
                .tools
                .get(id)
                .map(|t| t["arguments"].clone())
                .unwrap_or_else(|| json!({"command":a["command"],"path":a["path"]}));
            self.tool_start(
                &json!({"toolCallId":id,"toolName":a["tool"],"args":args}),
                &mut out,
            );
        }
        out
    }
    fn tool_end(&mut self, m: &Value, done: bool, out: &mut Vec<Value>) {
        let id = m["toolCallId"].as_str().unwrap_or("");
        // Aborted runtimes can flush progress after the terminal tool/turn frame.
        // Keep final results (including usage), but never reopen a finished tool.
        if !done && self.completed.contains(id) {
            return;
        }
        let Some(mut item) = self.find(id) else {
            return;
        };
        let result = &m[if done { "result" } else { "partialResult" }];
        let failed = m["isError"] == true || result["isError"] == true;
        if done {
            item["status"] = json!(if failed { "failed" } else { "completed" });
        }
        match item["type"].as_str().unwrap_or("") {
            "collabAgentToolCall" => {
                let previous_tree = item["agentsTree"].clone();
                if let Some(results) = super::agents::results(&result["details"]) {
                    item["agentsTree"] = super::agents::tree(id, results, &item["agentsTree"], done, (m["replay"] != true).then(|| chrono::Utc::now().timestamp_millis()), &mut self.transcripts);
                }
                for (i, r) in super::agents::results(&result["details"])
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let agent_id = format!("{id}:agent:{i}");
                    let bad = r["exitCode"].as_i64().is_some_and(|n| n > 0)
                        || matches!(r["stopReason"].as_str(), Some("error" | "aborted"))
                        || r["status"] == "failed";
                    let terminal = done
                        || r["status"] == "completed"
                        || (r["status"].is_null() && r["stopReason"] == "stop");
                    let status = if bad {
                        "errored"
                    } else if terminal {
                        "completed"
                    } else {
                        "running"
                    };
                    let message = r["messages"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .rev()
                        .find(|m| m["role"] == "assistant")
                        .map(|m| text(&m["content"]))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| r["stderr"].as_str().unwrap_or("").into());
                    // Snapshots contain cumulative usage. Only add the increment, including
                    // updates followed by final tool results and duplicate completion frames.
                    let old = &previous_tree[i]["usage"];
                    let usage = &item["agentsTree"][i]["usage"];
                    let mut delta = json!({});
                    for key in ["input", "output", "cacheRead", "cacheWrite"] {
                        delta[key] = json!(usage[key]
                            .as_u64()
                            .unwrap_or(0)
                            .saturating_sub(old[key].as_u64().unwrap_or(0)));
                    }
                    delta["totalTokens"] = json!(["input", "output", "cacheRead", "cacheWrite"]
                        .iter()
                        .map(|k| delta[k].as_u64().unwrap_or(0))
                        .sum::<u64>());
                    delta["cost"] = json!(usage["cost"].as_f64().map(|cost| (cost - old["cost"].as_f64().unwrap_or(0.0)).max(0.0)));
                    delta["knownCost"] = json!((usage["knownCost"].as_f64().unwrap_or(0.0) - old["knownCost"].as_f64().unwrap_or(0.0)).max(0.0));
                    if delta["totalTokens"] != 0 || delta["knownCost"].as_f64().unwrap_or(0.0) > 0.0 || usage["cost"].is_number() != old["cost"].is_number() {
                        self.usage(&delta, &agent_id, out);
                    }
                    item["agentsStates"][&agent_id] = json!({"status":status,"message":message.chars().take(300).collect::<String>(),"agent":r["agent"],"task":r["task"],"model":r["model"],"usage":r["usage"]});
                }
                if done
                    && item["agentsStates"]
                        .as_object()
                        .is_some_and(|s| s.values().any(|v| v["status"] == "errored"))
                {
                    item["status"] = json!("failed");
                }
                if done && failed {
                    item["error"] = json!({"message":text(&result["content"])});
                    super::agents::settle(&mut item["agentsTree"], if self.aborted { "aborted" } else { "failed" }, Some(chrono::Utc::now().timestamp_millis()));
                    for state in item["agentsStates"].as_object_mut().into_iter().flat_map(|states| states.values_mut()) {
                        if state["status"] == "running" { state["status"] = json!("errored"); }
                    }
                }
                if !done {
                    out.push(self.item_event("item/started", &item));
                }
            }
            "commandExecution" => {
                // Pi persists content, but omits structuredContent from toolResult.
                let output = text(&result["content"]);
                let old = item["aggregatedOutput"].as_str().unwrap_or("");
                // Pi partialResult is a snapshot, never append it wholesale.
                let delta = output.strip_prefix(old).unwrap_or(&output);
                if !delta.is_empty() {
                    out.push(self.event(
                        "item/commandExecution/outputDelta",
                        json!({"turnId":self.turn_id(),"itemId":id,"delta":delta}),
                    ));
                }
                item["aggregatedOutput"] = json!(output);
                if done {
                    item["exitCode"] = result["structuredContent"]["exit_code"]
                        .as_i64()
                        .or_else(|| result["details"]["exitCode"].as_i64())
                        .or_else(|| {
                            output
                                .lines()
                                .last()?
                                .strip_prefix("Command exited with code ")?
                                .parse::<i64>()
                                .ok()
                        })
                        .map(|v| json!(v))
                        .unwrap_or(if failed && self.aborted { Value::Null } else { json!(if failed { 1 } else { 0 }) });
                    item["durationMs"] = m["durationMs"].clone();
                }
            }
            "fileChange" if done && !failed => {
                if let Some(tool) = self.tools.get(id) {
                    let args = &tool["arguments"];
                    if let Some(path) = args["path"].as_str() {
                        if tool["name"] == "write" {
                            self.files
                                .insert(path.into(), args["content"].as_str().unwrap_or("").into());
                        } else if let Some(content) = self.files.get_mut(path) {
                            for edit in args["edits"]
                                .as_array()
                                .cloned()
                                .unwrap_or_else(|| vec![args.clone()])
                            {
                                if let (Some(old), Some(new)) =
                                    (edit["oldText"].as_str(), edit["newText"].as_str())
                                {
                                    *content = content.replacen(old, new, 1);
                                }
                            }
                        }
                    }
                }
                for c in item["changes"].as_array().unwrap() {
                    self.diff.push_str(c["diff"].as_str().unwrap_or(""));
                }
                out.push(self.event(
                    "turn/diff/updated",
                    json!({"turnId":self.turn_id(),"diff":self.diff}),
                ));
            }
            "mcpToolCall" if done => {
                item["result"] = if failed {
                    Value::Null
                } else {
                    json!({"content":result["content"],"structuredContent":result["structuredContent"]})
                };
                item["error"] = if failed {
                    json!({"message":text(&result["content"])})
                } else {
                    Value::Null
                };
                item["durationMs"] = m["durationMs"].clone();
            }
            "dynamicToolCall" if done => {
                item["contentItems"] = json!(result["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| match p["type"].as_str() {
                        Some("text") => p["text"].as_str().map(|s|json!({"type":"inputText","text":s})),
                        Some("image") => Some(json!({"type":"inputImage","imageUrl":format!("data:{};base64,{}",p["mimeType"].as_str().unwrap_or("image/png"),p["data"].as_str().unwrap_or(""))})),
                        _ => None,
                    })
                    .collect::<Vec<_>>());
                item["success"] = json!(!failed);
                item["durationMs"] = m["durationMs"].clone();
            }
            _ => (),
        }
        self.item(item, done, out);
    }
    pub fn apply(&mut self, m: &Value) -> Vec<Value> {
        let mut out = vec![];
        if m["type"] == "response" && m["success"] == true {
            match m["command"].as_str() {
                Some("get_state") => self.model = m["data"]["model"].clone(),
                Some("set_model") => {
                    self.model = m["data"].clone();
                    out.push(settings_event(
                        &self.thread,
                        &self.cwd,
                        if self.provider.is_empty() {
                            self.model["provider"].as_str().unwrap_or("")
                        } else {
                            &self.provider
                        },
                        self.model["id"].as_str().unwrap_or(""),
                        &self.access_mode,
                    ));
                }
                _ => (),
            }
            return out;
        }
        match m["type"].as_str().unwrap_or("") {
            "abort_requested"=>self.aborted=true,
            "agent_start"=>self.start(&mut out),
            "agent_settled"=>{if let Some(error)=m["errorMessage"].as_str().or(m["error"].as_str()){self.failure=Some(error.into());}self.settle(m["aborted"]==true || self.aborted,&mut out);},
            "message_start"=>{
                if m["message"]["role"]=="assistant" {self.message_count+=1;self.message=format!("{}:message:{}",self.thread,m["message"]["timestamp"].as_u64().unwrap_or(self.message_count as u64));}
                if m["message"]["role"]=="user" {let msg=&m["message"];let id=format!("{}:user:{}",self.thread,msg["timestamp"]);self.item(json!({"type":"userMessage","id":id,"clientId":null,"content":input(&msg["content"])}),false,&mut out);}
            },
            "message_end"=>self.message_end(&m["message"],&mut out),
            "message_update"=>{
                let e=&m["assistantMessageEvent"];let kind=e["type"].as_str().unwrap_or("");
                if let Some((_,thinking))=[("text_",false),("thinking_",true)].iter().find(|(prefix,_)|kind.starts_with(prefix)) {
                    let mut item=self.stream_item(e["contentIndex"].as_u64().unwrap_or(0) as usize,*thinking);
                    self.item(item.clone(),false,&mut out);
                    let old=if *thinking {item["content"][0].as_str().unwrap_or("")}else{item["text"].as_str().unwrap_or("")};
                    let value=if kind.ends_with("delta"){format!("{old}{}",e["delta"].as_str().unwrap_or(""))}else if kind.ends_with("end"){e["content"].as_str().or(e["thinking"].as_str()).unwrap_or(old).into()}else{old.into()};
                    if *thinking {item["content"]=json!([value]);}else{item["text"]=json!(value);}
                    self.item(item.clone(),kind.ends_with("end"),&mut out);
                    if kind.ends_with("delta"){let mut params=json!({"turnId":self.turn_id(),"itemId":item["id"],"delta":e["delta"]});if *thinking{params["contentIndex"]=json!(0);}out.push(self.event(if *thinking{"item/reasoning/textDelta"}else{"item/agentMessage/delta"},params));}
                }
            },
            "tool_execution_start"=>self.tool_start(m,&mut out),
            "tool_execution_update"=>self.tool_end(m,false,&mut out),
            "tool_execution_end"=>self.tool_end(m,true,&mut out),
            "queue_update"=>{self.queue=m.clone();out.push(self.event("thread/queue/changed",json!({})));},
            "compaction_start"=>{
                self.compaction_only=!self.active;self.start(&mut out);
                let id=format!("{}:compaction",self.turn_id());self.item(json!({"type":"contextCompaction","id":id}),false,&mut out);
            },
            "compaction_end"=>{
                let too_small=matches!(m["errorMessage"].as_str(), Some("Nothing to compact (session too small)" | "Compaction failed: Nothing to compact (session too small)"));
                let failed=!too_small && (m["errorMessage"].is_string() || m["aborted"]==true || m["result"].is_null());
                let id=format!("{}:compaction",self.turn_id());
                // The canonical compaction variant has no status field; retain failure as
                // an extension field and emit the canonical error as well.
                let mut item=json!({"type":"contextCompaction","id":id});
                if too_small {item["notice"]=json!("Nothing to compact yet");}
                if failed {item["status"]=json!("failed");let msg=m["errorMessage"].as_str().unwrap_or("Pi compaction cancelled");out.push(self.event("error",json!({"turnId":self.turn_id(),"error":error(msg),"willRetry":false})));if self.compaction_only{self.failure=Some(msg.into());}}
                self.item(item,true,&mut out);
                if m["willRetry"]==true && !failed{self.failure=None;}
                if self.compaction_only {self.settle(m["aborted"]==true,&mut out);self.compaction_only=false;}
            },
            "auto_retry_start"=>out.push(self.event("error",json!({"turnId":self.turn_id(),"error":error(m["errorMessage"].as_str().unwrap_or("Pi is retrying the provider request")),"willRetry":true}))),
            "auto_retry_end"=>{if m["success"]==true {self.failure=None;out.push(self.event("warning",json!({"message":"Pi provider retry succeeded"})));}else{let msg=m["finalError"].as_str().unwrap_or("Pi provider retry failed");self.failure=Some(msg.into());out.push(self.event("error",json!({"turnId":self.turn_id(),"error":error(msg),"willRetry":false})));}},
            "extension_error"=>out.push(self.event("error",json!({"turnId":self.turn_id(),"error":error(m["error"].as_str().unwrap_or("Pi extension failed")),"willRetry":false}))),
            _=>(),
        }
        out
    }
    pub fn with_connectors(mut self, names: HashMap<String, String>) -> Self {
        for turn in &mut self.turns {
            for item in turn["items"].as_array_mut().into_iter().flatten() {
                if item["type"] == "mcpToolCall" {
                    if let Some(name) = item["server"].as_str().and_then(|id| names.get(id)) {
                        item["server"] = json!(name);
                    }
                }
            }
        }
        self.connectors = names;
        self
    }
    pub fn history(thread: &str, cwd: &str, messages: &[Value]) -> Self {
        let mut t = Self::new(thread, cwd);
        let mut end = 0u64;
        let mut last_stop = String::new();
        for m in messages {
            if m["role"] == "taucodeInterrupted" {
                t.aborted = true;
                t.failure = Some("Execution interrupted before pending tools completed.".into());
                continue;
            }
            if m["role"] == "taucodeToolProgress" {
                t.apply(&json!({"type":"tool_execution_update","replay":true,"toolCallId":m["toolCallId"],"toolName":m["toolName"],"partialResult":m["result"]}));
                t.failure = Some("Execution interrupted. Recovered the last saved tool output; completion was not observed.".into());
                t.aborted = true;
                continue;
            }
            if m["role"] == "taucodeSubagentResult" {
                if t.find(m["toolCallId"].as_str().unwrap_or("")).is_some_and(|item| item["type"] == "collabAgentToolCall")
                    && super::agents::results(&m["result"]["details"]).is_some() {
                    if m["crashed"] == true { t.failure = Some("Execution interrupted before completion. Recovered the last saved agent progress.".into()); }
                    t.apply(&json!({"type":"tool_execution_end","replay":true,"toolCallId":m["toolCallId"],"result":m["result"]}));
                    t.aborted = true;
                }
                continue;
            }
            if m["role"] == "taucodeCompaction" {
                if last_stop != "toolUse" {
                    t.apply(&json!({"type":"agent_settled","aborted":t.aborted}));
                }
                t.apply(&json!({"type":"compaction_start"}));
                t.apply(&json!({"type":"compaction_end","result":m["result"]}));
                continue;
            }
            if m["role"] == "user" {
                let queued = m["timestamp"].as_u64().unwrap_or(u64::MAX) < end;
                if t.active && !queued && (last_stop != "toolUse" || t.aborted) {
                    t.apply(&json!({"type":"agent_settled","aborted":t.aborted}));
                }
                t.apply(&json!({"type":"agent_start"}));
            }
            t.apply(&json!({"type":"message_start","message":m}));
            t.apply(&json!({"type":"message_end","message":m}));
            if m["taucodeRecovered"] == true {
                t.failure = Some("Execution interrupted. Recovered an unfinished response.".into());
            }
            if m["role"] == "assistant" {
                end = m["timestamp"].as_u64().unwrap_or(0) + m["durationMs"].as_u64().unwrap_or(0);
                last_stop = m["stopReason"].as_str().unwrap_or("").into();
                for p in m["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["type"] == "toolCall")
                {
                    t.apply(&json!({"type":"tool_execution_start","replay":true,"toolCallId":p["id"],"toolName":p["name"],"args":p["arguments"]}));
                }
            }
            if m["role"] == "toolResult" {
                t.apply(&json!({"type":"tool_execution_end","replay":true,"toolCallId":m["toolCallId"],"toolName":m["toolName"],"result":m,"isError":m["isError"],"durationMs":m["durationMs"]}));
            }
        }
        if last_stop == "toolUse" {
            t.aborted = true;
            if let Some(turn) = t.turns.last_mut() {
                for item in turn["items"].as_array_mut().into_iter().flatten() {
                    if item.get("agentsTree").is_some() { super::agents::settle(&mut item["agentsTree"], "aborted", None); }
                }
            }
        }
        t.apply(&json!({"type":"agent_settled","aborted":t.aborted}));
        t
    }
}

pub(super) fn login_requested(m: &Value) -> bool {
    let text = format!(
        "{} {}",
        m["title"].as_str().unwrap_or(""),
        m["message"].as_str().unwrap_or("")
    )
    .to_lowercase();
    text.trim() == "login"
        || [
            "/login",
            "log in",
            "login to",
            "login required",
            "please login",
            "sign in",
            "oauth",
        ]
        .iter()
        .any(|s| text.contains(s))
}
fn connector_parts(name: &str) -> Option<(&str, &str)> {
    if let Some(split) = name.split_once("__")
        .filter(|(server, tool)| !server.is_empty() && !tool.is_empty() && !tool.contains("__")) {
        return Some(split);
    }
    let mut splits = name.match_indices('_').filter_map(|(i, _)| {
        let (server, tool) = (&name[..i], &name[i + 1..]);
        (!server.is_empty() && !tool.is_empty() && server.rsplit('_').next() == tool.split('_').next())
            .then_some((server, tool))
    });
    let split = splits.next()?;
    splits.next().is_none().then_some(split)
}
pub(super) fn ui_request(m: &Value, thread: &str, turn: &str) -> Value {
    if let Some(approval) = approval(m) {
        if let Some(name) = approval["path"].as_str()
            .filter(|_| matches!(approval["tool"].as_str(), Some("write" | "edit")))
            .and_then(|path| path.strip_prefix("xd://mcp__"))
        {
            let label = connector_parts(name).map(|(server, tool)| format!("{server} · {tool}")).unwrap_or_else(|| name.to_owned());
            return json!({"id":format!("pi:{thread}:{}",m["id"].as_str().unwrap_or("")),"method":"item/commandExecution/requestApproval","params":{"threadId":thread,"turnId":turn,"itemId":approval["toolCallId"],"approvalId":m["id"],"tool":format!("mcp__{name}"),"cwd":approval["cwd"],"reason":format!("Connector requests permission to call {label}"),"availableDecisions":["accept","decline","cancel"],"grantRoot":null}});
        }
        let mcp = approval["tool"]
            .as_str()
            .is_some_and(|t| t.starts_with("mcp__"));
        if matches!(approval["tool"].as_str(), Some("eval" | "lsp" | "debug")) {
            return json!({"id":format!("pi:{thread}:{}",m["id"].as_str().unwrap_or("")),"method":"item/commandExecution/requestApproval","params":{"threadId":thread,"turnId":turn,"itemId":approval["toolCallId"],"approvalId":m["id"],"command":approval["command"],"cwd":approval["cwd"],"reason":format!("Pi requests permission to use {}",approval["tool"].as_str().unwrap()),"availableDecisions":["accept","decline","cancel"]}});
        }
        let bash = approval["tool"] == "bash" || mcp;
        return json!({"id":format!("pi:{thread}:{}",m["id"].as_str().unwrap_or("")),"method":if bash {"item/commandExecution/requestApproval"} else {"item/fileChange/requestApproval"},"params":{"threadId":thread,"turnId":turn,"itemId":approval["toolCallId"],"approvalId":m["id"],"command":approval["command"],"cwd":approval["cwd"],"reason":if mcp {json!("Connector requests permission to call this tool")}else if bash {json!("Pi requests permission to run this command")}else{json!(format!("Pi requests permission to change {}",approval["path"].as_str().unwrap_or("")))},"availableDecisions":if mcp {json!(["accept","decline","cancel"])}else{json!(["accept","acceptForSession","decline","cancel"])},"grantRoot":null,"diffPreview":approval["diffPreview"]}});
    }
    let options = if m["method"] == "confirm" {
        json!(["Yes", "No"])
    } else {
        m["options"].clone()
    };
    json!({"id":format!("pi:{thread}:{}",m["id"].as_str().unwrap_or("")),"method":"item/tool/requestUserInput","params":{"threadId":thread,"turnId":turn,"itemId":m["id"],"isBlocking":true,"autoResolutionMs":m["timeout"],"questions":[{"id":"answer","header":"Pi","question":format!("{}{}",m["title"].as_str().unwrap_or(""),m["message"].as_str().map(|s|format!("\n{s}")).unwrap_or_default()),"isOther":m["method"]!="confirm","isSecret":false,"options":options.as_array().map(|a|a.iter().map(|o|json!({"label":o,"description":""})).collect::<Vec<_>>())}]}})
}
pub(super) fn ui_response(request: &Value, answer: &Value) -> Value {
    let mut response = json!({"type":"extension_ui_response","id":request["id"]});
    if approval(request).is_some() {
        response["value"] = json!(match answer["decision"].as_str() {
            Some("accept") => "Allow",
            Some("acceptForSession") => "Allow for session",
            _ => "Block",
        });
        return response;
    }
    if let Some(value) = answer
        .pointer("/answers/answer/answers/0")
        .and_then(Value::as_str)
    {
        if request["method"] == "confirm" {
            response["confirmed"] = json!(value == "Yes");
        } else {
            response["value"] = json!(value);
        }
    } else {
        response["cancelled"] = json!(true);
    }
    response
}

pub(super) fn approval(m: &Value) -> Option<Value> {
    if m["method"] != "select" {
        return None;
    }
    let value: Value =
        serde_json::from_str(m["title"].as_str()?.strip_prefix("taucode:approval:")?).ok()?;
    if !(matches!(value["tool"].as_str(), Some("bash" | "write" | "edit" | "eval" | "lsp" | "debug"))
        || value["tool"]
            .as_str()
            .is_some_and(|t| t.starts_with("mcp__")))
        || value["toolCallId"].as_str()?.is_empty()
        || !value["cwd"].is_string()
    {
        return None;
    }
    Some(value)
}

pub(super) fn sandbox(mode: &str, cwd: &str) -> Value {
    match mode {
        "full-access" => json!({"type":"dangerFullAccess"}),
        "read-only" => json!({"type":"readOnly"}),
        _ => json!({"type":"workspaceWrite","writableRoots":[cwd],"networkAccess":false}),
    }
}
fn settings_event(id: &str, cwd: &str, provider: &str, model: &str, mode: &str) -> Value {
    json!({"method":"thread/settings/updated","params":{"threadId":id,"threadSettings":{"disabledPluginIds":[],"cwd":cwd,"approvalPolicy":if mode == "full-access" {"never"}else{"on-request"},"approvalsReviewer":"user","sandboxPolicy":sandbox(mode,cwd),"activePermissionProfile":null,"model":model,"modelProvider":provider,"serviceTier":null,"effort":null,"summary":null,"collaborationMode":{"mode":"default","settings":{"model":model,"reasoning_effort":null,"developer_instructions":null}},"multiAgentMode":"explicitRequestOnly","personality":null}}})
}

pub(super) fn markdown_plan(text: &str) -> Vec<Value> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        let (status, step) = if let Some(step) = line.strip_prefix("- [ ] ").or_else(|| line.strip_prefix("* [ ] ")) {
            ("pending", step)
        } else if let Some(step) = line.strip_prefix("- [x] ").or_else(|| line.strip_prefix("- [X] ")).or_else(|| line.strip_prefix("* [x] ")) {
            ("completed", step)
        } else { return None };
        (!step.trim().is_empty()).then(|| json!({"step":step.trim(),"status":status}))
    }).take(100).collect()
}
