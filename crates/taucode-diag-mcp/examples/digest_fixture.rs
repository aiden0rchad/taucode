use serde_json::json;
use taucode_diag_mcp::core::*;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures");
    let thread = "e53185da-2175-4cf4-9846-79306911b175";
    let mut events = vec![event("perf", "engine", "engine.ready", "Engine ready")];
    events[0].context = json!({"engine":"omp"});
    events[0].metrics = json!({"durationMs":8200});
    for turn in 1..=2 {
        let context = json!({"engine":"omp","model":"GLM 5.3","threadId":thread,"turnId":format!("{thread}:turn:{turn}"),"responseId":format!("{thread}:message:1792960351528:{turn}")});
        let mut first = event("perf", "engine", "engine.first_token", "First token");
        first.context = context.clone();
        first.metrics = json!({"durationMs":9717});
        events.push(first);
        let mut usage = event("usage", "agent", "agent.turn", "Turn completed");
        usage.context = context;
        usage.metrics = json!({"tokensIn":129,"tokensOut":102,"cachedTokens":14080,"costUsd":null,"durationMs":9777,"toolCalls":1});
        events.push(usage);
    }
    let mut feedback = event("feedback", "ui", "feedback.idea", "User idea");
    feedback.context = json!({"threadId":thread,"responseId":format!("{thread}:message:1792960351528:2")});
    feedback.detail = json!({"note":format!("Investigate thread {thread}")});
    events.push(feedback);
    for (i, e) in events.iter_mut().enumerate() {
        e.ts = format!("2026-10-25T21:04:0{i}.000Z");
        e.run = "00000000-0000-4000-8000-000000000001".into();
        e.seq = i as u64 + 1;
        *e = sanitize(e.clone(), &Options::default(), "", &[]).unwrap();
    }
    let mut issues = Issues::new();
    aggregate(&mut issues, &events);
    std::fs::write(
        root.join("diagnostics-events.json"),
        serde_json::to_string_pretty(&events).unwrap() + "\n",
    )
    .unwrap();
    std::fs::write(
        root.join("diagnostics-digest.md"),
        digest(&events, &issues, "2026-10-25T21:04:01.000Z"),
    )
    .unwrap();
}
