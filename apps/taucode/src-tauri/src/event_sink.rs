use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput};

#[derive(Clone)]
pub(crate) struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        work_status(&self.app, &event.message);
        let _ = self.app.emit("app-server-event", event);
    }

    fn emit_terminal_output(&self, event: TerminalOutput) {
        let _ = self.app.emit("terminal-output", event);
    }

    fn emit_terminal_exit(&self, event: TerminalExit) {
        let _ = self.app.emit("terminal-exit", event);
    }
}

// Remote events use the same native notification/badge path as local Work tasks.
pub(crate) fn work_status(app: &AppHandle, message: &serde_json::Value) {
        if message["method"] == "work/updated" {
            let params = &message["params"];
            for window in app.webview_windows().values() {
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
