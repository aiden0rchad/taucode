//! Local, offline ingestion. Source roots are only opened by explicit detection;
//! parsers and mutations run on blocking workers, never on the UI executor.
mod io;
mod parse;
#[cfg(test)]
mod tests;

use crate::{
    storage,
    types::{AppSettings, WorkspaceEntry},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone)]
pub(crate) struct Roots {
    codex: PathBuf,
    claude: PathBuf,
    desktop: PathBuf,
}
impl Roots {
    pub fn local() -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        Self {
            codex: std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex")),
            claude: home.join(".claude"),
            desktop: home.join("Library/Application Support/Claude"),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    key: String,
    source: String,
    id: String,
    kind: String,
    title: String,
    project: String,
    cwd: String,
    archived: bool,
    missing_cwd: bool,
    needs_secrets: bool,
    warning: Option<String>,
    #[serde(skip)]
    payload: Value,
    #[serde(skip)]
    messages: Vec<Value>,
    #[serde(skip)]
    attachments: Vec<(String, PathBuf)>,
}
impl Entry {
    fn new(source: &str, id: &str, kind: &str, title: &str, project: &str, cwd: &str) -> Self {
        Self {
            key: format!(
                "{}:{id}",
                if source == "claude-cli" {
                    "claude"
                } else {
                    source
                }
            ),
            source: source.into(),
            id: id.into(),
            kind: kind.into(),
            title: title.into(),
            project: project.into(),
            cwd: cwd.into(),
            archived: false,
            missing_cwd: !cwd.is_empty() && !Path::new(cwd).is_dir(),
            needs_secrets: false,
            warning: None,
            payload: Value::Null,
            messages: vec![],
            attachments: vec![],
        }
    }
    fn failed(source: &str, path: &Path, error: &str) -> Self {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut e = Self::new(source, &name, "skipped", &name, "Skipped files", "");
        e.warning = Some(error.into());
        e
    }
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    key: String,
    title: String,
    kind: String,
    thread_id: String,
    workspace_id: Option<String>,
    files: HashMap<String, String>,
    workspace: Option<Value>,
    connector: Option<Value>,
    #[serde(default)]
    continued: bool,
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Log {
    id: String,
    created: Vec<Receipt>,
    skipped: Vec<String>,
    warnings: Vec<String>,
    cancelled: bool,
    undone: bool,
}

pub(crate) struct Imports {
    data: PathBuf,
    roots: Roots,
    operation: tokio::sync::Mutex<()>,
    cancel: AtomicBool,
    catalog: Mutex<Vec<Entry>>,
    progress: Mutex<Value>,
}
impl Imports {
    pub fn new(data: PathBuf) -> Arc<Self> {
        Self::with_roots(data, Roots::local())
    }
    fn with_roots(data: PathBuf, roots: Roots) -> Arc<Self> {
        Arc::new(Self {
            data,
            roots,
            operation: tokio::sync::Mutex::new(()),
            cancel: AtomicBool::new(false),
            catalog: Mutex::new(vec![]),
            progress: Mutex::new(json!({"running":false,"completed":0,"total":0})),
        })
    }
    // Shared by app and daemon. The caller holds registry locks across mutations,
    // preventing an import from overwriting simultaneous workspace/settings edits.
    pub async fn request(
        self: &Arc<Self>,
        action: &str,
        params: Value,
        workspaces: &tokio::sync::Mutex<HashMap<String, WorkspaceEntry>>,
        settings: &tokio::sync::Mutex<AppSettings>,
    ) -> Result<Value, String> {
        if action == "status" {
            return Ok(self.progress.lock().unwrap().clone());
        }
        if action == "cancel" {
            self.cancel.store(true, Ordering::Relaxed);
            return Ok(json!({}));
        }
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "An import operation is already running")?;
        self.cancel.store(false, Ordering::Relaxed);
        *self.progress.lock().unwrap() =
            json!({"running":true,"completed":0,"total":0,"phase":action});
        let mut workspaces = workspaces.lock().await;
        let mut settings = settings.lock().await;
        let service = self.clone();
        let action = action.to_owned();
        let ws = workspaces.clone();
        let app = settings.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut ws = ws;
            let mut app = app;
            let result = service.perform(&action, params, &mut ws, &mut app);
            (result, ws, app)
        })
        .await
        .map_err(|e| e.to_string());
        self.progress.lock().unwrap()["running"] = json!(false);
        let (result, ws, app) = result?;
        *workspaces = ws;
        *settings = app;
        result
    }
    fn perform(
        &self,
        action: &str,
        params: Value,
        workspaces: &mut HashMap<String, WorkspaceEntry>,
        settings: &mut AppSettings,
    ) -> Result<Value, String> {
        match action {
            "detect" | "export" => {
                let (entries, warnings) = if action == "export" {
                    (
                        parse::export_file(
                            Path::new(params["path"].as_str().ok_or("Missing export path")?),
                            &self.cancel,
                        )?,
                        vec![],
                    )
                } else {
                    parse::detect(&self.roots, &self.cancel)?
                };
                let response = json!({"entries":entries,"warnings":warnings});
                *self.catalog.lock().unwrap() = entries;
                Ok(response)
            }
            "run" => self.run(params, workspaces, settings),
            "logs" => {
                let mut logs = vec![];
                let root = self.data.join("imports");
                if root.exists() {
                    for file in fs::read_dir(root).map_err(|e| e.to_string())? {
                        let path = file.map_err(|e| e.to_string())?.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("json")
                            && path.file_stem().and_then(|s| s.to_str()) != Some("index")
                        {
                            logs.push(
                                serde_json::to_value(
                                    self.log(
                                        path.file_stem()
                                            .unwrap()
                                            .to_str()
                                            .ok_or("Invalid log name")?,
                                    )?,
                                )
                                .map_err(|e| e.to_string())?,
                            );
                        }
                    }
                }
                logs.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
                Ok(json!(logs))
            }
            "undo" => self.undo(params, workspaces, settings),
            "read" | "continue" => {
                let mut log = self.log(params["logId"].as_str().ok_or("Missing log ID")?)?;
                let receipt = log
                    .created
                    .iter_mut()
                    .find(|r| r.key == params["key"].as_str().unwrap_or_default())
                    .ok_or("Unknown imported item")?;
                let history = self
                    .data
                    .join("imports/history")
                    .join(format!("{}.json", receipt.thread_id));
                let value = storage::read_thread_metadata(&history)?;
                if action == "read" {
                    return Ok(value);
                }
                let messages = value["messages"]
                    .as_array()
                    .ok_or("No transcript to continue")?;
                let id = uuid::Uuid::new_v4().to_string();
                let context = continuation(messages);
                let entry = Entry::new(
                    "history",
                    &id,
                    "chat",
                    &format!("Continue: {}", receipt.title),
                    "Chats",
                    "",
                );
                let bytes = pi_session(
                    &entry,
                    &id,
                    &self.data.join("chats/scratch").join(&id),
                    &[json!({"role":"user","text":context})],
                    settings,
                );
                let path = self
                    .data
                    .join("chats/sessions/chats")
                    .join(format!("{id}.jsonl"));
                write_new(&path, &bytes)?;
                receipt.continued = true;
                self.save_log(&log)?;
                Ok(json!({"threadId":id,"mode":"chat"}))
            }
            _ => Err("Unknown import action".into()),
        }
    }
    fn index(&self) -> Result<Value, String> {
        let mut index = storage::read_thread_metadata(&self.data.join("imports/index.json"))?;
        // A process can exit after journaling files but before updating the index.
        // Recover those ownership claims; Undo can clear an interrupted import.
        if let Ok(files) = fs::read_dir(self.data.join("imports")) {
            for file in files {
                let path = file.map_err(|e| e.to_string())?.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json")
                    || path.file_stem().and_then(|s| s.to_str()) == Some("index")
                {
                    continue;
                }
                let log = self.log(
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or("Invalid log name")?,
                )?;
                if !log.undone {
                    for receipt in log.created {
                        if index.get(&receipt.key).is_none() {
                            index[&receipt.key] = json!(log.id);
                        }
                    }
                }
            }
        }
        Ok(index)
    }
    fn log(&self, id: &str) -> Result<Log, String> {
        if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err("Invalid import log ID".into());
        }
        serde_json::from_value(storage::read_thread_metadata(
            &self.data.join("imports").join(format!("{id}.json")),
        )?)
        .map_err(|e| e.to_string())
    }
    fn save_log(&self, log: &Log) -> Result<(), String> {
        storage::write_thread_metadata(
            &self.data.join("imports").join(format!("{}.json", log.id)),
            &json!(log),
        )
    }
    fn persist_registry(
        &self,
        workspaces: &HashMap<String, WorkspaceEntry>,
        settings: &AppSettings,
    ) -> Result<(), String> {
        fs::create_dir_all(&self.data).map_err(|e| e.to_string())?;
        storage::write_thread_metadata(
            &self.data.join("workspaces.json"),
            &json!(workspaces.values().collect::<Vec<_>>()),
        )?;
        storage::write_thread_metadata(&self.data.join("settings.json"), &json!(settings))
    }
    fn run(
        &self,
        params: Value,
        workspaces: &mut HashMap<String, WorkspaceEntry>,
        settings: &mut AppSettings,
    ) -> Result<Value, String> {
        let selected: HashSet<_> = params["keys"]
            .as_array()
            .ok_or("Choose items to import")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let catalog = self.catalog.lock().unwrap().clone();
        if selected
            .iter()
            .any(|key| !catalog.iter().any(|e| &e.key == key))
        {
            return Err("Selection expired; detect again".into());
        }
        let mut index = self.index()?;
        let mut log = Log {
            id: format!(
                "{}-{}",
                chrono::Utc::now().format("%Y%m%dT%H%M%S"),
                uuid::Uuid::new_v4()
            ),
            ..Log::default()
        };
        self.save_log(&log)?;
        let entries: Vec<_> = catalog
            .into_iter()
            .filter(|e| selected.contains(e.key.as_str()))
            .collect();
        *self.progress.lock().unwrap() =
            json!({"running":true,"phase":"run","completed":0,"total":entries.len()});
        for (n, entry) in entries.iter().enumerate() {
            if io::check(&self.cancel).is_err() {
                log.cancelled = true;
                break;
            }
            if index.get(&entry.key).is_some() {
                log.skipped
                    .push(format!("{}: already imported", entry.title));
            } else if entry.kind == "skipped" {
                log.skipped.push(format!(
                    "{}: {}",
                    entry.title,
                    entry.warning.as_deref().unwrap_or("Unavailable")
                ));
            } else {
                match self.prepare(entry, workspaces, settings) {
                    Ok((receipt, files)) => {
                        // Journal ownership before the first write, so a crash remains undoable.
                        log.created.push(receipt.clone());
                        self.save_log(&log)?;
                        let previous_workspaces = workspaces.clone();
                        let previous_settings = settings.clone();
                        let mut written = vec![];
                        let result = (|| {
                            for (path, bytes) in files {
                                io::check(&self.cancel)?;
                                let path = self.data.join(path);
                                write_new(&path, &bytes)?;
                                written.push(path);
                            }
                            if let Some(ws) = &receipt.workspace {
                                let ws: WorkspaceEntry = serde_json::from_value(ws.clone())
                                    .map_err(|e| e.to_string())?;
                                workspaces.insert(ws.id.clone(), ws);
                            }
                            if let Some(connector) = &receipt.connector {
                                settings.mcp_servers.push(
                                    serde_json::from_value(connector.clone())
                                        .map_err(|e| e.to_string())?,
                                );
                            }
                            self.persist_registry(workspaces, settings)?;
                            Ok::<_, String>(())
                        })();
                        if let Err(e) = result {
                            *workspaces = previous_workspaces;
                            *settings = previous_settings;
                            self.persist_registry(workspaces, settings)?;
                            for path in written {
                                fs::remove_file(path).map_err(|e| e.to_string())?;
                            }
                            log.created.pop();
                            log.skipped.push(format!("{}: {e}", entry.title));
                        } else {
                            index[&entry.key] = json!(log.id);
                            storage::write_thread_metadata(
                                &self.data.join("imports/index.json"),
                                &index,
                            )?;
                        }
                    }
                    Err(e) => log.skipped.push(format!("{}: {e}", entry.title)),
                }
            }
            self.progress.lock().unwrap()["completed"] = json!(n + 1);
            self.save_log(&log)?;
        }
        log.cancelled |= self.cancel.load(Ordering::Relaxed);
        self.save_log(&log)?;
        Ok(json!(log))
    }
    fn prepare(
        &self,
        e: &Entry,
        workspaces: &HashMap<String, WorkspaceEntry>,
        settings: &AppSettings,
    ) -> Result<(Receipt, Vec<(PathBuf, Vec<u8>)>), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut receipt = Receipt {
            key: e.key.clone(),
            title: e.title.clone(),
            kind: e.kind.clone(),
            thread_id: id.clone(),
            ..Receipt::default()
        };
        let mut files = vec![];
        if e.kind == "connector" {
            let connector = json!({"id":format!("import-{}", &id[..8]),"name":e.title,"enabled":false,"transport":e.payload,"approval":"ask","toolAllowlist":null,"toolDenylist":null});
            let parsed = serde_json::from_value(connector.clone()).map_err(|e| e.to_string())?;
            crate::shared::connectors::validate(&[parsed], false)?;
            receipt.connector = Some(connector);
        } else {
            let cwd_exists = !e.cwd.is_empty() && Path::new(&e.cwd).is_dir();
            if e.kind == "project" && !cwd_exists {
                return Err(format!("Missing root: {}", e.cwd));
            }
            if cwd_exists && ["project", "thread"].contains(&e.kind.as_str()) {
                let existing = workspaces.values().find(|w| w.path == e.cwd);
                let ws_id = existing
                    .map(|w| w.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if existing.is_none() {
                    let ws: WorkspaceEntry = serde_json::from_value(json!({"id":ws_id,"name":if e.kind == "project" {&e.title} else {&e.project},"path":e.cwd,"settings":{}})).map_err(|e| e.to_string())?;
                    receipt.workspace = Some(json!(ws));
                }
                receipt.workspace_id = Some(ws_id);
            }
            let native = e.kind == "thread"
                && cwd_exists
                && e.payload.is_array()
                && uuid::Uuid::parse_str(&e.id).is_ok();
            if native && e.source == "codex" {
                for tree in ["sessions", "archived_sessions"] {
                    if io::files(
                        &self.data.join("engines/codex-home").join(tree),
                        4,
                        &self.cancel,
                    )?
                    .iter()
                    .any(|p| {
                        p.file_name()
                            .and_then(|p| p.to_str())
                            .is_some_and(|p| p.ends_with(&format!("-{}.jsonl", e.id)))
                    }) {
                        return Err("Native Codex session already exists".into());
                    }
                }
                receipt.thread_id = e.id.clone();
                let tree = if e.archived {
                    "archived_sessions"
                } else {
                    "sessions"
                };
                let time = e
                    .payload
                    .as_array()
                    .unwrap()
                    .iter()
                    .find_map(|r| {
                        r["timestamp"]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    })
                    .map(|t| t.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now);
                let path = PathBuf::from("engines/codex-home")
                    .join(tree)
                    .join(time.format("%Y/%m/%d").to_string())
                    .join(format!(
                        "rollout-{}-{}.jsonl",
                        time.format("%Y-%m-%dT%H-%M-%S"),
                        e.id
                    ));
                files.push((path, io::lines(e.payload.as_array().unwrap())));
                // Retain title/archival metadata alongside the original native ID.
                files.push((
                    PathBuf::from("thread-metadata")
                        .join(receipt.workspace_id.as_ref().unwrap())
                        .join(format!("{}.json", e.id)),
                    serde_json::to_vec(&json!({"importedName":e.title})).unwrap(),
                ));
            } else if native && e.source.starts_with("claude") {
                let home = PathBuf::from("claude").join(receipt.workspace_id.as_ref().unwrap());
                let encoded = e.cwd.replace(['/', '.', '\\'], "-");
                files.push((
                    home.join("projects")
                        .join(encoded)
                        .join(format!("{}.jsonl", e.id)),
                    io::lines(e.payload.as_array().unwrap()),
                ));
                let metadata = json!({"id":id,"sessionId":e.id,"engine":"claude","cwd":e.cwd,"name":e.title,"preview":e.messages.first().map(|m| m["text"].clone()),"archived":e.archived,"createdAt":chrono::Utc::now().timestamp(),"updatedAt":chrono::Utc::now().timestamp(),"accessMode":"ask","status":{"type":"idle"},"turns":v2_turns(&e.messages)});
                files.push((
                    home.join("threads").join(format!("{id}.json")),
                    serde_json::to_vec(&metadata).unwrap(),
                ));
            } else if e.kind == "chat" {
                files.push((
                    PathBuf::from("chats/sessions/chats").join(format!("{id}.jsonl")),
                    pi_session(
                        e,
                        &id,
                        &self.data.join("chats/scratch").join(&id),
                        &e.messages,
                        settings,
                    ),
                ));
            }
            if e.kind != "project" {
                let history = json!({"title":e.title,"source":e.source,"kind":e.kind,"cwd":e.cwd,"readOnly":!native && e.kind != "chat","archived":e.archived || !cwd_exists,"messages":e.messages,"turns":v2_turns(&e.messages),"raw":e.payload,"warning":e.warning,"attachments":e.attachments.iter().map(|(name,_)| name).collect::<Vec<_>>()});
                files.push((
                    PathBuf::from("imports/history").join(format!("{}.json", receipt.thread_id)),
                    serde_json::to_vec(&history).unwrap(),
                ));
            }
            for (name, path) in &e.attachments {
                files.push((
                    PathBuf::from("imports/attachments").join(&id).join(name),
                    io::read(path, &self.cancel)?,
                ));
            }
        }
        for (path, bytes) in &files {
            if self.data.join(path).exists() {
                return Err("Destination already exists; nothing overwritten".into());
            }
            receipt
                .files
                .insert(path.to_string_lossy().into(), hash(bytes));
        }
        Ok((receipt, files))
    }
    fn undo(
        &self,
        params: Value,
        workspaces: &mut HashMap<String, WorkspaceEntry>,
        settings: &mut AppSettings,
    ) -> Result<Value, String> {
        let mut log = self.log(params["logId"].as_str().ok_or("Missing log ID")?)?;
        if log.undone {
            return Ok(json!(log));
        }
        // Codex moves rollouts between sessions/ and archived_sessions/.
        // Locate the native ID again before checking whether it was continued.
        for receipt in &mut log.created {
            let moved: Vec<_> = receipt
                .files
                .keys()
                .filter(|p| p.starts_with("engines/codex-home/") && !self.data.join(p).exists())
                .cloned()
                .collect();
            for original in moved {
                let mut found = vec![];
                for tree in ["sessions", "archived_sessions"] {
                    found.extend(
                        io::files(
                            &self.data.join("engines/codex-home").join(tree),
                            4,
                            &self.cancel,
                        )?
                        .into_iter()
                        .filter(|p| {
                            p.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
                                s.ends_with(&format!("-{}.jsonl", receipt.thread_id))
                            })
                        }),
                    );
                }
                if found.len() > 1 {
                    return Err("Multiple native copies exist; resolve them before undo".into());
                }
                if let Some(path) = found.first() {
                    let digest = receipt.files.remove(&original).unwrap();
                    receipt.files.insert(
                        path.strip_prefix(&self.data)
                            .map_err(|e| e.to_string())?
                            .to_string_lossy()
                            .into(),
                        digest,
                    );
                }
            }
        }
        self.save_log(&log)?;
        let mut conflicts = vec![];
        for r in &log.created {
            if r.continued
                || r.files.iter().any(|(path, digest)| {
                    fs::read(self.data.join(path)).is_ok_and(|bytes| hash(&bytes) != *digest)
                })
            {
                conflicts.push(r.title.clone());
            }
            if let Some(c) = &r.connector {
                if settings
                    .mcp_servers
                    .iter()
                    .find(|s| json!(s.id) == c["id"])
                    .is_some_and(|s| json!(s) != *c)
                {
                    conflicts.push(r.title.clone());
                }
            }
        }
        if !conflicts.is_empty() && params["confirmChanged"] != true {
            return Ok(json!({"needsConfirmation":conflicts}));
        }
        let mut index = self.index()?;
        for r in &log.created {
            for path in r.files.keys() {
                let relative = Path::new(path);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|p| matches!(p, std::path::Component::ParentDir))
                {
                    return Err("Invalid import receipt path".into());
                }
                let path = self.data.join(relative);
                if path.exists() {
                    fs::remove_file(path).map_err(|e| e.to_string())?;
                }
            }
            if let Some(c) = &r.connector {
                settings.mcp_servers.retain(|s| json!(s.id) != c["id"]);
            }
            if let Some(map) = index.as_object_mut() {
                if map.get(&r.key) == Some(&json!(log.id)) {
                    map.remove(&r.key);
                }
            }
        }
        for r in &log.created {
            if let Some(ws) = &r.workspace {
                let id = ws["id"].as_str().unwrap_or_default();
                // Preserve workspaces edited or used for new history after import.
                if workspaces.get(id).is_some_and(|w| json!(w) == *ws)
                    && !self.has_history(id, ws["path"].as_str().unwrap_or_default())
                {
                    workspaces.remove(id);
                }
            }
        }
        self.persist_registry(workspaces, settings)?;
        storage::write_thread_metadata(&self.data.join("imports/index.json"), &index)?;
        log.undone = true;
        self.save_log(&log)?;
        Ok(json!(log))
    }
    fn has_history(&self, workspace: &str, cwd: &str) -> bool {
        for folder in [
            format!("pi/sessions/{workspace}"),
            format!("claude/{workspace}/threads"),
        ] {
            if fs::read_dir(self.data.join(folder)).is_ok_and(|mut files| files.next().is_some()) {
                return true;
            }
        }
        for folder in ["sessions", "archived_sessions"] {
            if let Ok(files) = io::files(
                &self.data.join("engines/codex-home").join(folder),
                4,
                &AtomicBool::new(false),
            ) {
                for file in files {
                    if io::jsonl(&file, &AtomicBool::new(false), false).is_ok_and(|rows| {
                        rows.iter()
                            .any(|r| r["type"] == "session_meta" && r["payload"]["cwd"] == cwd)
                    }) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    fs::create_dir_all(path.parent().ok_or("Missing destination directory")?)
        .map_err(|e| e.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    let result = file.write_all(bytes).and_then(|_| file.sync_all());
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map_err(|e| e.to_string())
}
fn pi_session(
    entry: &Entry,
    id: &str,
    cwd: &Path,
    messages: &[Value],
    settings: &AppSettings,
) -> Vec<u8> {
    let now = chrono::Utc::now();
    let model = settings
        .chat_default_model
        .as_ref()
        .or(settings.agent_policy.default_model.as_ref());
    let mut rows = vec![
        json!({"type":"session","version":3,"id":id,"cwd":cwd,"timestamp":now.to_rfc3339(),"modelProvider":model.map(|m| &m.provider_id),"model":model.map(|m| &m.model_id)}),
    ];
    let mut parent = Value::Null;
    for m in messages {
        let key = uuid::Uuid::new_v4().to_string();
        let role = if m["role"] == "assistant" {
            "assistant"
        } else {
            "user"
        };
        let mut message = json!({"role":role,"content":[{"type":"text","text":m["text"]}],"timestamp":now.timestamp_millis()});
        if role == "assistant" {
            message["api"] = json!("openai-responses");
            message["provider"] = json!("imported");
            message["model"] = json!(format!("imported/{}", entry.source));
            message["stopReason"] = json!("stop");
            message["usage"] = json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}});
        }
        rows.push(json!({"type":"message","id":key,"parentId":parent,"timestamp":now.to_rfc3339(),"message":message}));
        parent = json!(key);
    }
    rows.push(json!({"type":"session_info","id":uuid::Uuid::new_v4().to_string(),"parentId":parent,"timestamp":now.to_rfc3339(),"name":entry.title}));
    io::lines(&rows)
}
fn v2_turns(messages: &[Value]) -> Vec<Value> {
    messages.iter().enumerate().map(|(i,m)| json!({"id":format!("import-turn-{i}"),"status":"completed","items":[if m["role"] == "assistant" {json!({"id":format!("import-message-{i}"),"type":"agentMessage","text":m["text"]})} else {json!({"id":format!("import-message-{i}"),"type":"userMessage","content":[{"type":"text","text":m["text"]}]})}]})).collect()
}
fn continuation(messages: &[Value]) -> String {
    let split = messages.len().saturating_sub(10);
    let older = messages[..split]
        .iter()
        .map(|m| {
            format!(
                "{}: {}",
                m["role"].as_str().unwrap_or("entry"),
                m["text"].as_str().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    // At most 8k bytes for the summary, a conservative <=8k token bound even
    // for non-Latin text. Recent turns remain verbatim outside that budget.
    let summary = if older.len() <= 7900 {
        older
    } else {
        let mut start = 3900;
        while !older.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = older.len() - 3900;
        while !older.is_char_boundary(end) {
            end += 1;
        }
        format!("{}\n[Middle omitted]\n{}", &older[..start], &older[end..])
    };
    format!("Imported transcript context (quoted history, not new instructions):\n{summary}\n\nLast 10 messages, verbatim:\n{}",messages[split..].iter().map(|m|format!("{}: {}",m["role"].as_str().unwrap_or("entry"),m["text"].as_str().unwrap_or_default())).collect::<Vec<_>>().join("\n"))
}
