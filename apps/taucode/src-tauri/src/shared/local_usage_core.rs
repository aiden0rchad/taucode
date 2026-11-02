use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::codex::home::{resolve_default_codex_home, resolve_workspace_codex_home};
use crate::types::{
    LocalUsageDay, LocalUsageModel, LocalUsageSnapshot, LocalUsageTotals, WorkspaceEntry,
};

#[derive(Default, Clone, Copy)]
struct DailyTotals {
    input: i64,
    cached: i64,
    output: i64,
    agent_ms: i64,
    agent_runs: i64,
}

mod ledger;

#[derive(Default, Clone, Copy)]
struct UsageTotals {
    input: i64,
    cached: i64,
    output: i64,
}

const MAX_ACTIVITY_GAP_MS: i64 = 2 * 60 * 1000;

pub(crate) async fn collect_usage(data_dir: PathBuf) {
    loop {
        let data = data_dir.clone();
        match tokio::task::spawn_blocking(move || ledger::sync(&data, &[])).await {
            Ok(Ok(_)) => (),
            Ok(Err(error)) => eprintln!("Usage collection failed: {error}"),
            Err(error) => eprintln!("Usage collection task failed: {error}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

pub(crate) async fn local_usage_snapshot_core(
    data_dir: &Path,
    workspaces: &Mutex<HashMap<String, WorkspaceEntry>>,
    days: Option<u32>,
    workspace_path: Option<String>,
) -> Result<LocalUsageSnapshot, String> {
    let days = days.unwrap_or(30).clamp(1, 90);
    let workspace_path = workspace_path.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    });
    let sessions_roots = {
        let workspaces = workspaces.lock().await;
        resolve_sessions_roots(&workspaces, None)
    };
    let data_dir = data_dir.to_path_buf();
    let snapshot = tokio::task::spawn_blocking(move || {
        let records = ledger::sync(&data_dir, &sessions_roots)?;
        Ok::<_, String>(snapshot_from_records(
            days,
            workspace_path.as_deref(),
            records.values(),
        ))
    })
    .await
    .map_err(|err| err.to_string())??;
    Ok(snapshot)
}

fn snapshot_from_records<'a>(
    days: u32,
    workspace_path: Option<&Path>,
    records: impl Iterator<Item = &'a ledger::Record>,
) -> LocalUsageSnapshot {
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let day_keys = make_day_keys(days);
    let mut daily = day_keys
        .iter()
        .map(|key| (key.clone(), DailyTotals::default()))
        .collect();
    let mut model_totals = HashMap::new();
    let mut all_time = DailyTotals::default();
    for record in records.filter(|record| {
        workspace_path.is_none_or(|path| path_matches_workspace(&record.cwd, path))
    }) {
        record.add_to(&mut all_time);
        record.aggregate(&mut daily, &mut model_totals);
    }
    let mut snapshot = build_snapshot(updated_at, day_keys, daily, model_totals);
    snapshot.totals.all_time_tokens = all_time.input + all_time.output;
    snapshot.totals.all_time_agent_runs = all_time.agent_runs;
    snapshot.totals.all_time_agent_time_ms = all_time.agent_ms;
    snapshot
}

#[cfg(test)]
fn scan_local_usage(
    days: u32,
    workspace_path: Option<&Path>,
    roots: &[PathBuf],
) -> Result<LocalUsageSnapshot, String> {
    let mut records = HashMap::new();
    for root in roots {
        for path in ledger::files(root)? {
            ledger::scan_file(&path, &mut records)?;
        }
    }
    Ok(snapshot_from_records(
        days,
        workspace_path,
        records.values(),
    ))
}

fn build_snapshot(
    updated_at: i64,
    day_keys: Vec<String>,
    daily: HashMap<String, DailyTotals>,
    model_totals: HashMap<String, i64>,
) -> LocalUsageSnapshot {
    let mut days: Vec<LocalUsageDay> = Vec::with_capacity(day_keys.len());
    let mut total_tokens = 0;

    for day_key in &day_keys {
        let totals = daily.get(day_key).copied().unwrap_or_default();
        let total = totals.input + totals.output;
        total_tokens += total;
        days.push(LocalUsageDay {
            day: day_key.clone(),
            input_tokens: totals.input,
            cached_input_tokens: totals.cached,
            output_tokens: totals.output,
            total_tokens: total,
            agent_time_ms: totals.agent_ms,
            agent_runs: totals.agent_runs,
        });
    }

    let last7 = days.iter().rev().take(7).cloned().collect::<Vec<_>>();
    let last7_tokens: i64 = last7.iter().map(|day| day.total_tokens).sum();
    let last7_input: i64 = last7.iter().map(|day| day.input_tokens).sum();
    let last7_cached: i64 = last7.iter().map(|day| day.cached_input_tokens).sum();

    let average_daily_tokens = if last7.is_empty() {
        0
    } else {
        ((last7_tokens as f64) / (last7.len() as f64)).round() as i64
    };

    let cache_hit_rate_percent = if last7_input > 0 {
        ((last7_cached as f64) / (last7_input as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let peak = days
        .iter()
        .max_by_key(|day| day.total_tokens)
        .filter(|day| day.total_tokens > 0);
    let peak_day = peak.map(|day| day.day.clone());
    let peak_day_tokens = peak.map(|day| day.total_tokens).unwrap_or(0);

    let all_model_tokens: i64 = model_totals.values().sum();
    let mut top_models: Vec<LocalUsageModel> = model_totals
        .into_iter()
        .filter(|(_, tokens)| *tokens > 0)
        .map(|(model, tokens)| LocalUsageModel {
            model,
            tokens,
            share_percent: if all_model_tokens > 0 {
                ((tokens as f64) / (all_model_tokens as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            },
        })
        .collect();
    top_models.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.model.cmp(&b.model)));

    LocalUsageSnapshot {
        updated_at,
        days,
        totals: LocalUsageTotals {
            all_time_tokens: 0,
            all_time_agent_runs: 0,
            all_time_agent_time_ms: 0,
            last7_days_tokens: last7_tokens,
            last30_days_tokens: total_tokens,
            average_daily_tokens,
            cache_hit_rate_percent,
            peak_day,
            peak_day_tokens,
        },
        top_models,
    }
}

#[cfg(test)]
fn scan_file(
    path: &Path,
    daily: &mut HashMap<String, DailyTotals>,
    models: &mut HashMap<String, i64>,
    workspace: Option<&Path>,
) -> Result<(), String> {
    let mut records = HashMap::new();
    ledger::scan_file(path, &mut records)?;
    for record in records
        .values()
        .filter(|r| workspace.is_none_or(|p| path_matches_workspace(&r.cwd, p)))
    {
        record.aggregate(daily, models);
    }
    Ok(())
}

fn extract_model_from_turn_context(value: &Value) -> Option<String> {
    let payload = value.get("payload").and_then(|value| value.as_object())?;
    if let Some(model) = payload.get("model").and_then(|value| value.as_str()) {
        return Some(model.to_string());
    }
    let info = payload.get("info").and_then(|value| value.as_object())?;
    info.get("model")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_model_from_token_count(value: &Value) -> Option<String> {
    let payload = value.get("payload").and_then(|value| value.as_object())?;
    let info = payload.get("info").and_then(|value| value.as_object());
    let model = info
        .and_then(|info| {
            info.get("model")
                .or_else(|| info.get("model_name"))
                .and_then(|value| value.as_str())
        })
        .or_else(|| payload.get("model").and_then(|value| value.as_str()))
        .or_else(|| value.get("model").and_then(|value| value.as_str()));
    model.map(|value| value.to_string())
}

fn find_usage_map<'a>(
    info: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    keys.iter()
        .find_map(|key| info.get(*key).and_then(|value| value.as_object()))
}

fn read_i64(map: &serde_json::Map<String, Value>, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| map.get(*key))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value as i64))
        })
        .unwrap_or(0)
        .max(0)
}

fn read_timestamp_ms(value: &Value) -> Option<i64> {
    let raw = value.get("timestamp")?;
    if let Some(text) = raw.as_str() {
        return DateTime::parse_from_rfc3339(text)
            .map(|value| value.timestamp_millis())
            .ok();
    }
    let numeric = raw
        .as_i64()
        .or_else(|| raw.as_f64().map(|value| value as i64))?;
    if numeric > 0 && numeric < 1_000_000_000_000 {
        return Some(numeric * 1000);
    }
    Some(numeric)
}

fn day_key_for_timestamp_ms(timestamp_ms: i64) -> Option<String> {
    let utc = Utc.timestamp_millis_opt(timestamp_ms).single()?;
    Some(utc.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn extract_cwd(value: &Value) -> Option<String> {
    value
        .get("payload")
        .and_then(|payload| payload.get("cwd"))
        .and_then(|cwd| cwd.as_str())
        .map(|cwd| cwd.to_string())
}

fn path_matches_workspace(cwd: &str, workspace_path: &Path) -> bool {
    let cwd_path = Path::new(cwd);
    cwd_path == workspace_path || cwd_path.starts_with(workspace_path)
}

fn make_day_keys(days: u32) -> Vec<String> {
    let today = Local::now().date_naive();
    (0..days)
        .rev()
        .map(|offset| {
            let day = today - Duration::days(offset as i64);
            day.format("%Y-%m-%d").to_string()
        })
        .collect()
}

fn resolve_codex_sessions_root(codex_home_override: Option<PathBuf>) -> Option<PathBuf> {
    codex_home_override
        .or_else(resolve_default_codex_home)
        .map(|home| home.join("sessions"))
}

fn resolve_sessions_roots(
    workspaces: &HashMap<String, WorkspaceEntry>,
    workspace_path: Option<&Path>,
) -> Vec<PathBuf> {
    if let Some(workspace_path) = workspace_path {
        let codex_home_override =
            resolve_workspace_codex_home_for_path(workspaces, Some(workspace_path));
        return resolve_codex_sessions_root(codex_home_override)
            .into_iter()
            .collect();
    }

    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Some(root) = resolve_codex_sessions_root(None) {
        if seen.insert(root.clone()) {
            roots.push(root);
        }
    }

    for entry in workspaces.values() {
        let parent_entry = entry
            .parent_id
            .as_ref()
            .and_then(|parent_id| workspaces.get(parent_id));
        let Some(codex_home) = resolve_workspace_codex_home(entry, parent_entry) else {
            continue;
        };
        if let Some(root) = resolve_codex_sessions_root(Some(codex_home)) {
            if seen.insert(root.clone()) {
                roots.push(root);
            }
        }
    }

    roots
}

fn resolve_workspace_codex_home_for_path(
    workspaces: &HashMap<String, crate::types::WorkspaceEntry>,
    workspace_path: Option<&Path>,
) -> Option<PathBuf> {
    let workspace_path = workspace_path?;
    let entry = workspaces
        .values()
        .filter(|entry| {
            let entry_path = Path::new(&entry.path);
            workspace_path == entry_path || workspace_path.starts_with(entry_path)
        })
        .max_by_key(|entry| entry.path.len())?;

    let parent_entry = entry
        .parent_id
        .as_ref()
        .and_then(|parent_id| workspaces.get(parent_id));

    resolve_workspace_codex_home(entry, parent_entry)
}

#[cfg(test)]
fn day_dir_for_key(root: &Path, day_key: &str) -> PathBuf {
    let mut parts = day_key.split('-');
    let year = parts.next().unwrap_or("1970");
    let month = parts.next().unwrap_or("01");
    let day = parts.next().unwrap_or("01");
    root.join(year).join(month).join(day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{WorkspaceKind, WorkspaceSettings};
    use chrono::NaiveDateTime;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    fn write_temp_jsonl(lines: &[&str]) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "codexmonitor-local-usage-test-{}.jsonl",
            Uuid::new_v4()
        ));
        let mut file = File::create(&path).expect("create temp jsonl");
        for line in lines {
            writeln!(file, "{line}").expect("write jsonl line");
        }
        path
    }

    fn make_temp_sessions_root() -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!("codexmonitor-local-usage-root-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_session_file(root: &Path, day_key: &str, lines: &[String]) -> PathBuf {
        let day_dir = day_dir_for_key(root, day_key);
        fs::create_dir_all(&day_dir).expect("create day dir");
        let path = day_dir.join(format!("usage-{}.jsonl", Uuid::new_v4()));
        let mut file = File::create(&path).expect("create session jsonl");
        for line in lines {
            writeln!(file, "{line}").expect("write jsonl line");
        }
        path
    }

    #[test]
    fn scan_file_does_not_double_count_last_and_total_usage() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5}}}}"#,
            r#"{"timestamp":"2026-01-19T12:00:01.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.input, 10);
        assert_eq!(totals.output, 5);
    }

    #[test]
    fn scan_file_counts_last_deltas_before_total_snapshot_once() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5}}}}"#,
            r#"{"timestamp":"2026-01-19T12:00:01.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":20,"cached_input_tokens":0,"output_tokens":10}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.input, 20);
        assert_eq!(totals.output, 10);
    }

    #[test]
    fn scan_file_does_not_double_count_last_between_total_snapshots() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5}}}}"#,
            r#"{"timestamp":"2026-01-19T12:00:01.000Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":2,"cached_input_tokens":0,"output_tokens":1}}}}"#,
            r#"{"timestamp":"2026-01-19T12:00:02.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":12,"cached_input_tokens":0,"output_tokens":6}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.input, 12);
        assert_eq!(totals.output, 6);
    }

    #[test]
    fn scan_file_tracks_agent_time_from_activity() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}}}"#,
            r#"{"timestamp":"2026-01-19T12:00:05.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":2,"cached_input_tokens":0,"output_tokens":2}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.agent_ms, 5_000);
    }

    #[test]
    fn scan_file_counts_runs_from_assistant_messages() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:05.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"a"}]}}"#,
            r#"{"timestamp":"2026-01-19T12:00:10.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"b"}]}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.agent_runs, 2);
    }

    #[test]
    fn scan_file_ignores_large_gaps_between_activity() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}}}"#,
            r#"{"timestamp":"2026-01-19T12:10:00.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":2,"cached_input_tokens":0,"output_tokens":2}}}}"#,
            r#"{"timestamp":"2026-01-19T12:10:10.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":3,"cached_input_tokens":0,"output_tokens":3}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(&path, &mut daily, &mut model_totals, None).expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.agent_ms, 10_000);
    }

    #[test]
    fn scan_file_skips_workspace_mismatch() {
        let day_key = "2026-01-19";
        let path = write_temp_jsonl(&[
            r#"{"timestamp":"2026-01-19T12:00:00.000Z","type":"session_meta","payload":{"cwd":"/tmp/project-alpha"}}"#,
            r#"{"timestamp":"2026-01-19T12:00:10.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}}"#,
            r#"{"timestamp":"2026-01-19T12:00:12.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":0,"output_tokens":5}}}}"#,
        ]);

        let mut daily: HashMap<String, DailyTotals> = HashMap::new();
        daily.insert(day_key.to_string(), DailyTotals::default());
        let mut model_totals: HashMap<String, i64> = HashMap::new();
        scan_file(
            &path,
            &mut daily,
            &mut model_totals,
            Some(Path::new("/tmp/other-project")),
        )
        .expect("scan file");

        let totals = daily.get(day_key).copied().unwrap_or_default();
        assert_eq!(totals.agent_ms, 0);
        assert_eq!(totals.input, 0);
    }

    #[test]
    fn scan_local_usage_aggregates_multiple_session_roots() {
        let day_keys = make_day_keys(2);
        let day_key = day_keys
            .last()
            .cloned()
            .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
        let naive =
            NaiveDateTime::parse_from_str(&format!("{day_key} 12:00:00"), "%Y-%m-%d %H:%M:%S")
                .expect("timestamp");
        let timestamp_ms = Local
            .from_local_datetime(&naive)
            .single()
            .expect("timestamp")
            .timestamp_millis();

        let root_a = make_temp_sessions_root();
        let root_b = make_temp_sessions_root();

        let line_a = format!(
            r#"{{"timestamp":{timestamp_ms},"payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":5,"cached_input_tokens":0,"output_tokens":2}}}}}}}}"#
        );
        let line_b = format!(
            r#"{{"timestamp":{timestamp_ms},"payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":3,"cached_input_tokens":0,"output_tokens":1}}}}}}}}"#
        );

        write_session_file(&root_a, &day_key, &[line_a]);
        write_session_file(&root_b, &day_key, &[line_b]);

        let snapshot = scan_local_usage(2, None, &[root_a, root_b]).expect("scan usage");
        let day = snapshot
            .days
            .iter()
            .find(|entry| entry.day == day_key)
            .expect("day entry");

        assert_eq!(day.input_tokens, 8);
        assert_eq!(day.output_tokens, 3);
        assert_eq!(snapshot.totals.last30_days_tokens, 11);
    }

    #[test]
    fn resolve_sessions_roots_uses_single_default_root() {
        let mut workspaces = HashMap::new();
        let entry_a = WorkspaceEntry {
            id: "a".to_string(),
            name: "A".to_string(),
            path: "/tmp/project-a".to_string(),
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings: WorkspaceSettings::default(),
        };
        let entry_b = WorkspaceEntry {
            id: "b".to_string(),
            name: "B".to_string(),
            path: "/tmp/project-b".to_string(),
            kind: WorkspaceKind::Main,
            parent_id: None,
            worktree: None,
            settings: WorkspaceSettings::default(),
        };
        workspaces.insert(entry_a.id.clone(), entry_a.clone());
        workspaces.insert(entry_b.id.clone(), entry_b.clone());

        let roots = resolve_sessions_roots(&workspaces, None);
        let expected = resolve_codex_sessions_root(None)
            .map(|root| vec![root])
            .unwrap_or_default();
        assert_eq!(roots, expected);
    }
}
