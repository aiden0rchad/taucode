//! Local journal format shared by the app and standalone reader. No network operations.
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const REDACTION: &str = include_str!("../../../../../docs/fixtures/diagnostics-redaction.json");
pub const CAP: u64 = 50 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub level: String,
    pub retention: u32,
    pub include_content_snippets: bool,
    pub expose_to_agents: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            level: "standard".into(),
            retention: 7,
            include_content_snippets: false,
            expose_to_agents: false,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=30).contains(&self.retention)
            || !["essential", "standard", "verbose"].contains(&self.level.as_str())
        {
            return Err("Invalid diagnostics settings".into());
        }
        Ok(())
    }
    pub fn accepts(&self, kind: &str) -> bool {
        match self.level.as_str() {
            "essential" => matches!(kind, "error" | "feedback"),
            "verbose" => true,
            _ => kind != "ux",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    #[serde(default = "version")]
    pub v: u8,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub run: String,
    #[serde(default)]
    pub seq: u64,
    pub kind: String,
    pub severity: String,
    pub area: String,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default = "object")]
    pub context: Value,
    #[serde(default = "object")]
    pub metrics: Value,
    #[serde(default = "object")]
    pub detail: Value,
}
fn version() -> u8 {
    1
}
fn object() -> Value {
    json!({})
}
pub fn event(kind: &str, area: &str, code: &str, message: &str) -> Event {
    Event {
        v: 1,
        ts: String::new(),
        run: String::new(),
        seq: 0,
        kind: kind.into(),
        severity: if kind == "error" { "high" } else { "low" }.into(),
        area: area.into(),
        code: code.into(),
        message: message.into(),
        fingerprint: String::new(),
        context: object(),
        metrics: object(),
        detail: object(),
    }
}
pub fn redact(text: &str, home: &str, secrets: &[String]) -> String {
    static RULES: OnceLock<Vec<(Regex, String)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        serde_json::from_str::<Value>(REDACTION).unwrap()["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    Regex::new(r[0].as_str().unwrap()).unwrap(),
                    r[1].as_str().unwrap().into(),
                )
            })
            .collect()
    });
    let mut text = text.to_owned();
    for key in secrets.iter().filter(|s| !s.is_empty()) {
        text = text.replace(key, "[REDACTED]");
    }
    if !home.is_empty() {
        text = text.replace(home, "~");
    }
    for (pattern, replacement) in rules {
        text = pattern
            .replace_all(&text, replacement.as_str())
            .into_owned();
    }
    text
}
pub fn fingerprint(e: &Event) -> String {
    static NORMAL: OnceLock<Regex> = OnceLock::new();
    let re = NORMAL.get_or_init(|| {
        Regex::new(r"(?:~|/|[A-Za-z]:\\)[^\s]+|\b[0-9a-fA-F-]{8,}\b|\b\w*\d\w*\b").unwrap()
    });
    let frames = e.detail["stack"]
        .as_str()
        .unwrap_or("")
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");
    let text = format!(
        "{}|{}|{}|{}",
        e.code,
        e.area,
        re.replace_all(&e.message, "#"),
        re.replace_all(&frames, "#")
    );
    format!("{:x}", Sha256::digest(text.as_bytes()))[..16].into()
}
pub fn sanitize(mut e: Event, options: &Options, home: &str, secrets: &[String]) -> Option<Event> {
    if !options.accepts(&e.kind)
        || ![
            "error",
            "warning",
            "perf",
            "usage",
            "ux",
            "feedback",
            "suggestion",
        ]
        .contains(&e.kind.as_str())
        || !["low", "medium", "high", "critical"].contains(&e.severity.as_str())
        || ![
            "engine",
            "provider",
            "ui",
            "ipc",
            "git",
            "browser",
            "computer",
            "terminal",
            "checkpoints",
            "connectors",
            "themes",
            "work",
            "chat",
            "startup",
            "agent",
        ]
        .contains(&e.area.as_str())
        || e.code.len() > 100
        || e.code.is_empty()
        || !e
            .code
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._".contains(&c))
    {
        return None;
    }
    for (value, allowed) in [
        (
            &mut e.context,
            &[
                "appVersion",
                "os",
                "mode",
                "panel",
                "engine",
                "providerKind",
                "model",
                "threadId",
                "turnId",
                "workspace",
                "codegraph",
                "responseId",
            ][..],
        ),
        (
            &mut e.metrics,
            &[
                "durationMs",
                "tokensIn",
                "tokensOut",
                "cachedTokens",
                "costUsd",
                "count",
                "toolCalls",
                "errors",
                "rssKb",
                "cpuPercent",
            ][..],
        ),
        (
            &mut e.detail,
            &[
                "stack",
                "tool",
                "errorClass",
                "command",
                "status",
                "note",
                "type",
                "relatedFingerprint",
                "attachedEvents",
                "stderrTail",
            ][..],
        ),
    ] {
        let mut clean = serde_json::Map::new();
        for key in allowed {
            if let Some(v) = value.get(*key) {
                if *key == "stderrTail" && !options.include_content_snippets {
                    continue;
                }
                if let Some(s) = v.as_str() {
                    clean.insert(
                        (*key).into(),
                        json!(redact(s, home, secrets)
                            .chars()
                            .take(if *key == "stack" { 2000 } else { 300 })
                            .collect::<String>()),
                    );
                } else if v.is_number() || v.is_boolean() {
                    clean.insert((*key).into(), v.clone());
                }
            }
        }
        *value = Value::Object(clean);
    }
    e.metrics
        .as_object_mut()
        .unwrap()
        .retain(|_, v| v.as_f64().is_some_and(|n| n >= 0.0));
    e.context.as_object_mut().unwrap().retain(|key, v| {
        if key == "codegraph" {
            v.is_boolean()
        } else {
            v.is_string()
        }
    });
    e.message = redact(&e.message, home, secrets)
        .chars()
        .take(300)
        .collect();
    e.fingerprint = fingerprint(&e);
    e.v = 1;
    Some(e)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub fingerprint: String,
    pub code: String,
    pub area: String,
    pub severity: String,
    pub title: String,
    pub count: u64,
    pub first_seen: String,
    pub last_seen: String,
    pub last_event: Event,
    pub engines: BTreeSet<String>,
    pub models: BTreeSet<String>,
    pub status: String,
    pub notes: Vec<String>,
    pub regressed: bool,
}
pub type Issues = BTreeMap<String, Issue>;
pub fn severity(s: &str) -> u64 {
    match s {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}
pub fn aggregate(issues: &mut Issues, events: &[Event]) {
    for e in events {
        let i = issues
            .entry(e.fingerprint.clone())
            .or_insert_with(|| Issue {
                fingerprint: e.fingerprint.clone(),
                code: e.code.clone(),
                area: e.area.clone(),
                severity: e.severity.clone(),
                title: e.message.clone(),
                count: 0,
                first_seen: e.ts.clone(),
                last_seen: e.ts.clone(),
                last_event: e.clone(),
                engines: BTreeSet::new(),
                models: BTreeSet::new(),
                status: "open".into(),
                notes: vec![],
                regressed: false,
            });
        i.count += 1;
        if e.ts < i.first_seen {
            i.first_seen = e.ts.clone();
        }
        if e.ts >= i.last_seen {
            i.last_seen = e.ts.clone();
            i.last_event = e.clone();
        }
        if severity(&e.severity) > severity(&i.severity) {
            i.severity = e.severity.clone();
        }
        if i.status == "fixed" {
            i.status = "open".into();
            i.regressed = true;
        }
        if let Some(v) = e.context["engine"].as_str() {
            i.engines.insert(v.into());
        }
        if let Some(v) = e.context["model"].as_str() {
            i.models.insert(v.into());
        }
    }
}
pub fn lines<T: serde::de::DeserializeOwned>(path: &Path) -> Vec<T> {
    fs::File::open(path)
        .map(|f| {
            io::BufReader::new(f)
                .lines()
                .map_while(Result::ok)
                .filter_map(|s| serde_json::from_str(&s).ok())
                .collect()
        })
        .unwrap_or_default()
}
pub fn event_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("events-") && n.ends_with(".ndjson"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}
pub fn events(dir: &Path) -> Vec<Event> {
    event_files(dir).iter().flat_map(|p| lines(p)).collect()
}
pub fn issues(dir: &Path) -> Issues {
    fs::read(dir.join("issues.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
pub fn atomic(dir: &Path, name: &str, data: &[u8]) -> io::Result<()> {
    let temp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, data)?;
    fs::rename(temp, dir.join(name))
}
pub fn append(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    let len = f.metadata()?.len();
    if len > 0 {
        f.seek(SeekFrom::End(-1))?;
        let mut b = [0];
        f.read_exact(&mut b)?;
        if b[0] != b'\n' {
            let mut pos = len;
            while pos > 0 {
                pos -= 1;
                f.seek(SeekFrom::Start(pos))?;
                f.read_exact(&mut b)?;
                if b[0] == b'\n' {
                    pos += 1;
                    break;
                }
            }
            f.set_len(pos)?;
        }
    }
    f.write_all(data)?;
    f.flush()
}
pub fn annotate(
    dir: &Path,
    fingerprint: &str,
    status: Option<&str>,
    note: &str,
) -> Result<(), String> {
    if fingerprint.len() != 16 || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid fingerprint".into());
    }
    if status.is_some_and(|s| !["open", "acknowledged", "fixed", "ignored"].contains(&s)) {
        return Err("Invalid status".into());
    }
    if !issues(dir).contains_key(fingerprint) {
        return Err("Unknown issue".into());
    }
    let note = crate::writer::redact_known(note)
        .chars()
        .take(300)
        .collect::<String>();
    let line = format!(
        "{}\n",
        json!({"id":uuid::Uuid::new_v4().to_string(),"ts":Utc::now().to_rfc3339(),"fingerprint":fingerprint,"status":status,"note":note})
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("annotations.ndjson"))
        .map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes()).map_err(|e| e.to_string())
}
pub fn merge_annotations(issues: &mut Issues, dir: &Path, seen: &mut BTreeSet<String>) {
    for a in lines::<Value>(&dir.join("annotations.ndjson")) {
        let Some(id) = a["id"].as_str() else { continue };
        if !seen.insert(id.into()) {
            continue;
        }
        if let Some(i) = a["fingerprint"].as_str().and_then(|f| issues.get_mut(f)) {
            if let Some(s) = a["status"]
                .as_str()
                .filter(|s| ["open", "acknowledged", "fixed", "ignored"].contains(s))
            {
                i.status = s.into();
                if s == "fixed" {
                    i.regressed = false;
                }
            }
            if let Some(n) = a["note"].as_str().filter(|s| !s.is_empty()) {
                i.notes.push(
                    redact(n, &std::env::var("HOME").unwrap_or_default(), &[])
                        .chars()
                        .take(300)
                        .collect(),
                );
            }
        }
    }
}
pub fn retain(dir: &Path, days: u32, cap: u64) -> io::Result<()> {
    let cutoff = (Utc::now() - chrono::Duration::days(days as i64))
        .format("%Y-%m-%d")
        .to_string();
    let mut kept = vec![];
    let mut total = 0;
    for p in event_files(dir) {
        let name = p.file_name().unwrap().to_string_lossy();
        let size = fs::metadata(&p)?.len();
        if name.get(7..17).is_some_and(|d| d < cutoff.as_str()) {
            fs::remove_file(p)?;
        } else {
            total += size;
            kept.push((p, size));
        }
    }
    for (p, size) in kept {
        if total <= cap {
            break;
        }
        fs::remove_file(p)?;
        total -= size;
    }
    Ok(())
}
pub fn default_dir() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default());
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    base.join("dev.taucode.desktop/diagnostics")
}
#[path = "digest.rs"]
mod digest;
pub use digest::digest;
#[path = "signals.rs"]
pub mod signals;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
