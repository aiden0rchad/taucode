use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, State};

use self::io::TextFileResponse;
use self::policy::{FileKind, FileScope};
use crate::remote_backend;
use crate::shared::codex_core;
use crate::shared::files_core::{file_read_core, file_write_core};
use crate::state::AppState;

pub(crate) mod io;
pub(crate) mod ops;
pub(crate) mod policy;

async fn file_read_impl(
    scope: FileScope,
    kind: FileKind,
    workspace_id: Option<String>,
    state: &AppState,
    app: &AppHandle,
) -> Result<TextFileResponse, String> {
    if remote_backend::is_remote_mode(state).await {
        let response = remote_backend::call_remote(
            state,
            app.clone(),
            "file_read",
            json!({ "scope": scope, "kind": kind, "workspaceId": workspace_id }),
        )
        .await?;
        return serde_json::from_value(response).map_err(|err| err.to_string());
    }

    file_read_core(&state.workspaces, scope, kind, workspace_id).await
}

async fn file_write_impl(
    scope: FileScope,
    kind: FileKind,
    workspace_id: Option<String>,
    content: String,
    state: &AppState,
    app: &AppHandle,
) -> Result<(), String> {
    if remote_backend::is_remote_mode(state).await {
        remote_backend::call_remote(
            state,
            app.clone(),
            "file_write",
            json!({
                "scope": scope,
                "kind": kind,
                "workspaceId": workspace_id,
                "content": content,
            }),
        )
        .await?;
        return Ok(());
    }

    file_write_core(&state.workspaces, scope, kind, workspace_id, content).await
}

#[tauri::command]
pub(crate) async fn file_read(
    scope: FileScope,
    kind: FileKind,
    workspace_id: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<TextFileResponse, String> {
    file_read_impl(scope, kind, workspace_id, &*state, &app).await
}

#[tauri::command]
pub(crate) async fn file_write(
    scope: FileScope,
    kind: FileKind,
    workspace_id: Option<String>,
    content: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    file_write_impl(scope, kind, workspace_id, content, &*state, &app).await
}

#[tauri::command]
pub(crate) async fn read_image_as_data_url(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return Err("Image path is required".to_string());
    }

    let mobile_runtime = cfg!(any(target_os = "ios", target_os = "android"));
    let remote_mode = remote_backend::is_remote_mode(&*state).await;
    if !mobile_runtime && !remote_mode {
        return Err("Image conversion is only supported in remote backend mode or on mobile runtimes".to_string());
    }

    let normalized = codex_core::normalize_file_path(trimmed_path);
    if normalized.is_empty() {
        return Err("Image path is required".to_string());
    }

    let _ = app;
    codex_core::read_image_as_data_url_core(&normalized)
}

#[tauri::command]
pub(crate) fn write_text_file(path: String, content: String) -> Result<(), String> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("Path is required".to_string());
    }
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("Failed to create export directory: {err}"))?;
        }
    }
    std::fs::write(&target, content).map_err(|err| format!("Failed to write export file: {err}"))
}

/// Local export to a path chosen in the desktop Save dialog, including in remote mode.
#[tauri::command]
pub(crate) fn write_png_file(path: String, data: String) -> Result<(), String> {
    use base64::Engine;
    use std::io::Write;
    let target = PathBuf::from(path);
    if !target.is_absolute() || target.extension().and_then(|ext| ext.to_str()).is_none_or(|ext| !ext.eq_ignore_ascii_case("png")) {
        return Err("Choose an absolute .png file path".into());
    }
    if data.len() > 32 * 1024 * 1024 { return Err("Screenshot exceeds 24 MB".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| "Invalid screenshot encoding")?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Err("Expected a PNG screenshot".into()); }
    let temporary = target.with_file_name(format!(".taucode-export-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &target)
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result.map_err(|err: std::io::Error| format!("Failed to save screenshot: {err}"))
}

#[cfg(test)]
mod export_tests {
    #[test]
    fn screenshot_export_rejects_non_png_and_relative_paths() {
        assert!(super::write_png_file("image.png".into(), "AAAA".into()).is_err());
        assert!(super::write_png_file("/tmp/image.txt".into(), "AAAA".into()).is_err());
        assert!(super::write_png_file("/tmp/image.png".into(), "AAAA".into()).unwrap_err().contains("PNG"));
    }
}
