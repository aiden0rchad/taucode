use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

// OMP stores image bytes under its isolated agent home, leaving content-addressed
// references in session JSON. Hydrate for display without rewriting the session.
pub(super) fn restore(message: &mut Value, directory: &Path) {
    for part in message
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if part["type"] != "image" {
            continue;
        }
        let Some(reference) = part["data"].as_str().filter(|s| s.starts_with("blob:")) else {
            continue;
        };
        let bytes = (|| {
            let hash = reference.strip_prefix("blob:sha256:")?;
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            if !matches!(
                part["mimeType"].as_str(),
                Some("image/png" | "image/jpeg" | "image/webp" | "image/gif")
            ) {
                return None;
            }
            let root = directory.canonicalize().ok()?;
            let path = root.join(hash).canonicalize().ok()?;
            if path.parent() != Some(root.as_path()) {
                return None;
            }
            let file = std::fs::File::open(path).ok()?;
            const LIMIT: u64 = 20 * 1024 * 1024;
            if !file.metadata().ok()?.is_file() {
                return None;
            }
            let mut bytes = Vec::new();
            file.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
            (bytes.len() as u64 <= LIMIT && format!("{:x}", Sha256::digest(&bytes)) == hash)
                .then_some(bytes)
        })();
        if let Some(bytes) = bytes {
            part["data"] = json!(STANDARD.encode(bytes));
        } else {
            *part = json!({"type":"text","text":"Image preview unavailable: the saved image is missing or invalid."});
        }
    }
}
