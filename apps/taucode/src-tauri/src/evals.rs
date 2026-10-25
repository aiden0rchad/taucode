use crate::{remote_backend,state::AppState};
use serde_json::{json,Value};
use tauri::{AppHandle,State};
#[tauri::command]
pub(crate) async fn eval_request(state:State<'_,AppState>,app:AppHandle,action:String,params:Value)->Result<Value,String> {
    if remote_backend::is_remote_mode(&*state).await {
        return remote_backend::call_remote(&*state,app,"eval_request",json!({"action":action,"params":params})).await;
    }
    let settings=state.app_settings.lock().await.clone();
    state.evals.request(&settings,&action,params).await
}


#[derive(Clone)]
struct LocalSink;
impl crate::backend::events::EventSink for LocalSink {
    fn emit_app_server_event(&self,_:crate::backend::events::AppServerEvent) {}
    fn emit_terminal_output(&self,_:crate::backend::events::TerminalOutput) {}
    fn emit_terminal_exit(&self,_:crate::backend::events::TerminalExit) {}
}

/// In-process CLI host using the same evaluator and data-directory settings.
pub struct LocalEvals {
    service: std::sync::Arc<crate::shared::evals::Evals>,
    data: std::path::PathBuf,
    browser: std::sync::Arc<crate::shared::browser::Browser>,
}
impl LocalEvals {
    pub fn new(data:std::path::PathBuf)->Self {
        crate::codex::home::set_app_data_dir(&data);
        Self {service:crate::shared::evals::Evals::new(data.clone(),LocalSink),browser:crate::shared::browser::Browser::new(data.clone()),data}
    }
    pub async fn shutdown(&self) { self.browser.shutdown().await; }
    pub async fn request(&self,action:&str,params:Value)->Result<Value,String> {
        if action=="start" { self.browser.serve().await?; }
        let settings=crate::storage::read_settings(&self.data.join("settings.json"))?;
        self.service.request(&settings,action,params).await
    }
}
