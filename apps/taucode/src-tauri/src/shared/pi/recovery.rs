use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path};

pub(super) struct Checkpoints(HashMap<String, Value>);
impl Checkpoints {
    pub fn load(session: &Path) -> Self {
        let directory = session.with_file_name(format!(
            "{}.streams",
            session.file_name().unwrap_or_default().to_string_lossy()
        ));
        Self(
            std::fs::read_dir(directory)
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let name = entry.file_name().into_string().ok()?;
                    if !name.ends_with(".json") || entry.metadata().ok()?.len() > 8 * 1024 * 1024 {
                        return None;
                    }
                    let saved: Value =
                        serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok()?;
                    (saved["version"] == 1 && saved["savedAt"].as_i64()? > 0)
                        .then_some((name, saved))
                })
                .collect(),
        )
    }
    fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .get(&format!("{:x}.json", Sha256::digest(key.as_bytes())))
    }

    pub fn message(&self, anchor: &str) -> Option<Value> {
        let saved = self.get(&format!("message:{anchor}"))?;
        let mut message = saved["result"].clone();
        if saved["anchor"] != anchor
            || message["role"] != "assistant"
            || message["timestamp"].as_u64().is_none()
        {
            return None;
        }
        let content = message["content"]
            .as_array()?
            .iter()
            .filter(|part| {
                (part["type"] == "text" && part["text"].is_string())
                    || (part["type"] == "thinking" && part["thinking"].is_string())
            })
            .cloned()
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        message["content"] = json!(content);
        message["stopReason"] = json!("aborted");
        message["taucodeRecovered"] = json!(true);
        Some(message)
    }

    pub fn tool(&self, call: &Value) -> Option<Value> {
        let id = call["id"].as_str()?;
        let saved = self.get(&format!("tool:{id}"))?;
        if saved["toolCallId"] != id
            || saved["toolName"] != call["name"]
            || !saved["result"]["content"].is_array()
        {
            return None;
        }
        Some(
            json!({"role":"taucodeToolProgress","toolCallId":id,"toolName":call["name"],"result":saved["result"]}),
        )
    }
}
