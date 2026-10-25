use serde_json::json;
use taucode_diag_mcp::core::*;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures");
    let mut events = vec![
        event(
            "error",
            "engine",
            "engine.spawn_failed",
            "Engine spawn failed",
        ),
        event("perf", "engine", "engine.first_token", "First token"),
        event("usage", "agent", "agent.turn", "Turn completed"),
        event("feedback", "ui", "feedback.idea", "User idea"),
    ];
    for (i, e) in events.iter_mut().enumerate() {
        e.ts = format!("2026-10-25T21:04:0{i}.000Z");
        e.run = "00000000-0000-4000-8000-000000000001".into();
        e.seq = i as u64 + 1;
        e.context =
            json!({"engine":"pi","model":"fixture-model","turnId":"turn-1","codegraph":true});
        e.fingerprint = fingerprint(e);
    }
    events[1].metrics = json!({"durationMs":1250});
    events[2].metrics =
        json!({"tokensIn":1000,"tokensOut":200,"cachedTokens":100,"costUsd":0.01,"toolCalls":2});
    events[3].detail = json!({"note":"Please make retries easier to see"});
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
