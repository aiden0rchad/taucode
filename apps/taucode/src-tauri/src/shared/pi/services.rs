//! Thread-owned services are controlled exclusively through OMP's daemon CLI.
use super::super::{managed_tasks::{scan_urls, strip_ansi, OUTPUT_LIMIT}, providers::BoxFuture};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}, process::Stdio, time::{Duration, Instant, SystemTime}};
use tokio::sync::Mutex;

fn words(raw: &str) -> Vec<String> {
    let mut words = Vec::new();
    let (mut word, mut quoted, mut started) = (String::new(), false, false);
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => { quoted = !quoted; started = true; }
            '\\' if !quoted => { if let Some(c) = chars.next() { word.push(c); } started = true; }
            c if c.is_whitespace() && !quoted => {
                if started { words.push(std::mem::take(&mut word)); started = false; }
            }
            c => { word.push(c); started = true; }
        }
    }
    if started { words.push(word); }
    words
}
fn display_command(raw: &str) -> String {
    let words = words(raw);
    words.iter().position(|s| s.ends_with("commandGuard.ts"))
        .and_then(|i| words.get(i + 3)).cloned().unwrap_or_else(|| raw.into())
}
fn guard_owner(raw: &str) -> Option<u32> {
    let words = words(raw);
    words.iter().position(|s| s.ends_with("commandGuard.ts"))
        .and_then(|i| words.get(i + 1)).and_then(|s| s.parse().ok())
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('-') && !name.contains(['/', '\\', '\0']) && !name.contains("..")
}
fn project(report: &Value) -> Option<&Value> {
    report.as_array()?.iter().find(|scope| scope["kind"] == "project")
}
fn terminal(daemon: &Value) -> bool { matches!(daemon["state"].as_str(), Some("exited" | "failed")) }

// Pure mapping keeps broker metadata private; only the existing task shape crosses the UI boundary.
fn map(report: &Value, thread: &str, workspace: &str) -> Vec<Value> {
    let mut seen = HashSet::new();
    project(report).and_then(|scope| scope["daemons"].as_array()).into_iter().flatten()
        .filter(|d| d["owner"].as_str() == Some(thread))
        .filter(|d| d["name"].as_str().is_some_and(valid_name))
        .filter(|d| d["id"].as_str().is_some_and(|id| !id.is_empty() && seen.insert(id)))
        .map(|d| {
            let status = match d["state"].as_str() {
                // A hangup/interrupt/terminate signal is a deliberate stop (e.g. the guard ending it on app quit), not a crash.
                Some("failed") if d["exitCode"].as_i64().is_some_and(|code| matches!(code, 129 | 130 | 143)) => "stopped",
                Some("failed") => "failed",
                Some("exited") if d["exitCode"] == 0 => "completed",
                Some("exited") => "stopped",
                _ => "running",
            };
            json!({"id":format!("omp:{}", d["id"].as_str().unwrap()), "workspaceId":workspace, "threadId":thread,
                "label":d["name"], "command":display_command(d["command"].as_str().unwrap_or("")), "cwd":d["cwd"],
                "pid":if terminal(d) { Value::Null } else { d["pid"].clone() }, "status":status,
                "startedAt":d.get("startedAt").or_else(|| d.get("createdAt")).unwrap_or(&Value::Null),
                "endedAt":if terminal(d) { d["exitedAt"].clone() } else { Value::Null }, "exitCode":d["exitCode"],
                "output":"", "outputTruncated":false, "urls":[], "source":"omp"})
        }).collect()
}
fn snapshots(report: &Value, thread: &str, workspace: &str) -> Vec<Value> {
    let mut tasks = map(report, thread, workspace);
    let Some(runtime) = project(report).and_then(|scope| scope["runtimeDir"].as_str()).map(Path::new).filter(|p| p.is_absolute()) else { return tasks; };
    for task in &mut tasks {
        let path = runtime.join("daemons").join(task["label"].as_str().unwrap()).join("output.log");
        // Never inspect spec.json: it contains the service's credential environment.
        if !std::fs::symlink_metadata(path.parent().unwrap()).is_ok_and(|m| m.is_dir())
            || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) { continue; }
        let Ok(mut file) = std::fs::File::open(path) else { continue; };
        let Ok(meta) = file.metadata() else { continue; };
        if !meta.is_file() { continue; }
        let offset = meta.len().saturating_sub(OUTPUT_LIMIT as u64);
        if file.seek(SeekFrom::Start(offset)).is_err() { continue; }
        let mut bytes = Vec::new();
        if file.take(OUTPUT_LIMIT as u64).read_to_end(&mut bytes).is_err() { continue; }
        let mut output = strip_ansi(&String::from_utf8_lossy(&bytes));
        let mut boundary = output.len().saturating_sub(OUTPUT_LIMIT);
        while !output.is_char_boundary(boundary) { boundary += 1; }
        output.drain(..boundary);
        let mut urls = Vec::new();
        scan_urls(&mut urls, &output);
        task["output"] = json!(output);
        task["outputTruncated"] = json!(offset > 0 || boundary > 0);
        task["urls"] = json!(urls);
    }
    tasks
}

type Fingerprint = Vec<(PathBuf, Option<SystemTime>)>;
fn fingerprint(report: &Value) -> Option<Fingerprint> {
    let runtime = project(report)?["runtimeDir"].as_str()?;
    let mut entries: Vec<_> = std::fs::read_dir(Path::new(runtime).join("daemons")).ok()?
        .filter_map(Result::ok).map(|entry| {
            let path = entry.path().join("meta.json");
            let modified = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
            (path, modified)
        }).collect();
    entries.sort();
    Some(entries)
}
struct Cached { report: Value, at: Instant, fingerprint: Option<Fingerprint> }
#[derive(Default)]
pub(super) struct Services { cache: Mutex<HashMap<(String, String), Cached>> }

pub(super) trait Runner: Send + Sync {
    fn run<'a>(&'a self, args: &'a [&'a str]) -> BoxFuture<'a, Result<Value, String>>;
}
pub(super) struct Cli<'a> { pub data: &'a Path, pub workspace: &'a str, pub cwd: &'a str }
impl Runner for Cli<'_> {
    fn run<'a>(&'a self, args: &'a [&'a str]) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            let runtime = self.data.join("engines/omp/node_modules");
            let bun = runtime.join(".bin/bun");
            let cli = runtime.join("@oh-my-pi/pi-coding-agent/dist/cli.js");
            if !bun.is_file() || !cli.is_file() { return Ok(json!([])); }
            let home = self.data.join("omp/agent").join(self.workspace);
            let mut command = tokio::process::Command::new(bun);
            command.arg(cli).args(args).env_clear()
                .envs(super::super::engine_registry::environment(std::env::vars(), &home))
                .env("PI_CODING_AGENT_DIR", &home).env("PI_CONFIG_DIR", &home)
                .env("TAUCODE_OMP", "1").env("OTEL_SDK_DISABLED", "true")
                .current_dir(self.cwd).stdin(Stdio::null()).kill_on_drop(true);
            let output = tokio::time::timeout(Duration::from_secs(10), command.output()).await
                .map_err(|_| "OMP service request timed out")?.map_err(|_| "Could not run OMP service CLI")?;
            // CLI diagnostics may include commands or environment values. Do not expose them.
            if !output.status.success() || !output.stderr.is_empty() { return Err("OMP service CLI failed".into()); }
            serde_json::from_slice(&output.stdout).map_err(|_| "Invalid OMP service response".into())
        })
    }
}
impl Services {
    async fn report(&self, runner: &dyn Runner, workspace: &str, cwd: &str, fresh: bool, ttl: Duration) -> Result<Value, String> {
        let key = (workspace.to_owned(), cwd.to_owned());
        if !fresh {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&key) {
                let current = fingerprint(&cached.report);
                if project(&cached.report).is_some() && current.as_ref().is_none_or(Vec::is_empty) { return Ok(json!([])); }
                if cached.at.elapsed() < ttl && current == cached.fingerprint { return Ok(cached.report.clone()); }
            }
        }
        let report = runner.run(&["ps", "--json", "--dir", cwd]).await?;
        let entry = Cached { fingerprint: fingerprint(&report), report: report.clone(), at: Instant::now() };
        self.cache.lock().await.insert(key, entry);
        Ok(report)
    }
    pub(super) async fn request(&self, runner: &dyn Runner, workspace: &str, thread: &str, cwd: &str, action: &str, id: Option<&str>, engine_pid: Option<u32>) -> Result<Vec<Value>, String> {
        if !matches!(action, "list" | "stop" | "restart") { return Err("Unknown OMP service action".into()); }
        let report = self.report(runner, workspace, cwd, action != "list", Duration::from_secs(15)).await?;
        if action == "list" { return Ok(snapshots(&report, thread, workspace)); }
        let id = id.and_then(|id| id.strip_prefix("omp:")).ok_or("OMP service id is required")?;
        let daemon = project(&report).and_then(|scope| scope["daemons"].as_array()).into_iter().flatten()
            .find(|d| d["id"].as_str() == Some(id) && d["owner"].as_str() == Some(thread))
            .ok_or("OMP service not found in this session")?;
        let name = daemon["name"].as_str().filter(|name| valid_name(name)).ok_or("Invalid OMP service name")?;
        if action == "stop" && terminal(daemon) { return Err("OMP service is already stopped".into()); }
        if action == "restart" {
            if !terminal(daemon) { return Err("Stop the task before restarting it".into()); }
            if daemon["supervised"] != true || engine_pid.is_none() || guard_owner(daemon["command"].as_str().unwrap_or("")) != engine_pid {
                return Err("This service was started by an earlier agent session. Ask the agent to start it again.".into());
            }
        }
        let result = runner.run(&["ps", action, name, "--dir", cwd, "--json"]).await;
        self.cache.lock().await.remove(&(workspace.into(), cwd.into()));
        result?;
        let report = self.report(runner, workspace, cwd, true, Duration::from_secs(15)).await?;
        Ok(snapshots(&report, thread, workspace))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    struct Fixture { path: PathBuf, report: StdMutex<Value>, calls: StdMutex<Vec<Vec<String>>> }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("taucode-services-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(path.join("daemons/preview")).unwrap();
            std::fs::write(path.join("daemons/preview/meta.json"), "{}").unwrap();
            Self { report: StdMutex::new(json!([{"kind":"project", "runtimeDir":path, "daemons":[daemon("ready")]}])), path, calls: StdMutex::new(vec![]) }
        }
        fn set(&self, state: &str) { self.report.lock().unwrap()[0]["daemons"][0]["state"] = json!(state); }
        fn count(&self) -> usize { self.calls.lock().unwrap().len() }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.path); } }
    impl Runner for Fixture {
        fn run<'a>(&'a self, args: &'a [&'a str]) -> BoxFuture<'a, Result<Value, String>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(args.iter().map(|s| (*s).into()).collect());
                if args.get(1) == Some(&"stop") { self.set("exited"); }
                if args.get(1) == Some(&"restart") { self.set("ready"); }
                Ok(self.report.lock().unwrap().clone())
            })
        }
    }
    fn daemon(state: &str) -> Value {
        json!({"name":"preview", "id":"service", "owner":"thread", "state":state, "pid":69586,
            "createdAt":1793407978300i64, "startedAt":1793407978300i64, "exitedAt":1793408601983i64,
            "exitCode":1, "supervised":true, "cwd":"/project",
            "command":"/bin/bash -l -c '/x/bun/bin/bun.exe' '/x/agent/ws/extensions/lib/commandGuard.ts' '85089' '/bin/bash' 'node server.mjs'"})
    }
    fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap() }

    #[test]
    fn maps_only_owned_project_daemons_and_all_states() {
        for (state, status) in [("starting", "running"), ("running", "running"), ("ready", "running"), ("restarting", "running"), ("stopping", "running"), ("exited", "stopped"), ("failed", "failed")] {
            let owned = daemon(state);
            let mut foreign = owned.clone(); foreign["owner"] = json!("another-thread"); foreign["id"] = json!("foreign");
            let mut unowned = owned.clone(); unowned.as_object_mut().unwrap().remove("owner");
            let report = json!([{"kind":"global","daemons":[owned]}, {"kind":"project","daemons":[owned, foreign, unowned, owned]}]);
            let tasks = map(&report, "thread", "workspace");
            assert_eq!(tasks.len(), 1);
            let task = &tasks[0];
            assert_eq!(task["status"], status);
            assert_eq!(task["id"], "omp:service");
            assert_eq!(task["workspaceId"], "workspace");
            assert_eq!(task["threadId"], "thread");
            assert_eq!(task["source"], "omp");
            assert_eq!(task["startedAt"], 1793407978300i64);
            assert_eq!(task["pid"].is_null(), terminal(&owned));
            assert_eq!(task["endedAt"].is_null(), !terminal(&owned));
        }
        for (code, status) in [(json!(143), "stopped"), (json!(130), "stopped"), (json!(129), "stopped"), (json!(137), "failed"), (json!(1), "failed"), (Value::Null, "failed")] {
            let mut failed = daemon("failed"); failed["exitCode"] = code;
            assert_eq!(map(&json!([{"kind":"project","daemons":[failed]}]), "thread", "workspace")[0]["status"], status);
        }
        let mut completed = daemon("exited"); completed["exitCode"] = json!(0);
        assert_eq!(map(&json!([{"kind":"project","daemons":[completed]}]), "thread", "workspace")[0]["status"], "completed");
    }

    #[test]
    fn unwraps_shell_quotes_and_guard_pid() {
        let d = daemon("ready");
        let raw = d["command"].as_str().unwrap();
        assert_eq!(display_command(raw), "node server.mjs");
        assert_eq!(guard_owner(raw), Some(85089));
        let raw = raw.replace("'node server.mjs'", "'printf '\\''hello world'\\''' ");
        assert_eq!(display_command(&raw), "printf 'hello world'");
        assert_eq!(guard_owner(&raw), Some(85089));
        assert_eq!(display_command("node server.mjs"), "node server.mjs");
        assert_eq!(guard_owner("node server.mjs"), None);
        assert_eq!(display_command("'commandGuard.ts' '123'"), "'commandGuard.ts' '123'");
    }

    #[test]
    fn reads_bounded_log_and_local_urls_without_reading_specs_or_traversal() {
        let fixture = Fixture::new();
        std::fs::write(fixture.path.join("daemons/preview/spec.json"), r#"{"env":{"TOKEN":"secret-from-spec"}}"#).unwrap();
        let log = fixture.path.join("daemons/preview/output.log");
        std::fs::write(&log, "\x1b[32mFocus Board preview: http://127.0.0.1:4173\x1b[0m\nhttps://example.com").unwrap();
        let report = fixture.report.lock().unwrap().clone();
        let task = snapshots(&report, "thread", "workspace").remove(0);
        assert_eq!(task["urls"], json!(["http://127.0.0.1:4173/"]));
        assert!(!task["output"].as_str().unwrap().contains('\x1b'));
        assert_eq!(task["outputTruncated"], false);
        std::fs::write(&log, format!("{}\nhttp://127.0.0.1:4173", "✓".repeat(OUTPUT_LIMIT))).unwrap();
        let task = snapshots(&report, "thread", "workspace").remove(0);
        assert!(task["output"].as_str().unwrap().len() <= OUTPUT_LIMIT);
        assert_eq!(task["outputTruncated"], true);
        for name in ["../preview", "sub/preview", "sub\\preview", "a..b", "x\0y", "--all"] {
            let mut report = report.clone(); report[0]["daemons"][0]["name"] = json!(name);
            assert!(snapshots(&report, "thread", "workspace").is_empty());
        }
        #[cfg(unix)] {
            std::fs::remove_file(&log).unwrap();
            std::os::unix::fs::symlink("spec.json", &log).unwrap();
            assert_eq!(snapshots(&report, "thread", "workspace")[0]["output"], "");
        }
    }

    #[test]
    fn cache_tracks_metadata_ttl_and_empty_directories() { runtime().block_on(async {
        let fixture = Fixture::new();
        let services = Services::default();
        for _ in 0..3 {
            let tasks = services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0]["status"], "running");
        }
        assert_eq!(fixture.count(), 1);
        fixture.set("exited");
        let meta = fixture.path.join("daemons/preview/meta.json");
        let file = std::fs::File::options().write(true).open(meta).unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(2)).unwrap();
        let tasks = services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap();
        assert_eq!(tasks[0]["status"], "stopped");
        assert_eq!(fixture.count(), 2);
        services.report(&fixture, "workspace", "/project", false, Duration::ZERO).await.unwrap();
        assert_eq!(fixture.count(), 3);
        std::fs::remove_dir_all(fixture.path.join("daemons/preview")).unwrap();
        assert!(services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap().is_empty());
        std::fs::remove_dir(fixture.path.join("daemons")).unwrap();
        assert!(services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap().is_empty());
        assert_eq!(fixture.count(), 3);
        std::fs::create_dir_all(fixture.path.join("daemons/preview")).unwrap();
        std::fs::write(fixture.path.join("daemons/preview/meta.json"), "{}").unwrap();
        fixture.set("ready");
        assert_eq!(services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap()[0]["status"], "running");
        assert_eq!(fixture.count(), 4);
    }); }

    #[test]
    fn actions_require_fresh_ownership_and_live_original_engine() { runtime().block_on(async {
        let fixture = Fixture::new();
        let services = Services::default();
        for (thread, id) in [("foreign", "omp:service"), ("thread", "omp:unknown")] {
            assert!(services.request(&fixture, "workspace", thread, "/project", "stop", Some(id), None).await.is_err());
        }
        assert!(services.request(&fixture, "workspace", "thread", "/project", "restart", Some("omp:service"), Some(85089)).await.unwrap_err().contains("Stop the task"));
        assert!(fixture.calls.lock().unwrap().iter().all(|args| args[1] == "--json"));
        let tasks = services.request(&fixture, "workspace", "thread", "/project", "stop", Some("omp:service"), None).await.unwrap();
        assert_eq!(tasks[0]["status"], "stopped");
        assert_eq!(fixture.calls.lock().unwrap().iter().filter(|args| args[1] == "stop").cloned().collect::<Vec<_>>(), vec![vec!["ps", "stop", "preview", "--dir", "/project", "--json"]]);
        for pid in [None, Some(1)] {
            assert!(services.request(&fixture, "workspace", "thread", "/project", "restart", Some("omp:service"), pid).await.unwrap_err().contains("earlier agent session"));
        }
        fixture.report.lock().unwrap()[0]["daemons"][0]["supervised"] = json!(false);
        assert!(services.request(&fixture, "workspace", "thread", "/project", "restart", Some("omp:service"), Some(85089)).await.is_err());
        assert!(!fixture.calls.lock().unwrap().iter().any(|args| args[1] == "restart"));
        fixture.report.lock().unwrap()[0]["daemons"][0]["supervised"] = json!(true);
        assert_eq!(services.request(&fixture, "workspace", "thread", "/project", "restart", Some("omp:service"), Some(85089)).await.unwrap()[0]["status"], "running");
        let calls = fixture.count();
        services.request(&fixture, "workspace", "thread", "/project", "list", None, None).await.unwrap();
        assert_eq!(fixture.count(), calls);
        assert!(services.request(&fixture, "workspace", "thread", "/project", "unknown", None, None).await.is_err());
    }); }

    #[test]
    fn missing_runtime_lists_no_services() { runtime().block_on(async {
        let fixture = Fixture::new();
        let cli = Cli { data: &fixture.path, workspace: "workspace", cwd: "/project" };
        assert_eq!(cli.run(&["ps", "--json", "--dir", "/project"]).await.unwrap(), json!([]));
    }); }

    #[test]
    fn dispatch_and_owner_routing_do_not_start_an_engine_or_write_configuration() { runtime().block_on(async {
        use super::super::{PiEngine, Session};
        use crate::shared::engines::WorkspaceSession;
        use std::sync::Arc;
        let fixture = Fixture::new();
        let mut engine = PiEngine::new(fixture.path.clone(), "workspace".into(), "/project".into(), None, Arc::new(|_, _| {}));
        engine.profile.definition.id = "omp".into();
        engine.sessions.lock().await.insert("thread".into(), Session {
            workspace: "workspace".into(), thread: json!({"id":"thread","cwd":"/project"}),
            file: fixture.path.join("missing.jsonl"), process: None, configuration: String::new(),
        });
        // A normal dispatch would write policy here; listing must leave it alone.
        std::fs::create_dir_all(fixture.path.join("pi/agent/workspace")).unwrap();
        let session = WorkspaceSession::lazy(Box::new(|_| Box::pin(async { Err("must not start codex".into()) })), vec![], Default::default(), None, None, "workspace".into());
        let engine = Arc::new(engine);
        session.extra_pi.lock().await.insert("omp".into(), engine.clone());
        for owned in [false, true] {
            if owned { session.owners.lock().await.insert("thread".into(), "omp".into()); }
            assert_eq!(session.engine_services("workspace", "thread", "list", json!({})).await.unwrap(), Some(json!([])));
        }
        assert_eq!(session.engine_services("workspace", "unknown", "list", json!({})).await.unwrap(), None);
        assert!(engine.sessions.lock().await["thread"].process.is_none());
        assert_eq!(std::fs::read_dir(fixture.path.join("pi/agent/workspace")).unwrap().count(), 0);
        assert!(!fixture.path.join("omp/agent").exists());
        // After relaunch nothing is in memory yet; a saved session still routes the first listing.
        let saved = "05815c65-334c-4f6a-b300-fde97c50f12b";
        assert_eq!(session.engine_services("workspace", saved, "list", json!({})).await.unwrap(), None);
        std::fs::create_dir_all(fixture.path.join("omp/sessions/workspace")).unwrap();
        std::fs::write(fixture.path.join(format!("omp/sessions/workspace/{saved}.jsonl")),
            format!("{}\n", json!({"type":"session","version":3,"engine":"omp","id":saved,"timestamp":"2026-10-31T00:00:00Z","cwd":"/project"}))).unwrap();
        assert_eq!(session.engine_services("workspace", saved, "list", json!({})).await.unwrap(), Some(json!([])));
        assert!(engine.sessions.lock().await[saved].process.is_none());
        assert!(!engine.has_session("workspace", "../workspace/thread").await);
    }); }
}
