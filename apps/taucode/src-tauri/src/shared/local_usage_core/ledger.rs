use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

// Stored outside the application bundle and build output. Only accounting metadata,
// never prompts, answers, credentials, or tool output, goes into this ledger.
#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    version: u32,
    #[serde(default)]
    parser_version: u32,
    records: HashMap<String, Record>,
    files: HashMap<PathBuf, (u64, u128)>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(super) struct Record {
    timestamp: i64,
    pub cwd: String,
    model: String,
    input: i64,
    cached: i64,
    output: i64,
    runs: i64,
    agent_ms: i64,
}
impl Record {
    pub fn add_to(&self, totals: &mut DailyTotals) {
        totals.input += self.input;
        totals.cached += self.cached.min(self.input);
        totals.output += self.output;
        totals.agent_runs += self.runs;
        totals.agent_ms += self.agent_ms;
    }
    pub fn aggregate(
        &self,
        daily: &mut HashMap<String, DailyTotals>,
        models: &mut HashMap<String, i64>,
    ) {
        if let Some(totals) =
            day_key_for_timestamp_ms(self.timestamp).and_then(|day| daily.get_mut(&day))
        {
            self.add_to(totals);
        }
        *models.entry(self.model.clone()).or_default() += self.input + self.output;
    }
}

pub(super) fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(e) => return Err(format!("Read usage history {}: {e}", root.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        // Do not follow symlinks out of managed history or into directory cycles.
        if kind.is_dir() {
            result.extend(files(&entry.path())?);
        } else if kind.is_file() && entry.path().extension().is_some_and(|e| e == "jsonl") {
            result.push(entry.path());
        }
    }
    result.sort();
    Ok(result)
}

pub(super) fn sync(
    data: &Path,
    codex_roots: &[PathBuf],
) -> Result<HashMap<String, Record>, String> {
    let directory = data.join("usage");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("ledger.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?; // App and daemon can refresh simultaneously.
    let path = directory.join("ledger-v1.json");
    let mut ledger: Ledger = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid usage ledger {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ledger {
            version: 1,
            ..Ledger::default()
        },
        Err(e) => return Err(e.to_string()),
    };
    if ledger.version != 1 {
        return Err("Unsupported usage ledger version".into());
    }
    let parser_changed = ledger.parser_version != 1;
    if parser_changed {
        ledger.files.clear();
        ledger.parser_version = 1;
    }
    let mut roots = codex_roots.to_vec();
    roots.push(data.join("engines/codex-home/sessions"));
    roots.push(data.join("engines/codex-home/archived_sessions"));
    for root in codex_roots {
        if let Some(home) = root.parent() {
            roots.push(home.join("archived_sessions"));
        }
    }
    // Built-in and custom pi-RPC engines use <engine>/sessions. Chat/Work share
    // these formats. Claude's native transcripts live in <engine>/<workspace>/projects.
    for entry in fs::read_dir(data).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        roots.push(entry.path().join("sessions"));
        if entry.file_name() == "claude" {
            for workspace in fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                let workspace = workspace.map_err(|e| e.to_string())?;
                if workspace.file_type().map_err(|e| e.to_string())?.is_dir() {
                    roots.push(workspace.path().join("projects"));
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    let mut changed = !path.exists() || parser_changed;
    for root in roots {
        for file in files(&root)? {
            let metadata = fs::metadata(&file).map_err(|e| e.to_string())?;
            let stamp = (
                metadata.len(),
                metadata
                    .modified()
                    .map_err(|e| e.to_string())?
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            // Checkpoints can change while the session itself is unchanged.
            let checkpoints = ["agents", "streams"]
                .map(|suffix| PathBuf::from(format!("{}.{suffix}", file.display())));
            let has_checkpoints = checkpoints.iter().any(|p| {
                fs::read_dir(p).is_ok_and(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .any(|entry| entry.path().extension().is_some_and(|e| e == "json"))
                })
            });
            if ledger.files.get(&file) == Some(&stamp) && !has_checkpoints {
                continue;
            }
            scan_file(&file, &mut ledger.records)?;
            ledger.files.insert(file, stamp);
            changed = true;
        }
    }
    if changed {
        let temporary = directory.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|e| e.to_string())?;
            serde_json::to_writer(&mut file, &ledger).map_err(|e| e.to_string())?;
            file.flush()
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            File::open(&directory)
                .and_then(|file| file.sync_all())
                .map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result?;
    }
    Ok(ledger.records)
}

fn put(records: &mut HashMap<String, Record>, identity: Value, record: Record) {
    let key = format!("{:x}", Sha256::digest(identity.to_string().as_bytes()));
    let old = records.entry(key).or_insert_with(|| record.clone());
    // Streaming/recovery snapshots can arrive out of order. Never erase billed
    // work on rewind, transcript deletion, fork, or a shorter recovery snapshot.
    old.input = old.input.max(record.input);
    old.cached = old.cached.max(record.cached);
    old.output = old.output.max(record.output);
    old.runs = old.runs.max(record.runs);
    old.agent_ms = old.agent_ms.max(record.agent_ms);
}

fn number(value: &Value, key: &str) -> i64 {
    value[key].as_i64().unwrap_or(0).max(0)
}
fn elapsed(previous: Option<i64>, timestamp: i64) -> i64 {
    previous
        .map(|p| timestamp.saturating_sub(p))
        .filter(|d| *d > 0 && *d <= MAX_ACTIVITY_GAP_MS)
        .unwrap_or(0)
}

fn pi_message(
    message: &Value,
    cwd: &str,
    context: &str,
    fallback_time: i64,
    records: &mut HashMap<String, Record>,
    depth: usize,
) {
    if depth > 32 {
        return;
    }
    let timestamp = read_timestamp_ms(message).unwrap_or(fallback_time);
    if message["role"] == "assistant" && message["usage"].is_object() {
        let usage = &message["usage"];
        let cached = number(usage, "cacheRead");
        let input = number(usage, "input") + cached + number(usage, "cacheWrite");
        let output = number(usage, "output");
        put(
            records,
            json!([
                "pi",
                context,
                timestamp,
                message["provider"],
                message["model"]
            ]),
            Record {
                timestamp,
                cwd: cwd.into(),
                model: message["model"].as_str().unwrap_or("unknown").into(),
                input,
                cached,
                output,
                runs: i64::from(input + output > 0),
                // Pi stores request start on the message and completion on the entry.
                agent_ms: fallback_time.saturating_sub(timestamp).max(0),
            },
        );
    }
    if message["role"] == "toolResult" || message["role"] == "taucodeToolProgress" {
        let details = if message["details"].is_object() {
            &message["details"]
        } else {
            &message["result"]["details"]
        };
        pi_children(
            details,
            message["toolCallId"].as_str().unwrap_or(context),
            cwd,
            timestamp,
            records,
            depth + 1,
        );
    }
}

fn pi_children(
    details: &Value,
    call: &str,
    cwd: &str,
    timestamp: i64,
    records: &mut HashMap<String, Record>,
    depth: usize,
) {
    let details = if details["xdev"]["tool"] == "subagent" {
        &details["xdev"]["inner"]
    } else {
        details
    };
    for (index, result) in details["results"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let context = format!("{call}:{index}");
        let Some(messages) = result["messages"].as_array() else {
            continue;
        };
        for (index, message) in messages.iter().enumerate() {
            let completed = messages
                .get(index + 1)
                .and_then(read_timestamp_ms)
                .or_else(|| result["endedAt"].as_i64())
                .unwrap_or(timestamp);
            pi_message(message, cwd, &context, completed, records, depth);
        }
    }
}

pub(super) fn scan_file(path: &Path, records: &mut HashMap<String, Record>) -> Result<(), String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Read usage {}: {e}", path.display())),
    };
    let mut cwd = String::new();
    let mut model = String::from("unknown");
    let mut previous = UsageTotals::default();
    let mut activity = None;
    let mut pi = false;
    let mut seen = HashSet::new();
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) if !line.ends_with('\n') => break, // writer hasn't finished the last record
            Err(e) => return Err(format!("Invalid usage history {}: {e}", path.display())),
        };
        let identity = format!("{:x}", Sha256::digest(value.to_string().as_bytes()));
        if !seen.insert(identity) {
            continue;
        }
        let kind = value["type"].as_str().unwrap_or("");
        if let Some(path) = value["cwd"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| extract_cwd(&value))
        {
            cwd = path;
        }
        if kind == "session" {
            pi = true;
            continue;
        }
        if kind == "turn_context" {
            if let Some(next) = extract_model_from_turn_context(&value) {
                model = next;
            }
            continue;
        }
        let Some(timestamp) = read_timestamp_ms(&value) else {
            continue;
        };
        if pi {
            if kind == "message" {
                pi_message(
                    &value["message"],
                    &cwd,
                    value["parentId"].as_str().unwrap_or(""),
                    timestamp,
                    records,
                    0,
                );
            }
            if value["customType"] == "taucode-subagent-result" {
                pi_children(
                    &value["data"]["result"]["details"],
                    value["data"]["toolCallId"].as_str().unwrap_or(""),
                    &cwd,
                    timestamp,
                    records,
                    0,
                );
            }
            continue;
        }
        if kind == "assistant" && value["message"]["usage"].is_object() {
            // Claude input excludes cache reads/writes; reasoning is already in output.
            let message = &value["message"];
            let usage = &message["usage"];
            let cached = number(usage, "cache_read_input_tokens");
            put(
                records,
                json!(["claude", message["id"], value["requestId"]]),
                Record {
                    timestamp,
                    cwd: cwd.clone(),
                    model: message["model"].as_str().unwrap_or("unknown").into(),
                    input: number(usage, "input_tokens")
                        + cached
                        + number(usage, "cache_creation_input_tokens"),
                    cached,
                    output: number(usage, "output_tokens"),
                    runs: 1,
                    agent_ms: elapsed(activity, timestamp),
                },
            );
            activity = Some(timestamp);
            continue;
        }
        let payload = &value["payload"];
        let payload_type = payload["type"].as_str().unwrap_or("");
        let mut record = Record {
            timestamp,
            cwd: cwd.clone(),
            model: model.clone(),
            ..Record::default()
        };
        if payload_type == "token_count" {
            let Some(info) = payload["info"].as_object() else {
                continue;
            };
            let total = find_usage_map(info, &["total_token_usage", "totalTokenUsage"]);
            let Some(usage) =
                total.or_else(|| find_usage_map(info, &["last_token_usage", "lastTokenUsage"]))
            else {
                continue;
            };
            let next = UsageTotals {
                input: read_i64(usage, &["input_tokens", "inputTokens"]),
                cached: read_i64(
                    usage,
                    &[
                        "cached_input_tokens",
                        "cache_read_input_tokens",
                        "cachedInputTokens",
                        "cacheReadInputTokens",
                    ],
                ),
                output: read_i64(usage, &["output_tokens", "outputTokens"]),
            };
            if total.is_some() {
                // A lower cumulative total starts a fresh accounting segment (compaction/restart).
                let reset = next.input < previous.input || next.output < previous.output;
                record.input = if reset {
                    next.input
                } else {
                    next.input - previous.input
                };
                record.cached = if reset {
                    next.cached
                } else {
                    (next.cached - previous.cached).max(0)
                };
                record.output = if reset {
                    next.output
                } else {
                    next.output - previous.output
                };
                previous = next;
            } else {
                record.input = next.input;
                record.cached = next.cached;
                record.output = next.output;
                previous.input += next.input;
                previous.cached += next.cached;
                previous.output += next.output;
            }
            if record.input + record.output == 0 {
                continue;
            }
            record.model = extract_model_from_token_count(&value).unwrap_or_else(|| model.clone());
        } else if kind == "response_item" && payload["role"] == "assistant" {
            record.runs = 1;
        } else if !matches!(payload_type, "agent_reasoning" | "agent_message")
            && !(kind == "response_item" && payload_type != "message")
        {
            if kind == "user" {
                activity = Some(timestamp);
            }
            continue;
        }
        record.agent_ms = elapsed(activity, timestamp);
        activity = Some(timestamp);
        // Timestamp + payload deduplicates copied/forked Codex history.
        let identity = if record.runs > 0 {
            json!(["codex-run", cwd, timestamp])
        } else {
            json!(["codex", cwd, timestamp, payload])
        };
        put(records, identity, record);
    }
    if pi {
        for suffix in ["agents", "streams"] {
            let directory = PathBuf::from(format!("{}.{suffix}", path.display()));
            let entries = match fs::read_dir(directory) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.to_string()),
            };
            for entry in entries {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.path().extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let bytes = match fs::read(entry.path()) {
                    Ok(bytes) => bytes,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.to_string()),
                };
                let saved: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                let timestamp = saved["savedAt"].as_i64().unwrap_or(0);
                if saved["version"] != 1 || timestamp <= 0 {
                    continue;
                }
                if saved["result"]["role"] == "assistant" {
                    pi_message(
                        &saved["result"],
                        &cwd,
                        saved["anchor"].as_str().unwrap_or(""),
                        timestamp,
                        records,
                        0,
                    );
                } else {
                    pi_children(
                        &saved["result"]["details"],
                        saved["toolCallId"].as_str().unwrap_or(""),
                        &cwd,
                        timestamp,
                        records,
                        0,
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Explicit local backfill; requires TAUCODE_USAGE_DATA_DIR"]
    fn backfill_existing_usage() {
        let data = PathBuf::from(
            std::env::var_os("TAUCODE_USAGE_DATA_DIR").expect("Set TAUCODE_USAGE_DATA_DIR"),
        );
        assert!(data.is_dir());
        let records = sync(&data, &[]).expect("backfill usage");
        println!(
            "{}",
            serde_json::to_string(&snapshot_from_records(30, None, records.values())).unwrap()
        );
        let repeated = sync(&data, &[]).expect("reload usage");
        assert_eq!(
            snapshot_from_records(30, None, records.values())
                .totals
                .all_time_tokens,
            snapshot_from_records(30, None, repeated.values())
                .totals
                .all_time_tokens
        );
    }

    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("taucode-usage-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn write(path: &Path, values: &[Value]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            values.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
    }
    fn assistant(timestamp: i64) -> Value {
        json!({"role":"assistant","model":"model-a","provider":"provider-a","timestamp":timestamp,
            "usage":{"input":10,"cacheRead":20,"cacheWrite":5,"output":7,"totalTokens":42}})
    }

    #[test]
    fn pi_time_uses_request_start_and_completion_instead_of_user_idle_time() {
        let mut records = HashMap::new();
        let start = Utc::now().timestamp_millis();
        pi_message(
            &assistant(start),
            "/project",
            "parent",
            start + 180_000,
            &mut records,
            0,
        );
        let snapshot = snapshot_from_records(30, None, records.values());
        assert_eq!(snapshot.totals.all_time_agent_time_ms, 180_000);
        assert_eq!(snapshot.totals.all_time_agent_runs, 1);
    }
    fn session(timestamp: i64) -> Vec<Value> {
        vec![
            json!({"type":"session","id":"original","cwd":"/project","timestamp":timestamp-1000}),
            json!({"type":"message","id":"user","timestamp":timestamp-1000,"message":{"role":"user","timestamp":timestamp-1000}}),
            json!({"type":"message","id":"answer","parentId":"user","timestamp":timestamp,"message":assistant(timestamp)}),
        ]
    }

    #[test]
    fn persists_all_engines_forks_deleted_history_and_old_sessions_across_restarts() {
        let root = root();
        let now = Utc::now().timestamp_millis();
        let pi = root.join("pi/sessions/workspace/one.jsonl");
        write(&pi, &session(now));
        let mut fork = session(now);
        fork[0]["id"] = json!("fork");
        write(&root.join("pi/sessions/workspace/fork.jsonl"), &fork);
        write(
            &root.join("omp/sessions/workspace/one.jsonl"),
            &session(now + 1),
        );
        write(
            &root.join("chats/sessions/chats/one.jsonl"),
            &session(now + 2),
        );
        // Session folder creation date is irrelevant to event dates.
        write(
            &root.join("engines/codex-home/archived_sessions/2020/01/01/one.jsonl"),
            &[
                json!({"type":"session_meta","payload":{"cwd":"/other"}}),
                json!({"type":"turn_context","payload":{"model":"codex-model"}}),
                json!({"timestamp":now,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":50}}}}),
            ],
        );
        write(
            &root.join("pi/sessions/workspace/old.jsonl"),
            &session(now - 120 * 86_400_000),
        );
        let first = sync(&root, &[]).unwrap();
        let snapshot = snapshot_from_records(30, None, first.values());
        assert_eq!(snapshot.totals.last30_days_tokens, 276);
        assert_eq!(snapshot.totals.all_time_tokens, 318);
        assert_eq!(snapshot.totals.all_time_agent_runs, 4);
        assert_eq!(
            snapshot_from_records(30, Some(Path::new("/project")), first.values())
                .totals
                .all_time_tokens,
            168
        );
        assert_eq!(sync(&root, &[]).unwrap().len(), first.len());
        fs::remove_dir_all(root.join("pi")).unwrap();
        fs::remove_dir_all(root.join("omp")).unwrap();
        fs::remove_dir_all(root.join("chats")).unwrap();
        fs::remove_dir_all(root.join("engines")).unwrap();
        let restarted = sync(&root, &[]).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, restarted.values())
                .totals
                .all_time_tokens,
            318
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn counts_nested_subagents_once_across_checkpoints_and_completed_results() {
        let root = root();
        let now = Utc::now().timestamp_millis();
        let path = root.join("pi/sessions/w/thread.jsonl");
        let nested = json!({"role":"toolResult","toolCallId":"nested","details":{"results":[{"messages":[assistant(now+2)]}]}});
        let details = json!({"results":[{"messages":[assistant(now+1),nested]}]});
        let mut entries = session(now);
        entries.push(json!({"type":"message","timestamp":now+3,"message":{"role":"toolResult","toolCallId":"call","details":details}}));
        entries.push(json!({"type":"custom","timestamp":now+4,"customType":"taucode-subagent-result","data":{"toolCallId":"call","result":{"details":details}}}));
        write(&path, &entries);
        let checkpoint = PathBuf::from(format!("{}.agents/recovery.json", path.display()));
        fs::create_dir_all(checkpoint.parent().unwrap()).unwrap();
        fs::write(
            &checkpoint,
            json!({"version":1,"savedAt":now+5,"toolCallId":"call","result":{"details":details}})
                .to_string(),
        )
        .unwrap();
        let records = sync(&root, &[]).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, records.values())
                .totals
                .all_time_tokens,
            126
        );
        fs::remove_file(checkpoint).unwrap();
        assert_eq!(sync(&root, &[]).unwrap().len(), records.len());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_tail_is_retried_and_large_assistant_messages_are_counted() {
        let root = root();
        let path = root.join("pi/sessions/w/thread.jsonl");
        let now = Utc::now().timestamp_millis();
        let mut entries = session(now);
        entries[2]["message"]["content"] = json!([{"type":"text","text":"x".repeat(600_000)}]);
        write(&path, &entries);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"type\":")
            .unwrap();
        let first = sync(&root, &[]).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, first.values())
                .totals
                .all_time_tokens,
            42
        );
        entries.push(json!({"type":"message","timestamp":now+1,"message":assistant(now+1)}));
        write(&path, &entries);
        let next = sync(&root, &[]).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, next.values())
                .totals
                .all_time_tokens,
            84
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovered_stream_is_reconciled_with_its_final_message() {
        let root = root();
        let now = Utc::now().timestamp_millis();
        let path = root.join("pi/sessions/w/thread.jsonl");
        let entries = session(now);
        write(&path, &entries[..2]);
        let checkpoint = PathBuf::from(format!("{}.streams/recovery.json", path.display()));
        fs::create_dir_all(checkpoint.parent().unwrap()).unwrap();
        let mut partial = assistant(now);
        partial["usage"]["output"] = json!(2);
        fs::write(
            &checkpoint,
            json!({"version":1,"savedAt":now,"anchor":"user","result":partial}).to_string(),
        )
        .unwrap();
        assert_eq!(
            snapshot_from_records(30, None, sync(&root, &[]).unwrap().values())
                .totals
                .all_time_tokens,
            37
        );
        write(&path, &entries);
        let records = sync(&root, &[]).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, records.values())
                .totals
                .all_time_tokens,
            42
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn claude_stream_snapshots_count_cached_input_and_output_once() {
        let root = root();
        let path = root.join("claude/workspace/projects/project/session.jsonl");
        let now = Utc::now().timestamp_millis();
        let a = json!({"type":"assistant","timestamp":now,"cwd":"/project","requestId":"r1","message":{"id":"msg1","model":"claude","usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":2}}});
        let mut b = a.clone();
        b["timestamp"] = json!(now + 1);
        b["message"]["usage"]["output_tokens"] = json!(5);
        write(&path, &[a, b]);
        let records = sync(&root, &[]).unwrap();
        let snapshot = snapshot_from_records(30, None, records.values());
        assert_eq!(snapshot.totals.all_time_tokens, 65);
        assert_eq!(snapshot.days.last().unwrap().cached_input_tokens, 20);
        assert_eq!(snapshot.totals.all_time_agent_runs, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn codex_cumulative_reset_and_duplicate_totals_are_counted_once() {
        let root = root();
        let path = root.join("session.jsonl");
        let now = Utc::now().timestamp_millis();
        let entries: Vec<_> = [100, 100, 10, 20].iter().enumerate().map(|(i, n)| json!({"timestamp":now+i as i64,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":n,"output_tokens":0}}}})).collect();
        write(&path, &entries);
        let mut records = HashMap::new();
        scan_file(&path, &mut records).unwrap();
        assert_eq!(
            snapshot_from_records(30, None, records.values())
                .totals
                .all_time_tokens,
            120
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_ledger_is_never_overwritten() {
        let root = root();
        fs::create_dir_all(root.join("usage")).unwrap();
        let path = root.join("usage/ledger-v1.json");
        fs::write(&path, "broken").unwrap();
        assert!(sync(&root, &[])
            .unwrap_err()
            .contains("Invalid usage ledger"));
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simultaneous_refreshes_do_not_lose_or_duplicate_usage() {
        let root = root();
        write(
            &root.join("pi/sessions/w/thread.jsonl"),
            &session(Utc::now().timestamp_millis()),
        );
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || {
                    snapshot_from_records(30, None, sync(&root, &[]).unwrap().values())
                        .totals
                        .all_time_tokens
                })
            })
            .collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), 42);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
