use super::*;
use std::sync::{atomic::{AtomicUsize,Ordering},Mutex as StdMutex};
#[derive(Clone,Default)] struct Output(Arc<StdMutex<Vec<Value>>>);
impl EventSink for Output {
 fn emit_app_server_event(&self,e:AppServerEvent){ self.0.lock().unwrap().push(e.message); }
 fn emit_terminal_output(&self,_:TerminalOutput){}
 fn emit_terminal_exit(&self,_:TerminalExit){}
}
struct Fake {root:PathBuf,sink:Sink,turns:AtomicUsize,stops:Arc<AtomicUsize>,delay:bool}
impl Driver for Fake {
 fn request<'a>(&'a self,w:&'a str,m:&'a str,p:Value)->BoxFuture<'a,Result<Value,String>> { Box::pin(async move {
  if m=="thread/start" {assert!(matches!(workspace(w).unwrap().kind,WorkspaceKind::Eval));return Ok(json!({"result":{"thread":{"id":"fake"}}}));}
  let n=self.turns.fetch_add(1,Ordering::SeqCst)+1;
  if self.delay { tokio::time::sleep(Duration::from_secs(10)).await; }
  if n>1 {
   assert!(p.to_string().contains("still fails"));
   let patch=Path::new(env!("CARGO_MANIFEST_DIR")).join("../evals/solutions/ts-bugfix-range.patch");
   let output=tokio::process::Command::new("git").arg("apply").arg(patch).current_dir(&self.root).output().await.unwrap();
   assert!(output.status.success());
  }
  for message in [
   json!({"id":"outside","method":"item/fileChange/requestApproval","params":{"threadId":"fake","cwd":self.root,"path":"/outside-eval/secret"}}),
   json!({"method":"turn/started","params":{"threadId":"fake","turn":{"id":n}}}),
   json!({"method":"item/started","params":{"threadId":"fake","item":{"type":"commandExecution"}}}),
   json!({"method":"item/agentMessage/delta","params":{"threadId":"fake","delta":"Done"}}),
   json!({"method":"item/completed","params":{"threadId":"fake","item":{"type":"agentMessage","text":"Done"}}}),
   json!({"method":"thread/tokenUsage/updated","params":{"threadId":"fake","tokenUsage":{"total":{"inputTokens":10*n,"cachedInputTokens":20*n,"outputTokens":5*n,"cost":0.01*n as f64},"last":{"cost":0.01}}}}),
   json!({"method":"turn/completed","params":{"threadId":"fake","turn":{"status":"completed"}}}),
  ] { self.sink.emit_app_server_event(AppServerEvent {workspace_id:w.into(),message}); }
  Ok(json!({"result":{}}))
 })}
 fn respond(&self,_:Value,p:Value)->BoxFuture<'_,Result<(),String>>{Box::pin(async move{assert_eq!(p["decision"],"decline");Ok(())})}
 fn stop(&self)->BoxFuture<'_,()>{Box::pin(async{self.stops.fetch_add(1,Ordering::SeqCst);})}
}
fn fixture(delay:bool)->(Arc<Evals>,Output,Arc<AtomicUsize>) {
 let root=std::env::temp_dir().join(format!("eval-test-{}",uuid::Uuid::new_v4()));
 let output=Output::default(); let emit=output.clone(); let stops=Arc::new(AtomicUsize::new(0)); let stopped=stops.clone();
 let factory:Factory=Arc::new(move |entry,_,sink|{let stops=stopped.clone();Box::pin(async move{Ok(Arc::new(Fake{root:PathBuf::from(entry.path),sink,turns:AtomicUsize::new(0),stops,delay}) as Arc<dyn Driver>)})});
 (Arc::new(Evals{data:root,emit:Arc::new(move|e|emit.emit_app_server_event(e)),active:Mutex::new(HashMap::new()),factory,prepare:Arc::new(|_,_|Box::pin(async {panic!("ready engines must not prepare")}))}),output,stops)
}
fn plan()->Plan { serde_json::from_value(json!({"engines":["pi"],"models":["lab/model"],"ids":["ts-bugfix-range"],"concurrency":1})).unwrap() }
async fn start(service:&Arc<Evals>,plan:Plan)->String {
 service.start(AppSettings::default(),plan,json!([{"engine":"pi","state":"ready","version":"fake"}])).await.unwrap()["runId"].as_str().unwrap().into()
}
async fn finished(service:&Arc<Evals>,id:&str)->Run {
 tokio::time::timeout(Duration::from_secs(15),async {
  loop {if !service.active.lock().await.contains_key(id){return service.read(id).unwrap();}tokio::time::sleep(Duration::from_millis(10)).await;}
 }).await.unwrap()
}
#[tokio::test] async fn lifecycle_usage_nudge_persistence_cleanup() {
 let (service,output,stops)=fixture(false); let id=start(&service,plan()).await;
 let run=finished(&service,&id).await;
 assert_eq!(run.status,"completed"); let cell=&run.cells[0]; assert_eq!(cell.status,"passed"); assert_eq!(cell.turns,2);
 assert_eq!(cell.usage.tokens(),70); assert_eq!(cell.usage.cached_tokens,40); assert_eq!(cell.usage.cost_usd,Some(0.02)); assert_eq!(cell.usage.tool_calls,2);
 assert!(cell.usage.first_token_ms.is_some()); assert!(cell.workdir.is_none()); assert_eq!(stops.load(Ordering::SeqCst),1);
 assert!(output.0.lock().unwrap().iter().any(|e|e["params"]["cells"][0]["status"]=="running"));
 assert!(cell.transcript.iter().any(|e|e["method"]=="eval/prompt"));
 assert_eq!(cell.usage.errors.iter().filter(|e|e.contains("Denied approval")).count(),2);
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[tokio::test] async fn budget_stops_and_retains_failed_work() {
 let (service,_,_)=fixture(false); let mut p=plan();p.repeats=3;p.max_total_tokens=Some(1);
 let id=start(&service,p).await; let run=finished(&service,&id).await;
 assert_eq!(run.status,"budget-stopped");assert!(run.cells.iter().skip(1).all(|c|c.status=="cancelled"));
 assert!(run.cells[0].usage.tokens()>=35);assert!(Path::new(run.cells[0].workdir.as_ref().unwrap()).is_dir());
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[tokio::test] async fn cancellation_stops_inflight_and_keep_flag() {
 let (service,_,stops)=fixture(true); let mut p=plan();p.keep_workdirs=Some(false);
 let id=start(&service,p).await;
 tokio::time::sleep(Duration::from_millis(300)).await;
 service.request(&AppSettings::default(),"cancel",json!({"runId":id})).await.unwrap();
 let run=finished(&service,&id).await;assert_eq!(run.status,"cancelled"); assert_eq!(run.cells[0].status,"cancelled");
 assert_eq!(stops.load(Ordering::SeqCst),1); assert!(run.cells[0].workdir.is_none());
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[test] fn sandbox_and_usage_unknown() {
 let root=std::env::temp_dir().join(format!("eval-path-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
 assert!(approval(&root,&json!({"path":root.join("new.txt")})));
 assert!(!approval(&root,&json!({"path":"/etc/passwd"})));assert!(!approval(&root,&json!({"path":"../outside"})));
 assert!(!approval(&root,&json!({"cwd":root,"command":"cat /etc/passwd"})));
 assert!(!approval(&root,&json!({"cwd":root,"toolCall":{"rawInput":{"command":"cat /etc/passwd"}}})));
 assert!(!approval(&root,&json!({"cwd":root})));
 #[cfg(unix)] {std::os::unix::fs::symlink(root.parent().unwrap(),root.join("escape")).unwrap();assert!(!approval(&root,&json!({"path":"escape/new"})));}
 let usage=taucode_diag_mcp::observe::normalize_usage(&json!({"inputTokens":20,"cost":0.}),&json!({"cost":null}),&json!({"inputTokens":10}),false);
 assert_eq!(usage["tokensIn"],10);assert!(usage["costUsd"].is_null());
 std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test] async fn gate_thresholds_and_invalid_plans() {
 let (service,_,_)=fixture(false);let id=start(&service,plan()).await;let baseline=finished(&service,&id).await;let mut candidate=baseline.clone();
 assert_eq!(gate(&baseline,&candidate,0.1,0.25).unwrap()["passed"],true);
 candidate.cells[0].status="failed".into();assert_eq!(gate(&baseline,&candidate,0.1,0.25).unwrap()["passed"],false);
 candidate=baseline.clone();candidate.cells[0].usage.tokens_in=200;assert_eq!(gate(&baseline,&candidate,0.1,0.25).unwrap()["passed"],false);
 candidate.cells.clear();assert!(gate(&baseline,&candidate,0.1,0.25).is_err());
 let mut p=plan();p.concurrency=7;assert!(p.validate().is_err());assert!(safe_id("../secret").is_err());
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[tokio::test] async fn check_environment_timeout_and_retention() {
 let (service,_,_)=fixture(false);let mut p=plan();p.keep_workdirs=Some(true);
 let id=start(&service,p).await;let run=finished(&service,&id).await;assert!(run.cells[0].workdir.is_some());
 let root=Path::new(run.cells[0].workdir.as_ref().unwrap());
 assert!(process::command(root,"sleep 5",1).await.unwrap_err().contains("timed out"));
 let (_,out)=process::command(root,"env",1).await.unwrap();assert!(!out.contains("API_KEY"));assert!(!out.contains("TOKEN="));
 service.request(&AppSettings::default(),"delete",json!({"runId":id})).await.unwrap();assert!(!root.exists());
 std::fs::remove_dir_all(&service.data).unwrap();
}


#[test] fn codegraph_override_is_private_and_removed_on_drop() {
 let data=std::env::temp_dir().join(format!("eval-registry-{}",uuid::Uuid::new_v4()));
 std::fs::create_dir_all(&data).unwrap();
 let mut settings=AppSettings::default();
 settings.mcp_servers.iter_mut().find(|c|c.id=="codegraph").unwrap().enabled=false;
 crate::storage::write_settings(&data.join("settings.json"),&settings).unwrap();
 let before=std::fs::read(data.join("settings.json")).unwrap();
 let mut cg=crate::types::default_mcp_servers().into_iter().find(|c|c.id=="codegraph").unwrap();cg.enabled=true;
 let id=format!("eval-{}",uuid::Uuid::new_v4());
 let guard=WorkspaceGuard::register(WorkspaceEntry{id:id.clone(),name:"test".into(),path:data.to_string_lossy().into(),kind:WorkspaceKind::Eval,parent_id:None,worktree:None,
     settings:WorkspaceSettings{codegraph_enabled:Some(true),mcp_servers:vec![cg],..Default::default()}});
 let graph=crate::codegraph::CodegraphSetup{proxy_exe:"proxy".into(),upstream:vec!["codegraph".into()],state_dir:data.join("graph")};
 assert!(super::super::connectors::load(&data,&id,Some(&graph)).unwrap().iter().find(|c|c.id=="codegraph").unwrap().enabled);
 assert_eq!(std::fs::read(data.join("settings.json")).unwrap(),before);
 assert!(!data.join("workspaces.json").exists());
 drop(guard);assert!(workspace(&id).is_none());
 std::fs::remove_dir_all(data).unwrap();
}
#[tokio::test] async fn failed_directories_are_capped_at_twenty() {
 let (service,_,_)=fixture(false);let id=start(&service,plan()).await;let mut run=finished(&service,&id).await;
 let template=run.cells[0].clone();
 run.cells=(0..22).map(|i|{
  let mut c=template.clone();c.id=format!("retained-{i}");c.status="failed".into();c.completed_at=Some(format!("2026-10-25T12:00:{i:02}Z"));
  let dir=service.data.join("evals/work").join(&id).join(&c.id);std::fs::create_dir_all(&dir).unwrap();c.workdir=Some(dir.to_string_lossy().into());c
 }).collect();
 service.save(&run).unwrap();service.prune().await;
 let stored=service.read(&id).unwrap();
 assert_eq!(stored.cells.iter().filter(|c|c.workdir.is_some()).count(),20);
 assert!(stored.cells[0].workdir.is_none());assert!(stored.cells[21].workdir.is_some());
 assert_eq!(std::fs::read_dir(service.data.join("evals/work").join(&id)).unwrap().count(),20);
 std::fs::remove_dir_all(&service.data).unwrap();
}

fn installed_runtime(data:&Path,engine:&str,version:&str) {
 let (package,entry)=match engine {"pi"=>("@earendil-works/pi-coding-agent","dist/bundle/cli.js"),"omp"=>("@oh-my-pi/pi-coding-agent","dist/cli.js"),_=>panic!("unexpected test engine")};
 let modules=data.join("engines").join(engine).join("node_modules");
 let cli=modules.join(package).join(entry);
 std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
 std::fs::write(cli,"").unwrap();
 std::fs::write(modules.join(package).join("package.json"),json!({"version":version}).to_string()).unwrap();
 if engine=="omp" {std::fs::create_dir_all(modules.join(".bin")).unwrap();std::fs::write(modules.join(".bin/bun"),"").unwrap();}
}
#[tokio::test] async fn pending_engine_environment_records_installed_version() {
 let (mut service,_,_)=fixture(false);
 Arc::get_mut(&mut service).unwrap().prepare=Arc::new(|data,engine|Box::pin(async move {
  let path=std::fs::read_dir(data.join("evals/runs")).unwrap().next().unwrap().unwrap().path();
  let before:Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
  assert_eq!(before["environment"]["engines"][0]["state"],"pending");
  assert!(before["environment"]["engines"][0]["version"].is_null());
  installed_runtime(&data,&engine,"99.0.0-eval-fixture");
  Ok(())
 }));
 let result=service.start(AppSettings::default(),plan(),json!([{"engine":"pi","state":"pending"},{"engine":"custom","state":"ready","version":"custom-1"}])).await.unwrap();
 let run=finished(&service,result["runId"].as_str().unwrap()).await;
 assert_eq!(run.status,"completed");
 let stored:Value=serde_json::from_slice(&std::fs::read(service.path(&run.id).unwrap()).unwrap()).unwrap();
 assert_eq!(stored["environment"]["engines"][0]["state"],"ready");
 assert_eq!(stored["environment"]["engines"][0]["version"],"99.0.0-eval-fixture");
 assert_eq!(stored["environment"]["engines"][1]["version"],"custom-1");
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[tokio::test] async fn pending_engines_prepare_once_before_any_cells() {
 let (mut service,output,_)=fixture(false);
 let calls=Arc::new(StdMutex::new(Vec::new()));let seen=calls.clone();
 Arc::get_mut(&mut service).unwrap().prepare=Arc::new(move|data,engine|{
  let seen=seen.clone();Box::pin(async move {
   assert!(!data.join("evals/work").exists(),"no cell may start while preparing");
   seen.lock().unwrap().push(engine.clone());
   installed_runtime(&data,&engine,"prepared");
   Ok(())
  })
 });
 let mut p=plan();p.engines=vec!["pi".into(),"omp".into()];p.repeats=2;p.concurrency=4;
 let mut settings=AppSettings::default();
 let mut omp=super::super::engine_registry::resolve(&settings,"omp").unwrap().0;omp.enabled=true;settings.engines.push(omp);
 let result=service.start(settings,p,json!([{"engine":"pi","state":"pending"},{"engine":"omp","state":"pending"}])).await.unwrap();
 assert_eq!(result["status"],"preparing");
 let run=finished(&service,result["runId"].as_str().unwrap()).await;
 assert_eq!(*calls.lock().unwrap(),vec!["pi","omp"]);
 assert_eq!(run.status,"completed");assert!(run.cells.iter().all(|c|c.status=="passed"));
 for engine in ["pi","omp"] {assert!(output.0.lock().unwrap().iter().any(|e|e["params"]["message"]==format!("Preparing {engine}…")));}
 assert!(run.environment["engines"].as_array().unwrap().iter().all(|r|r["version"]=="prepared"));
 std::fs::remove_dir_all(&service.data).unwrap();
}
#[tokio::test] async fn engine_errors_and_failed_preparation_are_explicit() {
 let (mut service,_,stops)=fixture(false);
 for state in ["error","missing"] {
  let runtime=if state=="missing" {json!([])} else {json!([{"engine":"pi","state":"error","message":"bad executable"}])};
  let error=service.start(AppSettings::default(),plan(),runtime).await.unwrap_err();
  assert!(error.contains("pi") && error.contains(state),"{error}");
 }
 Arc::get_mut(&mut service).unwrap().prepare=Arc::new(|_,_|Box::pin(async{Err("offline package unavailable".into())}));
 let result=service.start(AppSettings::default(),plan(),json!([{"engine":"pi","state":"pending"}])).await.unwrap();
 let run=finished(&service,result["runId"].as_str().unwrap()).await;
 assert_eq!(run.status,"failed");
 assert!(run.message.unwrap().contains("Engine pi: preparation failed — offline package unavailable"));
 assert!(run.cells.iter().all(|c|c.turns==0 && c.status=="cancelled"));
 assert_eq!(stops.load(Ordering::SeqCst),0);assert!(!service.data.join("evals/work").exists());
 std::fs::remove_dir_all(&service.data).unwrap();
}
