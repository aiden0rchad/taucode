//! GitHub release checks and issue reports for taucode itself.
//! Nothing here installs updates or posts issues: the user follows a link and acts on GitHub.
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::OnceLock;
use taucode_diag_mcp::{core, writer};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

pub(crate) const REPO: &str = "aiden0rchad/taucode";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
// GitHub rejects new-issue links much longer than this.
const MAX_ISSUE_URL: usize = 7800;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateCheck {
    /// available | latest | noReleases | unavailable
    status: &'static str,
    current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    html_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl UpdateCheck {
    fn unavailable(current: &str, message: impl Into<String>) -> Self {
        Self { status: "unavailable", current_version: current.into(), latest_version: None, name: None, notes: None, html_url: None, download_url: None, published_at: None, message: Some(message.into()) }
    }
}

/// Numeric `major.minor.patch` plus an optional pre-release tag, ordered like semver.
fn parse_version(value: &str) -> Option<(Vec<u64>, Option<String>)> {
    let value = value.trim().trim_start_matches(['v', 'V']);
    let value = value.split('+').next()?;
    let (core, pre) = match value.split_once('-') { Some((core, pre)) => (core, Some(pre.to_string())), None => (value, None) };
    let mut parts = core.split('.').map(|part| part.parse::<u64>().ok()).collect::<Option<Vec<_>>>()?;
    if parts.is_empty() { return None; }
    parts.resize(3.max(parts.len()), 0);
    Some((parts, pre))
}

pub(crate) fn is_newer(latest: &str, current: &str) -> bool {
    let (Some((latest, latest_pre)), Some((current, current_pre))) = (parse_version(latest), parse_version(current)) else { return false };
    match latest.cmp(&current) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (latest_pre, current_pre) {
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a > b,
            _ => false,
        },
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max { return text.to_string(); }
    text.chars().take(max).collect::<String>() + "…"
}

/// Turn a `releases/latest` response into what the UI shows. Pure, so every outcome is testable.
pub(crate) fn interpret_release(status: u16, rate_remaining: Option<&str>, body: &str, current: &str) -> UpdateCheck {
    match status {
        200 => {
            let Ok(release) = serde_json::from_str::<Value>(body) else { return UpdateCheck::unavailable(current, "GitHub sent an unreadable release.") };
            let tag = release["tag_name"].as_str().unwrap_or_default();
            if tag.is_empty() || parse_version(tag).is_none() { return UpdateCheck::unavailable(current, "The latest release has no version number."); }
            let latest = tag.trim_start_matches(['v', 'V']).to_string();
            let html_url = release["html_url"].as_str().map(str::to_string);
            let download_url = release["assets"].as_array().and_then(|assets| {
                assets.iter().filter_map(|asset| asset["browser_download_url"].as_str()).find(|url| url.ends_with(".dmg")).map(str::to_string)
            }).or_else(|| html_url.clone());
            UpdateCheck {
                status: if is_newer(&latest, current) { "available" } else { "latest" },
                current_version: current.into(),
                latest_version: Some(latest),
                name: release["name"].as_str().filter(|name| !name.trim().is_empty()).map(str::to_string),
                notes: release["body"].as_str().map(|notes| truncate(notes.trim(), 4000)).filter(|notes| !notes.is_empty()),
                html_url,
                download_url,
                published_at: release["published_at"].as_str().map(str::to_string),
                message: None,
            }
        }
        404 => UpdateCheck { status: "noReleases", message: Some("No releases published yet.".into()), ..UpdateCheck::unavailable(current, "") },
        403 | 429 if status == 429 || rate_remaining == Some("0") => UpdateCheck::unavailable(current, "GitHub's rate limit was reached. taucode will try again later."),
        _ => UpdateCheck::unavailable(current, format!("GitHub answered with status {status}. taucode will try again later.")),
    }
}

#[tauri::command]
pub(crate) async fn app_update_check() -> Result<UpdateCheck, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("taucode/{CURRENT_VERSION}"))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await;
    let Ok(response) = response else { return Ok(UpdateCheck::unavailable(CURRENT_VERSION, "Couldn't reach GitHub. Check your connection and try again.")) };
    let status = response.status().as_u16();
    let remaining = response.headers().get("x-ratelimit-remaining").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = response.text().await.unwrap_or_default();
    Ok(interpret_release(status, remaining.as_deref(), &body, CURRENT_VERSION))
}

/// Strip known secrets, key-shaped tokens, e-mail addresses and every user's home directory.
pub(crate) fn scrub(text: &str) -> String {
    static HOMES: OnceLock<Regex> = OnceLock::new();
    let homes = HOMES.get_or_init(|| Regex::new(r"(?:/Users|/home)/[^/\s:]+|[A-Za-z]:\\Users\\[^\\\s]+").unwrap());
    let text = writer::redact_known(text);
    let home = std::env::var("HOME").unwrap_or_default();
    let text = if home.len() > 1 { text.replace(&home, "~") } else { text };
    homes.replace_all(&text, "~").into_owned()
}

fn cell(text: &str) -> String {
    truncate(&scrub(text).replace(['\n', '\r'], " ").replace('|', "\\|").replace('`', "'"), 120)
}

pub(crate) fn draft_body(environment: &[(&str, String)], issues: &[core::Issue], focus: Option<&str>) -> String {
    let mut body = String::from("### What happened\n<!-- What did you do, what did you expect, and what happened instead? -->\n\n\n### Steps to reproduce\n1. \n\n### Environment\n");
    for (label, value) in environment {
        body += &format!("- {label}: {}\n", cell(value));
    }
    body += "\n### Recent diagnostics\n";
    let mut rows: Vec<&core::Issue> = issues.iter().filter(|i| i.status == "open" || Some(i.fingerprint.as_str()) == focus).collect();
    rows.sort_by(|a, b| (Some(b.fingerprint.as_str()) == focus).cmp(&(Some(a.fingerprint.as_str()) == focus)).then(b.last_seen.cmp(&a.last_seen)));
    if rows.is_empty() {
        body += "_No open issues in the local diagnostics journal (it may be turned off in Settings → Diagnostics)._\n";
    } else {
        body += "Open issues from the local diagnostics journal (codes and counts only, no file contents):\n\n| Issue | Code | Area | Severity | Count | Last seen | Fingerprint |\n|---|---|---|---|---|---|---|\n";
        for issue in rows.into_iter().take(8) {
            let marker = if Some(issue.fingerprint.as_str()) == focus { "**This issue:** " } else { "" };
            body += &format!("| {marker}{} | `{}` | {} | {} | {} | {} | `{}` |\n", cell(&issue.title), cell(&issue.code), cell(&issue.area), cell(&issue.severity), issue.count, cell(issue.last_seen.get(..10).unwrap_or(&issue.last_seen)), cell(&issue.fingerprint));
        }
    }
    body += "\n<!-- Screenshots: paste an image here (taucode can copy one to your clipboard). -->\n";
    body
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// Build the prefilled new-issue link, shortening the body if GitHub would reject it.
pub(crate) fn issue_url(title: &str, body: &str) -> (String, bool) {
    let base = format!("https://github.com/{REPO}/issues/new?title={}&body=", encode(title.trim()));
    let full = base.clone() + &encode(body);
    if full.len() <= MAX_ISSUE_URL { return (full, false); }
    let note = "\n\n_(Shortened to fit a GitHub link. The full report is on your clipboard; paste it over this text.)_";
    let mut chars: Vec<char> = body.chars().collect();
    loop {
        let candidate = base.clone() + &encode(&(chars.iter().collect::<String>() + note));
        if candidate.len() <= MAX_ISSUE_URL || chars.is_empty() { return (candidate, true); }
        let excess = (candidate.len() - MAX_ISSUE_URL) / 9 + 1;
        chars.truncate(chars.len().saturating_sub(excess));
    }
}

fn macos_version() -> String {
    std::process::Command::new("sw_vers").arg("-productVersion").output().ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .map(|v| format!("macOS {v}"))
        .unwrap_or_else(|| std::env::consts::OS.to_string())
}

#[tauri::command]
pub(crate) async fn issue_report_draft(app: AppHandle, context: Option<Value>) -> Result<Value, String> {
    let context = context.unwrap_or_else(|| json!({}));
    let text = |key: &str| context[key].as_str().filter(|v| !v.trim().is_empty()).unwrap_or("unknown").to_string();
    let focus = context["fingerprint"].as_str().map(str::to_string);
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("diagnostics");
    let issues: Vec<core::Issue> = if dir.is_dir() { core::journal_view(&dir).issues.into_values().collect() } else { Vec::new() };
    let environment = [
        ("taucode", format!("{CURRENT_VERSION} ({}, commit {})", text("buildType"), text("commit"))),
        ("OS", format!("{} ({})", macos_version(), std::env::consts::ARCH)),
        ("Engine", text("engine")),
        ("Model", text("model")),
    ];
    let title = focus.as_deref().and_then(|f| issues.iter().find(|i| i.fingerprint == f)).map(|i| truncate(&scrub(&i.title), 100)).unwrap_or_default();
    Ok(json!({ "title": title, "body": draft_body(&environment, &issues, focus.as_deref()) }))
}

#[tauri::command]
pub(crate) async fn issue_report_open(app: AppHandle, title: String, body: String) -> Result<Value, String> {
    let (url, truncated) = issue_url(&scrub(&title), &scrub(&body));
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())?;
    Ok(json!({ "truncated": truncated }))
}

#[cfg(target_os = "macos")]
fn copy_png(bytes: &[u8]) -> Result<(), String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG};
    use objc2_foundation::NSData;
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    if pasteboard.setData_forType(Some(&NSData::with_bytes(bytes)), unsafe { NSPasteboardTypePNG }) { Ok(()) } else { Err("The clipboard refused the screenshot".into()) }
}

/// Snapshot taucode's own window content (no screen-recording permission needed),
/// then copy it to the clipboard or save it to Downloads and reveal it in Finder.
#[tauri::command]
pub(crate) async fn issue_report_screenshot(app: AppHandle, action: String) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    {
        use base64::Engine;
        let view = app.get_webview("main").ok_or("The taucode window is not open")?;
        let encoded = crate::shared::browser::native::macos::image(&view).await?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.as_str().unwrap_or_default()).map_err(|e| e.to_string())?;
        match action.as_str() {
            "copy" => { copy_png(&bytes)?; Ok(json!({ "copied": true })) }
            "save" => {
                let folder = app.path().download_dir().or_else(|_| app.path().temp_dir()).map_err(|e| e.to_string())?;
                let path = folder.join(format!("taucode-screenshot-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                std::fs::write(&path, bytes).map_err(|e| format!("Couldn't save the screenshot: {e}"))?;
                let _ = app.opener().reveal_item_in_dir(&path);
                Ok(json!({ "path": scrub(&path.to_string_lossy()) }))
            }
            _ => Err("Unknown screenshot action".into()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = (app, action); Err("Screenshots are only available on macOS".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_like_semver() {
        assert!(is_newer("0.7.69", "0.7.68"));
        assert!(is_newer("v0.8.0", "0.7.68"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.8.0", "0.8.0-beta.1"));
        assert!(is_newer("0.8.0-beta.2", "0.8.0-beta.1"));
        assert!(!is_newer("0.7.68", "0.7.68"));
        assert!(!is_newer("0.7.9", "0.7.68"));
        assert!(!is_newer("0.8.0-beta.1", "0.8.0"));
        assert!(!is_newer("nightly", "0.7.68"));
    }

    #[test]
    fn interprets_release_responses() {
        let release = json!({"tag_name":"v0.8.0","name":"taucode 0.8","body":"Notes","html_url":"https://github.com/aiden0rchad/taucode/releases/tag/v0.8.0","published_at":"2026-11-01T00:00:00Z","assets":[{"browser_download_url":"https://x/taucode.zip"},{"browser_download_url":"https://x/taucode.dmg"}]}).to_string();
        let check = interpret_release(200, None, &release, "0.7.68");
        assert_eq!(check.status, "available");
        assert_eq!(check.latest_version.as_deref(), Some("0.8.0"));
        assert_eq!(check.download_url.as_deref(), Some("https://x/taucode.dmg"));
        assert_eq!(check.notes.as_deref(), Some("Notes"));
        assert_eq!(interpret_release(200, None, &release, "0.8.0").status, "latest");
        let no_assets = json!({"tag_name":"0.8.0","html_url":"https://h"}).to_string();
        assert_eq!(interpret_release(200, None, &no_assets, "0.7.0").download_url.as_deref(), Some("https://h"));
        assert_eq!(interpret_release(404, None, "{}", "0.7.68").status, "noReleases");
        let limited = interpret_release(403, Some("0"), "{}", "0.7.68");
        assert_eq!(limited.status, "unavailable");
        assert!(limited.message.unwrap().contains("rate limit"));
        assert!(interpret_release(429, None, "", "0.7.68").message.unwrap().contains("rate limit"));
        assert_eq!(interpret_release(500, None, "", "0.7.68").status, "unavailable");
        assert_eq!(interpret_release(200, None, "not json", "0.7.68").status, "unavailable");
        assert_eq!(interpret_release(200, None, r#"{"tag_name":"latest"}"#, "0.7.68").status, "unavailable");
    }

    #[test]
    fn scrubs_secrets_and_home_paths() {
        let text = scrub("key sk-ant-abc123def456 at /Users/someone/Projects/app and OPENAI_API_KEY=xyz mail a@b.co");
        assert!(!text.contains("sk-ant-abc123"), "{text}");
        assert!(!text.contains("someone"), "{text}");
        assert!(text.contains("~/Projects/app"), "{text}");
        assert!(!text.contains("xyz") && !text.contains("a@b.co"), "{text}");
    }

    fn issue(fingerprint: &str, status: &str, last_seen: &str, title: &str) -> core::Issue {
        core::Issue { fingerprint: fingerprint.into(), code: "engine.crash".into(), area: "engine".into(), severity: "high".into(), title: title.into(), count: 3, first_seen: last_seen.into(), last_seen: last_seen.into(), last_event: core::event("error", "engine", "engine.crash", title), engines: Default::default(), models: Default::default(), status: status.into(), notes: vec![], regressed: false }
    }

    #[test]
    fn drafts_body_with_environment_and_open_issues() {
        let issues = vec![
            issue("aaaa", "open", "2026-10-30T00:00:00Z", "Old | crash in /Users/someone/x"),
            issue("bbbb", "open", "2026-10-31T00:00:00Z", "New crash"),
            issue("cccc", "fixed", "2026-10-31T00:00:00Z", "Fixed crash"),
        ];
        let body = draft_body(&[("taucode", "0.7.68".into()), ("Model", "glm".into())], &issues, None);
        assert!(body.contains("- taucode: 0.7.68\n- Model: glm"));
        assert!(body.find("New crash").unwrap() < body.find("Old \\| crash").unwrap());
        assert!(!body.contains("Fixed crash") && !body.contains("someone"));
        let focused = draft_body(&[], &issues, Some("cccc"));
        assert!(focused.find("**This issue:** Fixed crash").unwrap() < focused.find("New crash").unwrap());
        assert!(draft_body(&[], &[], None).contains("No open issues"));
    }

    #[test]
    fn builds_issue_links_within_github_limits() {
        let (url, truncated) = issue_url("Crash on start", "Body & more");
        assert_eq!(url, "https://github.com/aiden0rchad/taucode/issues/new?title=Crash+on+start&body=Body+%26+more");
        assert!(!truncated);
        let (long, truncated) = issue_url("t", &"é".repeat(5000));
        assert!(truncated && long.len() <= MAX_ISSUE_URL);
        assert!(long.contains("Shortened"));
    }
}
