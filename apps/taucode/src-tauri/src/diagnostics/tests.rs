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

#[test]
fn issue_kinds_and_perf_thresholds() {
    let mut inbox = Issues::new();
    let mut accepted = Vec::new();
    for (code, threshold) in [
        ("ipc.slow", 300.0),
        ("ui.long_task", 150.0),
        ("git.slow", 500.0),
        ("startup.interactive", 5000.0),
        ("startup.first_paint", 5000.0),
        ("engine.first_token", 10000.0),
    ] {
        let mut e = event("perf", "ui", code, code);
        e.fingerprint = fingerprint(&e);
        for duration in [0.0, threshold - 1.0, threshold] {
            e.metrics = json!({"durationMs": duration});
            assert!(!is_issue(&e), "{code} at {duration}");
        }
        e.metrics = json!({"durationMs": threshold + 1.0});
        assert!(is_issue(&e), "{code} above threshold");
        accepted.push(e);
    }
    for code in [
        "agent.tool_loop",
        "agent.reread",
        "agent.high_tokens",
        "agent.aborted",
        "agent.denied",
        "agent.rewound",
        "agent.compaction",
    ] {
        let mut e = event("usage", "agent", code, code);
        e.fingerprint = fingerprint(&e);
        assert!(is_issue(&e));
        accepted.push(e);
    }
    for kind in ["error", "warning"] {
        let mut e = event(kind, "ui", "ui.problem", kind);
        e.fingerprint = kind.into();
        assert!(is_issue(&e));
        accepted.push(e);
    }
    aggregate(&mut inbox, &accepted);
    assert_eq!(inbox.len(), accepted.len());
    for (kind, code) in [
        ("usage", "agent.turn"),
        ("feedback", "feedback.idea"),
        ("feedback", "agent.reread"),
        ("perf", "engine.spawn"),
        ("ux", "ui.panel_open"),
    ] {
        let mut e = event(kind, "ui", code, code);
        e.metrics = json!({"durationMs": 50000});
        assert!(!is_issue(&e));
        aggregate(&mut inbox, &[e]);
    }
    assert_eq!(inbox.len(), accepted.len());
}

#[test]
fn severity_then_decayed_count_then_recency_rank_both_views() {
    let mut inbox = Issues::new();
    for (code, severity, count, ts) in [
        ("critical", "critical", 1, "2026-10-20T00:00:00Z"),
        ("high", "high", 1, "2026-10-20T00:00:00Z"),
        ("medium", "medium", 1, "2026-10-20T00:00:00Z"),
        ("recent-many", "low", 4, "2026-10-25T00:00:00Z"),
        ("old-many", "low", 6, "2026-10-23T00:00:00Z"),
        ("recent-tie", "low", 2, "2026-10-25T00:00:00Z"),
    ] {
        let mut e = event("error", "engine", code, code);
        e.fingerprint = code.into();
        e.severity = severity.into();
        e.ts = ts.into();
        aggregate(&mut inbox, &vec![e; count]);
    }
    let expected = [
        "critical",
        "high",
        "medium",
        "recent-many",
        "recent-tie",
        "old-many",
    ];
    assert_eq!(
        ranked_issues(&inbox, &[])
            .iter()
            .map(|i| i.code.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    let markdown = digest(&[], &inbox, "");
    let open = markdown
        .split("## Open issues\n")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap();
    let positions = expected.map(|code| open.find(&format!("- {code} [")).unwrap());
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn legacy_indexes_drop_samples_without_losing_real_issue_annotations() {
    let dir = tempfile::tempdir().unwrap();
    let e = sample();
    let mut inbox = Issues::new();
    aggregate(&mut inbox, &[e.clone()]);
    inbox.get_mut(&e.fingerprint).unwrap().status = "acknowledged".into();
    inbox
        .get_mut(&e.fingerprint)
        .unwrap()
        .notes
        .push("Keep this note".into());
    // Model an index produced by the former unfiltered aggregator.
    for (kind, code) in [
        ("perf", "engine.first_token"),
        ("usage", "agent.turn"),
        ("feedback", "feedback.idea"),
    ] {
        let mut old = inbox[&e.fingerprint].clone();
        old.fingerprint = code.into();
        old.last_event = event(kind, "agent", code, "Routine sample");
        old.last_event.metrics = json!({"durationMs": 1250});
        inbox.insert(code.into(), old);
    }
    atomic(
        dir.path(),
        "issues.json",
        &serde_json::to_vec(&inbox).unwrap(),
    )
    .unwrap();
    for actual in [issues(dir.path()), journal_view(dir.path()).issues] {
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[&e.fingerprint].status, "acknowledged");
        assert_eq!(actual[&e.fingerprint].notes, ["Keep this note"]);
    }
}
