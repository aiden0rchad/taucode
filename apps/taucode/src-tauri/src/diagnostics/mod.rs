use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};
use taucode_diag_mcp::{core, writer, server::Journal};
use std::sync::atomic::Ordering;
use crate::{state::AppState, types::AppSettings, shared::diagnostics_core::{configure, PROCESS_START}};
pub(crate) fn initialize(app:&AppHandle,settings:&AppSettings)->Result<(),String> {writer::install_panic_hook();configure(app.path().app_data_dir().map_err(|e|e.to_string())?.join("diagnostics"),settings)}
#[tauri::command]
pub(crate) async fn diagnostics_request(app:AppHandle,state:State<'_,AppState>,action:String,params:Option<Value>)->Result<Value,String> {
    let dir=app.path().app_data_dir().map_err(|e|e.to_string())?.join("diagnostics");let p=params.unwrap_or_else(||json!({}));
    match action.as_str() {
        "record"=>{if !writer::enabled(){return Ok(Value::Null);}let list=p["events"].as_array().ok_or("events required")?;if list.len()>100{return Err("At most 100 events per batch".into());}for v in list {let mut event:core::Event=serde_json::from_value(v.clone()).map_err(|e|e.to_string())?;
            if event.area=="startup" {let start=PROCESS_START.load(Ordering::Relaxed);if start>0 {if let (Some(origin),Some(duration))=(event.detail["timeOriginMs"].as_f64(),event.metrics["durationMs"].as_f64()){event.metrics["durationMs"]=json!((origin+duration-start as f64).max(0.0));}}}
            writer::record(||event);}Ok(Value::Null)},
        "info"=>{let binary=crate::daemon_binary::resolve_diagnostics_binary_path();let inbox=core::journal_view(&dir).issues;let today=chrono::Utc::now().format("%Y-%m-%d").to_string();let latest:Value=std::fs::read(dir.join("latest.json")).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);Ok(json!({"dir":dir,"bin":binary.as_ref().ok(),"binaryError":binary.as_ref().err(),"openIssues":inbox.values().filter(|i|i.status=="open").count(),"eventsToday":core::lines::<core::Event>(&dir.join(format!("events-{today}.ndjson"))).len(),"lastDigest":latest["lastDigest"],"dropped":writer::dropped()}))},
        "clear"=>{if p["confirmed"]!=true{return Err("Confirmation required".into());}writer::stop();if dir.exists(){std::fs::remove_dir_all(&dir).map_err(|e|e.to_string())?;}configure(dir,&state.app_settings.lock().await.clone())?;Ok(Value::Null)},
        "regenerate"=>{if !writer::enabled(){return Err("Enable diagnostics to regenerate the saved digest".into());}writer::flush()?;Ok(json!(std::fs::read_to_string(dir.join("digest.md")).map_err(|e|e.to_string())?))},
        "open"=>{use tauri_plugin_opener::OpenerExt;if dir.is_dir(){app.opener().open_path(dir.to_string_lossy().as_ref(),None::<&str>).map_err(|e|e.to_string())?;}Ok(Value::Null)},
        "diag_annotate"=>{if !writer::enabled(){return Err("Enable diagnostics to annotate from the app".into());}let result=Journal{dir}.query(&action,&p)?;writer::flush()?;Ok(result)},
        _=>Journal{dir}.query(&action,&p)
    }
}
