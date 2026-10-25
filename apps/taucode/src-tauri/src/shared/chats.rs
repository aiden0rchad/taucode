use super::{engines::EngineBackend, pi::{config, PiEngine}};
use crate::{backend::events::{AppServerEvent, EventSink}, types::AppSettings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub(crate) const WORKSPACE: &str = "chats";
const RESTORE_SECONDS: i64 = 30 * 24 * 60 * 60;

// App-only ingress for a file explicitly dropped by the user. Never exposed to pi or remote RPC.
pub(crate) fn attachment(path: &std::path::Path) -> Result<Value, String> {
    use std::io::Read;
    use base64::Engine;
    let name = path.file_name().and_then(|n| n.to_str()).ok_or("Invalid attachment name")?;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    let mime = match extension.as_str() {
        "png" => Some("image/png"), "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"), "webp" => Some("image/webp"), _ => None,
    };
    if mime.is_none() && !["txt", "md", "csv", "tsv", "json", "jsonl", "xml", "yaml", "yml", "toml", "log", "js", "jsx", "ts", "tsx", "py", "rs", "css", "html", "sh", "sql"].contains(&extension.as_str()) {
        return Err("Attach an image or a text-like file".into());
    }
    let limit = if mime.is_some() { 20 * 1024 * 1024 } else { 1024 * 1024 };
    if !std::fs::metadata(path).map_err(|e| e.to_string())?.is_file() { return Err("Attach a regular file".into()); }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() { return Err("Attach a regular file".into()); }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit { return Err("Text attachments must be ≤ 1 MB; images ≤ 20 MB".into()); }
    if let Some(mime) = mime {
        Ok(json!({"name":name,"url":format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))}))
    } else {
        let text = String::from_utf8(bytes).map_err(|_| "Text attachments must be UTF-8")?;
        if text.contains('\0') { return Err("Binary files are not supported".into()); }
        Ok(json!({"name":name,"text":text}))
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    title: Option<String>,
    pinned: bool,
    deleted_at: Option<i64>,
    #[serde(default)]
    web: bool,
}

pub(crate) struct Chats {
    pub engine: PiEngine,
    path: PathBuf,
    operation: Mutex<()>,
}
impl Chats {
    pub fn new(data: PathBuf, sink: impl EventSink) -> Self {
        Self {
            path: data.join("chats/metadata.json"),
            engine: PiEngine::chat(data, Arc::new(move |workspace, message| {
                sink.emit_app_server_event(AppServerEvent { workspace_id: workspace.into(), message });
            })),
            operation: Mutex::new(()),
        }
    }
    fn metadata(&self) -> Result<HashMap<String, Metadata>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub async fn request(&self, settings: &AppSettings, action: &str, params: Value) -> Result<Value, String> {
        // The public chat API never accepts cwd, access policies, local file paths, or arbitrary engine methods.
        let mut policy = settings.agent_policy.clone();
        if settings.chat_default_model.is_some() { policy.default_model = settings.chat_default_model.clone(); }
        self.engine.configure(WORKSPACE, settings.providers.clone(), policy).await;
        let _operation = self.operation.lock().await;
        let mut metadata = self.metadata()?;
        if action == "list" {
            let mut threads = Vec::new();
            let mut cursor = Value::Null;
            loop {
                let response = self.engine.request(WORKSPACE, "thread/list", json!({"limit":1000,"cursor":cursor})).await?;
                threads.extend(response["result"]["data"].as_array().ok_or("Invalid chat list")?.iter().cloned());
                cursor = response["result"]["nextCursor"].clone();
                if cursor.is_null() { break; }
            }
            for thread in &mut threads {
                let meta = metadata.get(thread["id"].as_str().unwrap_or_default()).cloned().unwrap_or_default();
                thread["chat"] = json!(meta);
                thread["running"] = json!(self.engine.is_running(thread["id"].as_str().unwrap_or_default()).await);
            }
            return Ok(json!(threads));
        }
        if action == "start" {
            return self.engine.request(WORKSPACE, "thread/start", json!({"model":params["model"]})).await;
        }
        let id = params["threadId"].as_str().ok_or("Missing chat ID")?;
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid chat ID")?;
        if !self.engine.owns(WORKSPACE, id).await { return Err("Unknown chat".into()); }
        let meta = metadata.entry(id.into()).or_default();
        if action == "restore" {
            let deleted = meta.deleted_at.ok_or("Chat is not in Trash")?;
            if chrono::Utc::now().timestamp() - deleted > RESTORE_SECONDS { return Err("The 30-day restore period has expired".into()); }
            meta.deleted_at = None;
        } else {
            if meta.deleted_at.is_some() { return Err("Restore this chat from Trash first".into()); }
            match action {
                "rename" => {
                    let title = params["title"].as_str().ok_or("Missing title")?.trim();
                    if title.is_empty() || title.chars().count() > 200 { return Err("Title must be 1–200 characters".into()); }
                    meta.title = Some(title.into());
                }
                "pin" => meta.pinned = params["pinned"].as_bool().ok_or("Missing pin state")?,
                "trash" => {
                    if self.engine.is_running(id).await { return Err("Stop the chat before moving it to Trash".into()); }
                    meta.deleted_at = Some(chrono::Utc::now().timestamp());
                }
                "web" => {
                    let web = params["web"].as_bool().ok_or("Missing Web state")?;
                    if web && !settings.mcp_servers.iter().any(|c| c.id == "taucode-browser" && c.enabled) { return Err("Enable the browser connector in Settings first".into()); }
                    self.engine.request(WORKSPACE, "thread/chatWeb/set", json!({"threadId":id,"web":web})).await?;
                    meta.web = web;
                }
                "read" => return self.engine.request(WORKSPACE, "thread/read", json!({"threadId":id})).await,
                "stop" => return self.engine.request(WORKSPACE, "turn/interrupt", json!({"threadId":id})).await,
                "fork" => {
                    let inherited = Metadata { pinned: false, ..meta.clone() };
                    let response = self.engine.request(WORKSPACE, "thread/fork", json!({"threadId":id,"model":params["model"],"beforeUserItemId":params["beforeUserItemId"]})).await?;
                    let child = response["result"]["thread"]["id"].as_str().ok_or("Missing fork ID")?;
                    metadata.insert(child.into(), inherited);
                    std::fs::create_dir_all(self.path.parent().unwrap()).map_err(|e| e.to_string())?;
                    config::write_json(&self.path, &json!(metadata))?;
                    return Ok(response);
                }
                "send" => {
                    self.engine.request(WORKSPACE, "thread/chatWeb/set", json!({"threadId":id,"web":meta.web && settings.mcp_servers.iter().any(|c| c.id == "taucode-browser" && c.enabled)})).await?;
                    let input = params["input"].as_array().ok_or("Missing chat input")?;
                    if input.is_empty() || input.len() > 32 { return Err("Attach at most 31 files".into()); }
                    for part in input {
                        match part["type"].as_str() {
                            Some("text") if part["text"].as_str().is_some_and(|v| v.len() <= 32 * 1024 * 1024) => (),
                            Some("image") if part["url"].as_str().is_some_and(|v| v.starts_with("data:image/") && v.len() <= 28 * 1024 * 1024) => (),
                            _ => return Err("Chat accepts only inline text and image attachments".into()),
                        }
                    }
                    if input.iter().any(|part| part["type"] == "image") {
                        let response = self.engine.request(WORKSPACE, "thread/read", json!({"threadId":id,"includeTurns":false})).await?;
                        let thread = &response["result"]["thread"];
                        if settings.providers.iter().find(|p| thread["modelProvider"] == p.id)
                            .and_then(|p| p.models.iter().find(|m| thread["model"] == m.id))
                            .is_some_and(|m| m.supports_images == Some(false)) { return Err("This model does not support images".into()); }
                    }
                    return self.engine.request(WORKSPACE, "turn/start", json!({"threadId":id,"input":input,"effort":params["effort"]})).await;
                }
                _ => return Err("Unknown chat action".into()),
            }
        }
        std::fs::create_dir_all(self.path.parent().unwrap()).map_err(|e| e.to_string())?;
        config::write_json(&self.path, &json!(metadata))?;
        Ok(json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachments_are_bounded_and_read_only_on_explicit_ingress() {
        let root = std::env::temp_dir().join(format!("chat-attachments-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let text = root.join("notes.md");
        std::fs::write(&text, "hello").unwrap();
        assert_eq!(attachment(&text).unwrap()["text"], "hello");
        std::fs::write(&text, vec![b'a'; 1024 * 1024 + 1]).unwrap();
        assert!(attachment(&text).unwrap_err().contains("1 MB"));
        std::fs::write(&text, b"binary\0data").unwrap();
        assert!(attachment(&text).is_err());
        assert!(attachment(&root.join("private.key")).is_err());
        let image = root.join("image.png");
        std::fs::write(&image, [137, 80, 78, 71]).unwrap();
        assert!(attachment(&image).unwrap()["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn chat_metadata_trash_restore_and_boundaries_survive_restart() {
        let data = std::env::temp_dir().join(format!("taucode-chats-{}", uuid::Uuid::new_v4()));
        let path = data.join("chats/metadata.json");
        let service = || Chats { engine: PiEngine::chat(data.clone(), Arc::new(|_, _| {})), path: path.clone(), operation: Mutex::new(()) };
        let chats = service();
        let id = uuid::Uuid::new_v4().to_string();
        let sessions = data.join("chats/sessions/chats");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join(format!("{id}.jsonl")), format!("{}\n", json!({"type":"session","version":3,"id":id,"cwd":data,"timestamp":"2026-10-23T12:00:00Z"}))).unwrap();
        let mut settings = AppSettings::default();
        for connector in &mut settings.mcp_servers { if connector.id == "taucode-browser" { connector.enabled = false; } }
        assert_eq!(chats.request(&settings, "list", json!({})).await.unwrap().as_array().unwrap().len(), 1);
        assert!(!data.join("workspaces.json").exists());
        chats.request(&settings, "rename", json!({"threadId":id,"title":"Saved title"})).await.unwrap();
        chats.request(&settings, "pin", json!({"threadId":id,"pinned":true})).await.unwrap();
        chats.request(&settings, "trash", json!({"threadId":id})).await.unwrap();
        let restarted = service();
        let list = restarted.request(&settings, "list", json!({})).await.unwrap();
        assert_eq!(list[0]["chat"]["title"], "Saved title");
        assert_eq!(list[0]["chat"]["pinned"], true);
        assert!(list[0]["chat"]["deletedAt"].is_number());
        assert!(restarted.request(&settings, "send", json!({"threadId":id,"input":[]})).await.unwrap_err().contains("Restore"));
        restarted.request(&settings, "restore", json!({"threadId":id})).await.unwrap();
        assert!(restarted.request(&settings, "read", json!({"threadId":id})).await.is_ok());
        assert!(restarted.request(&settings, "send", json!({"threadId":id,"input":[{"type":"localImage","path":"/etc/passwd"}]})).await.unwrap_err().contains("inline"));
        assert!(restarted.request(&settings, "web", json!({"threadId":id,"web":true})).await.is_err());
        assert!(restarted.request(&settings, "read", json!({"threadId":"../outside"})).await.is_err());
        let mut meta = restarted.metadata().unwrap();
        meta.get_mut(&id).unwrap().deleted_at = Some(chrono::Utc::now().timestamp() - RESTORE_SECONDS - 1);
        config::write_json(&path, &json!(meta)).unwrap();
        assert!(restarted.request(&settings, "restore", json!({"threadId":id})).await.unwrap_err().contains("expired"));
        assert!(restarted.request(&settings, "permanent-delete", json!({"threadId":id})).await.is_err());
        std::fs::remove_dir_all(data).unwrap();
    }
}
