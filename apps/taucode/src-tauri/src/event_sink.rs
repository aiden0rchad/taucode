use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput};

pub(crate) struct TauriEventSink<R: tauri::Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: tauri::Runtime> Clone for TauriEventSink<R> {
    fn clone(&self) -> Self { Self::new(self.app.clone()) }
}

impl<R: tauri::Runtime> TauriEventSink<R> {
    pub(crate) fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> EventSink for TauriEventSink<R> {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        taucode_diag_mcp::observe::observe(&event.message);
        work_status(&self.app, &event.message);
        let _ = emit(&self.app, "app-server-event", event);
    }

    fn emit_terminal_output(&self, event: TerminalOutput) {
        let _ = emit(&self.app, "terminal-output", event);
    }

    fn emit_terminal_exit(&self, event: TerminalExit) {
        let _ = emit(&self.app, "terminal-exit", event);
    }
}

// Remote events use the same native notification/badge path as local Work tasks.
pub(crate) fn work_status<R: tauri::Runtime>(app: &AppHandle<R>, message: &serde_json::Value) {
        if message["method"] == "work/updated" {
            let params = &message["params"];
            for window in app.windows().values() {
                let _ = window.set_badge_count(params["needsInput"].as_i64().filter(|n| *n > 0));
            }
            if params["notify"] == true {
                let task = &params["task"];
                let _ = app.notification().builder()
                    .title(task["title"].as_str().unwrap_or("Work task"))
                    .body(format!("Task {}", task["status"].as_str().unwrap_or("updated").replace('-', " "))).show();
            }
        }
}

// Explicit allowlist: new/untrusted child webviews never inherit app data events.
pub(crate) fn is_app_webview(label: &str) -> bool {
    matches!(label, "main" | "about")
}

pub(crate) fn emit<R: tauri::Runtime, S: serde::Serialize + Clone>(
    app: &AppHandle<R>, event: &str, payload: S,
) -> tauri::Result<()> {
    app.emit_filter(event, payload, |target| matches!(target,
        tauri::EventTarget::Webview { label } if is_app_webview(label)))
}
