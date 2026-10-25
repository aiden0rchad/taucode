use super::*;
fn sample() -> Event {
    let mut e = event(
        "error",
        "engine",
        "engine.spawn_failed",
        "Spawn 123 failed at /Users/test/project",
    );
    e.ts = "2026-10-25T00:00:00.000Z".into();
    sanitize(e, &Options::default(), "/Users/test", &[]).unwrap()
}
#[test]
fn shared_redaction_fixtures() {
    let data: Value = serde_json::from_str(REDACTION).unwrap();
    for row in data["fixtures"].as_array().unwrap() {
        assert_eq!(
            redact(
                row[0].as_str().unwrap(),
                "/Users/test",
                &["real-key".into()]
            ),
            row[1].as_str().unwrap()
        );
    }
}
#[test]
fn fingerprint_normalization_and_stack() {
    let a = sample();
    let mut b = a.clone();
    b.message = "Spawn 999 failed at /other/path".into();
    assert_eq!(fingerprint(&a), fingerprint(&b));
    b.code = "engine.exit_nonzero".into();
    assert_ne!(fingerprint(&a), fingerprint(&b));
    b = a.clone();
    b.detail = json!({"stack":"other_frame"});
    assert_ne!(fingerprint(&a), fingerprint(&b));
}
#[test]
fn aggregation_regression_annotations() {
    let dir = tempfile::tempdir().unwrap();
    let mut inbox = Issues::new();
    let e = sample();
    aggregate(&mut inbox, &[e.clone(), e.clone()]);
    atomic(
        dir.path(),
        "issues.json",
        &serde_json::to_vec(&inbox).unwrap(),
    )
    .unwrap();
    annotate(
        dir.path(),
        &e.fingerprint,
        Some("fixed"),
        "checked sk-secret",
    )
    .unwrap();
    let mut seen = BTreeSet::new();
    merge_annotations(&mut inbox, dir.path(), &mut seen);
    assert_eq!(inbox[&e.fingerprint].status, "fixed");
    assert_eq!(inbox[&e.fingerprint].notes, vec!["checked [REDACTED]"]);
    let mut next = e.clone();
    next.ts = "2026-10-26T00:00:00.000Z".into();
    next.severity = "critical".into();
    aggregate(&mut inbox, &[next.clone()]);
    merge_annotations(&mut inbox, dir.path(), &mut seen);
    let i = &inbox[&e.fingerprint];
    assert_eq!(i.count, 3);
    assert_eq!(i.first_seen, e.ts);
    assert_eq!(i.last_seen, next.ts);
    assert_eq!(i.severity, "critical");
    assert_eq!(i.status, "open");
    assert!(i.regressed);
}
#[test]
fn partial_append_rotation_retention_cap() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("events-2000-01-01.ndjson");
    fs::write(&p, "{\"a\":1}\n{\"partial\":").unwrap();
    append(&p, b"{\"b\":2}\n").unwrap();
    assert_eq!(lines::<Value>(&p), vec![json!({"a":1}), json!({"b":2})]);
    let today = dir
        .path()
        .join(format!("events-{}.ndjson", Utc::now().format("%Y-%m-%d")));
    fs::write(&today, "{}\n").unwrap();
    retain(dir.path(), 7, CAP).unwrap();
    assert!(!p.exists());
    assert!(today.exists());
    retain(dir.path(), 7, 1).unwrap();
    assert!(!today.exists());
}
#[test]
fn content_is_opt_in_and_metadata_allowlisted() {
    let mut e = sample();
    e.detail = json!({"stderrTail":"sk-secret private output","prompt":"private prompt","note":"user@example.com"});
    e.context = json!({"workspace":"/Users/test/project","apiKey":"secret","response":"private"});
    let e = sanitize(e, &Options::default(), "/Users/test", &[]).unwrap();
    assert!(e.detail.get("stderrTail").is_none());
    assert!(e.detail.get("prompt").is_none());
    assert!(e.context.get("apiKey").is_none());
    assert_eq!(e.context["workspace"], "~/project");
    assert_eq!(e.detail["note"], "[REDACTED]");
}
#[test]
fn signals_and_rules() {
    let mut t = signals::Turn {
        context: json!({"model":"m"}),
        compacted: true,
        ..Default::default()
    };
    for _ in 0..4 {
        t.tool("read_file", &json!({"path":"private"}), true);
    }
    let mut h = BTreeMap::from([("m".into(), vec![20_000])]);
    let out = t.finish(json!({"tokensIn":60_000,"tokensOut":1}), &mut h);
    for code in [
        "agent.tool_loop",
        "agent.reread",
        "agent.high_tokens",
        "agent.compaction",
        "agent.tool_errors",
    ] {
        assert!(out.iter().any(|e| e.code == code));
    }
    for &(code, n, text, _) in digest::RULES {
        assert!(!digest(&[], &Issues::new(), "").contains(text));
        let events = vec![event("warning", "agent", code, code); n];
        assert!(digest(&events, &Issues::new(), "").contains(text));
    }
}
#[test]
fn digest_golden() {
    assert_eq!(
        digest(&[], &Issues::new(), ""),
        include_str!("../../../../../docs/fixtures/diagnostics-empty-digest.md")
    );
}

#[test]
fn toast_link_fingerprint() {
    assert_eq!(
        fingerprint(&event(
            "error",
            "ui",
            "ui.error_toast",
            "Error notification displayed"
        )),
        "697cfae5c4ba4fe3"
    );
}

#[test]
fn digest_fixture_golden() {
    let events: Vec<Event> = serde_json::from_str(include_str!(
        "../../../../../docs/fixtures/diagnostics-events.json"
    ))
    .unwrap();
    let mut issues = Issues::new();
    aggregate(&mut issues, &events);
    assert_eq!(
        digest(&events, &issues, "2026-10-25T21:04:01.000Z"),
        include_str!("../../../../../docs/fixtures/diagnostics-digest.md")
    );
}
