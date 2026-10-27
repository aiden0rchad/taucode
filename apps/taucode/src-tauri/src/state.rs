use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::dictation::DictationState;
use crate::storage::{read_settings, read_workspaces};
use crate::types::{AppSettings, TcpDaemonState, TcpDaemonStatus, WorkspaceEntry};

pub(crate) struct TcpDaemonRuntime {
    pub(crate) child: Option<Child>,
    pub(crate) status: TcpDaemonStatus,
}

impl Default for TcpDaemonRuntime {
    fn default() -> Self {
        Self {
            child: None,
            status: TcpDaemonStatus {
                state: TcpDaemonState::Stopped,
                pid: None,
                started_at_ms: None,
                last_error: None,
                listen_addr: None,
            },
        }
    }
}

pub(crate) struct AppState {
    pub(crate) managed_tasks: Arc<crate::shared::managed_tasks::ManagedTasks>,
    pub(crate) evals: Arc<crate::shared::evals::Evals>,
    pub(crate) work: Arc<crate::shared::work::Work>,
    pub(crate) imports: Arc<crate::shared::imports::Imports>,
    pub(crate) chats: crate::shared::chats::Chats,
    pub(crate) browser: Arc<crate::shared::browser::Browser>,
    pub(crate) computer: Arc<crate::shared::computer::Computer>,
    pub(crate) browser_streams: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    pub(crate) codegraph: Arc<crate::shared::codegraph_core::Codegraph>,
    pub(crate) workspaces: Mutex<HashMap<String, WorkspaceEntry>>,
    pub(crate) sessions: Mutex<HashMap<String, Arc<crate::codex::WorkspaceSession>>>,
    pub(crate) terminal_sessions: Mutex<HashMap<String, Arc<crate::terminal::TerminalSession>>>,
    pub(crate) remote_backend: Mutex<Option<crate::remote_backend::RemoteBackend>>,
    pub(crate) storage_path: PathBuf,
    pub(crate) settings_path: PathBuf,
    pub(crate) app_settings: Mutex<AppSettings>,
    pub(crate) dictation: Mutex<DictationState>,
    pub(crate) tcp_daemon: Mutex<TcpDaemonRuntime>,
}

impl AppState {
    pub(crate) fn load(app: &AppHandle) -> Self {
        let data_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        crate::codex::home::set_app_data_dir(&data_dir);
        Self::from_data_dir(app, data_dir)
    }

    pub(crate) fn from_data_dir<R: tauri::Runtime>(app: &AppHandle<R>, data_dir: PathBuf) -> Self {
        let storage_path = data_dir.join("workspaces.json");
        let settings_path = data_dir.join("settings.json");
        let workspaces = read_workspaces(&storage_path).unwrap_or_default();
        let app_settings = read_settings(&settings_path).unwrap_or_default();
        Self {
            managed_tasks: crate::shared::managed_tasks::ManagedTasks::new(),
            evals: crate::shared::evals::Evals::new(data_dir.clone(), crate::event_sink::TauriEventSink::new(app.clone())),
            imports: crate::shared::imports::Imports::new(data_dir.clone()),
            browser: crate::shared::browser::Browser::new(data_dir.clone()),
            work: crate::shared::work::Work::new(data_dir.clone(), crate::event_sink::TauriEventSink::new(app.clone())),
            chats: crate::shared::chats::Chats::new(data_dir.clone(), crate::event_sink::TauriEventSink::new(app.clone())),
            computer: crate::shared::computer::Computer::new(data_dir.clone()),
            browser_streams: Mutex::new(HashMap::new()),
            codegraph: crate::shared::codegraph_core::Codegraph::new(data_dir),
            workspaces: Mutex::new(workspaces),
            sessions: Mutex::new(HashMap::new()),
            terminal_sessions: Mutex::new(HashMap::new()),
            remote_backend: Mutex::new(None),
            storage_path,
            settings_path,
            app_settings: Mutex::new(app_settings),
            dictation: Mutex::new(DictationState::default()),
            tcp_daemon: Mutex::new(TcpDaemonRuntime::default()),
        }
    }
}
