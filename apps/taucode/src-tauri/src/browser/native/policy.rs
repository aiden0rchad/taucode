use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub enum Navigation {
    Allow,
    Block,
    External,
}
pub fn navigation(url: &str, current: &str) -> Navigation {
    let Ok(url) = reqwest::Url::parse(url) else {
        return Navigation::Block;
    };
    if url
        .host_str()
        .is_some_and(|h| h == "tauri.localhost" || h == "asset.localhost" || h == "ipc.localhost")
    {
        return Navigation::Block;
    }
    match url.scheme() {
        "http" | "https" => Navigation::Allow,
        "about" if url.as_str() == "about:blank" => Navigation::Allow,
        "data"
            if [
                "image/png",
                "image/jpeg",
                "image/gif",
                "image/webp",
                "image/avif",
                "image/bmp",
            ]
            .iter()
            .any(|mime| {
                url.path()
                    .strip_prefix(mime)
                    .is_some_and(|s| s.starts_with(';') || s.starts_with(','))
            }) =>
        {
            Navigation::Allow
        }
        "blob"
            if reqwest::Url::parse(url.path())
                .ok()
                .zip(reqwest::Url::parse(current).ok())
                .is_some_and(|(a, b)| {
                    matches!(b.scheme(), "http" | "https") && a.origin() == b.origin()
                }) =>
        {
            Navigation::Allow
        }
        "file" | "tauri" | "asset" | "ipc" | "taucode" | "codex-monitor" | "javascript"
        | "about" | "data" | "blob" => Navigation::Block,
        _ => Navigation::External,
    }
}
pub fn label(id: &str) -> Result<String, String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Invalid browser tab ID".into());
    }
    Ok(format!("browser-{id}"))
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub fn validate(self) -> Result<Self, String> {
        if ![self.x, self.y, self.w, self.h]
            .iter()
            .all(|n| n.is_finite())
            || self.x < 0.
            || self.y < 0.
            || self.w < 1.
            || self.h < 1.
            || self.x + self.w > 32768.
            || self.y + self.h > 32768.
        {
            return Err("Invalid browser bounds".into());
        }
        Ok(Self {
            x: self.x.round(),
            y: self.y.round(),
            w: self.w.round(),
            h: self.h.round(),
        })
    }
}
/// Oldest hidden view is evicted; tab metadata remains in the shared state.
pub fn touch(live: &mut Vec<String>, id: &str) -> Option<String> {
    live.retain(|s| s != id);
    live.push(id.into());
    (live.len() > 8).then(|| live.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_policy() {
        for url in [
            "https://example.com",
            "http://localhost:3000",
            "about:blank",
            "data:image/png;base64,AA",
            "blob:https://example.com/id",
        ] {
            assert_eq!(
                navigation(url, "https://example.com"),
                Navigation::Allow,
                "{url}"
            );
        }
        for url in [
            "file:///etc/passwd",
            "tauri://localhost",
            "asset://localhost/a",
            "ipc://x",
            "javascript:alert(1)",
            "about:config",
            "data:text/html,hi",
            "data:image/svg+xml,<svg/>",
            "blob:https://evil.com/id",
            "taucode://open",
            "https://tauri.localhost",
        ] {
            assert_eq!(
                navigation(url, "https://example.com"),
                Navigation::Block,
                "{url}"
            );
        }
        assert_eq!(navigation("mailto:a@example.com", ""), Navigation::External);
    }
    #[test]
    fn labels_bounds_and_lru() {
        assert!(label("../main").is_err());
        assert!(label("").is_err());
        assert_eq!(label("tab-1").unwrap(), "browser-tab-1");
        assert!(Rect {
            x: 0.,
            y: 0.,
            w: f64::NAN,
            h: 20.
        }
        .validate()
        .is_err());
        assert_eq!(
            Rect {
                x: 1.4,
                y: 2.6,
                w: 100.2,
                h: 50.
            }
            .validate()
            .unwrap()
            .y,
            3.
        );
        let mut live = (0..8).map(|n| n.to_string()).collect();
        assert_eq!(touch(&mut live, "0"), None);
        assert_eq!(touch(&mut live, "8"), Some("1".into()));
        assert_eq!(live.len(), 8);
    }
    #[test]
    fn capabilities_exclude_browser_webviews() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let caps = value["capabilities"]
                .as_array()
                .cloned()
                .unwrap_or(vec![value]);
            for cap in caps {
                assert!(cap.get("remote").is_none());
                assert!(cap.get("windows").is_none());
                for label in cap["webviews"].as_array().unwrap() {
                    assert!(matches!(label.as_str(), Some("main" | "about")));
                }
            }
        }
    }
}
