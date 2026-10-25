mod model;
mod process;
#[cfg(test)] mod tests;
pub(crate) use model::*;
use crate::{backend::events::{AppServerEvent,EventSink,TerminalOutput,TerminalExit}, types::{AppSettings,WorkspaceEntry,WorkspaceKind,WorkspaceSettings}};
use super::{engines::WorkspaceSession,providers::BoxFuture};
use futures_util::{stream::FuturesUnordered,StreamExt};
use serde_json::{json,Value};
use std::{collections::{HashMap,VecDeque},path::{Path,PathBuf},sync::Arc,time::{Duration,Instant}};
use tokio::sync::{mpsc,watch,Mutex};

static WORKSPACES: std::sync::OnceLock<std::sync::Mutex<HashMap<String,WorkspaceEntry>>> = std::sync::OnceLock::new();
pub(crate) fn workspace(id:&str)->Option<WorkspaceEntry> {
    WORKSPACES.get_or_init(Default::default).lock().unwrap().get(id).cloned()
}
struct WorkspaceGuard(String);
impl WorkspaceGuard {
    fn register(entry:WorkspaceEntry)->Self {
        let id=entry.id.clone();
        WORKSPACES.get_or_init(Default::default).lock().unwrap().insert(id.clone(),entry);
        Self(id)
    }
}
impl Drop for WorkspaceGuard {
    fn drop(&mut self) { WORKSPACES.get_or_init(Default::default).lock().unwrap().remove(&self.0); }
}

#[derive(Clone)]
struct Sink(mpsc::UnboundedSender<AppServerEvent>);
impl EventSink for Sink {
    fn emit_app_server_event(&self,event:AppServerEvent) { let _=self.0.send(event); }
    fn emit_terminal_output(&self,_:TerminalOutput) {}
    fn emit_terminal_exit(&self,_:TerminalExit) {}
}
trait Driver: Send+Sync {
    fn request<'a>(&'a self,workspace:&'a str,method:&'a str,params:Value)->BoxFuture<'a,Result<Value,String>>;
    fn respond(&self,id:Value,params:Value)->BoxFuture<'_,Result<(),String>>;
    fn stop(&self)->BoxFuture<'_,()>;
}
impl Driver for WorkspaceSession {
    fn request<'a>(&'a self,w:&'a str,m:&'a str,p:Value)->BoxFuture<'a,Result<Value,String>> { Box::pin(self.send_request_for_workspace(w,m,p)) }
    fn respond(&self,id:Value,p:Value)->BoxFuture<'_,Result<(),String>> { Box::pin(self.send_response(id,p)) }
    fn stop(&self)->BoxFuture<'_,()> { Box::pin(self.shutdown()) }
}
type Factory=Arc<dyn Fn(WorkspaceEntry,AppSettings,Sink)->BoxFuture<'static,Result<Arc<dyn Driver>,String>>+Send+Sync>;
type Prepare=Arc<dyn Fn(PathBuf,String)->BoxFuture<'static,Result<(),String>>+Send+Sync>;
pub(crate) struct Evals {
    data:PathBuf, emit:Arc<dyn Fn(AppServerEvent)+Send+Sync>,
    active:Mutex<HashMap<String,watch::Sender<bool>>>, factory:Factory, prepare:Prepare,
}
impl Evals {
    pub fn new(data:PathBuf,sink:impl EventSink)->Arc<Self> {
        let dir=data.clone();
        let factory:Factory=Arc::new(move |entry,settings,sink| {
            let data=dir.clone();
            Box::pin(async move {
                let graph=if entry.settings.codegraph_enabled==Some(true) { crate::codegraph::discover(data.clone()) } else {None};
                let session=crate::backend::app_server::spawn_workspace_session(entry,settings.codex_bin.clone(),settings.codex_args.clone(),Some(data.join("engines/codex-home")),env!("CARGO_PKG_VERSION").into(),sink,graph,settings).await?;
                Ok(session as Arc<dyn Driver>)
            })
        });
        let prepare:Prepare=Arc::new(|data,engine|Box::pin(async move {
            super::engine_runtime::prepare(&data,&engine,None).await.map(|_|())
        }));
        Arc::new(Self {data,emit:Arc::new(move |e|sink.emit_app_server_event(e)),active:Mutex::new(HashMap::new()),factory,prepare})
    }
    fn path(&self,id:&str)->Result<PathBuf,String> { safe_id(id)?; Ok(self.data.join("evals/runs").join(format!("{id}.json"))) }
    fn read(&self,id:&str)->Result<Run,String> {
        serde_json::from_slice(&std::fs::read(self.path(id)?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
    }
    fn save(&self,run:&Run)->Result<(),String> {
        let path=self.path(&run.id)?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
        super::pi::config::write_json(&path,&json!(run))?;
        (self.emit)(AppServerEvent {workspace_id:"evals".into(),message:json!({"method":"eval/progress","params":{"runId":run.id,"status":run.status,"message":run.message,"cells":run.cells}})});
        Ok(())
    }
    fn runs(&self)->Result<Vec<Run>,String> {
        let path=self.data.join("evals/runs");
        if !path.exists() { return Ok(vec![]); }
        let mut runs=Vec::new();
        for entry in std::fs::read_dir(path).map_err(|e|e.to_string())? {
            let path=entry.map_err(|e|e.to_string())?.path();
            if path.extension().is_some_and(|e|e=="json") {
                let run:Run=serde_json::from_slice(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                runs.push(run);
            }
        }
        runs.sort_by(|a,b|b.created_at.cmp(&a.created_at));
        Ok(runs)
    }
    pub async fn request(self:&Arc<Self>,settings:&AppSettings,action:&str,p:Value)->Result<Value,String> {
        match action {
            "suites"=>Ok(json!(suite().tasks)),
            "plan"=>{
                let plan:Plan=serde_json::from_value(p).map_err(|e|e.to_string())?; plan.validate()?;
                let tasks=plan.tasks().len(); let cells=tasks*plan.engines.len()*plan.models.len()*plan.codegraph.len()*plan.repeats;
                let previous=self.runs()?.into_iter().find(|r|r.status=="completed");
                let estimate=previous.map(|r|median(r.cells.iter().filter(|c|c.turns>0).map(|c|c.usage.tokens() as f64).collect())*cells as f64);
                Ok(json!({"tasks":tasks,"cells":cells,"estimatedTokens":estimate}))
            },
            "start"=>{
                let plan:Plan=serde_json::from_value(p).map_err(|e|e.to_string())?; plan.validate()?;
                let mut runtime=super::engine_runtime::status(&self.data).await;
                for engine in super::engine_registry::registry(settings).iter().filter(|e|!e.built_in && e.enabled && plan.engines.contains(&e.id)) {
                    let status=super::engine_registry::test(&self.data,engine).await.unwrap_or_else(|error|json!({"engine":engine.id,"state":"error","message":error}));
                    runtime.as_array_mut().unwrap().push(status);
                }
                self.start(settings.clone(),plan,runtime).await
            },
            "status"|"run"=>{
                let id=p["runId"].as_str().ok_or("Missing runId")?;
                let mut run=self.read(id)?;
                if matches!(run.status.as_str(),"running"|"preparing") && !self.active.lock().await.contains_key(id) { run.status="interrupted".into(); self.save(&run)?; }
                Ok(json!({"summary":summary(&run),"run":run}))
            },
            "cancel"=>{
                let id=p["runId"].as_str().ok_or("Missing runId")?;
                if let Some(cancel)=self.active.lock().await.get(id) { let _=cancel.send(true); }
                Ok(json!({}))
            },
            "runs"=>Ok(json!(self.runs()?.iter().map(|r|json!({"id":r.id,"createdAt":r.created_at,"status":r.status,"plan":r.plan,"summary":summary(r)})).collect::<Vec<_>>())),
            "delete"=>{
                let id=p["runId"].as_str().ok_or("Missing runId")?;
                if self.active.lock().await.contains_key(id) { return Err("Cancel and wait before deleting a run".into()); }
                std::fs::remove_file(self.path(id)?).map_err(|e|e.to_string())?;
                let work=self.data.join("evals/work").join(id);
                if work.exists() { std::fs::remove_dir_all(work).map_err(|e|e.to_string())?; }
                Ok(json!({}))
            },
            "gate"=>{
                let mut candidate=self.read(p["runId"].as_str().ok_or("Missing runId")?)?;
                let baseline=p["baseline"].as_str().ok_or("Missing baseline")?;
                let mut old=if baseline=="latest-passing" {
                    self.runs()?.into_iter().find(|r|r.id!=candidate.id && r.status=="completed" && r.cells.iter().any(|c|c.status=="passed") && r.cells.iter().all(|c|matches!(c.status.as_str(),"passed"|"skipped"))).ok_or("No passing baseline")?
                } else {self.read(baseline)?};
                if let Some(tags)=p["tags"].as_array().filter(|a|!a.is_empty()) {
                    let ids:Vec<_>=suite().tasks.into_iter().filter(|t|t.tags.iter().any(|s|tags.contains(&json!(s)))).map(|t|t.id).collect();
                    old.cells.retain(|c|ids.contains(&c.task)); candidate.cells.retain(|c|ids.contains(&c.task));
                }
                gate(&old,&candidate,p["maxPassDrop"].as_f64().unwrap_or(0.1),p["maxTokenIncrease"].as_f64().unwrap_or(0.25))
            },
            _=>Err("Unknown eval action".into()),
        }
    }
    async fn start(self:&Arc<Self>,settings:AppSettings,plan:Plan,runtime:Value)->Result<Value,String> {
        let registry=super::engine_registry::registry(&settings);
        let connectors=super::connectors::effective(&settings,&WorkspaceSettings::default(),None)?;
        let mut preparing=Vec::new();
        for engine in &plan.engines {
            let state=runtime.as_array().and_then(|rows|rows.iter().find(|r|r["engine"]==if engine=="claude" {"claude-acp"} else {engine}));
            let enabled=registry.iter().any(|e|e.id==*engine && e.enabled);
            let name=state.and_then(|s|s["state"].as_str()).unwrap_or("missing");
            if !enabled || !matches!(name,"ready"|"pending") {
                return Err(format!("Engine {engine}: {}{}",if enabled {name} else {"disabled"},state.and_then(|s|s["message"].as_str()).map(|s|format!(" — {s}")).unwrap_or_default()));
            }
            if name=="pending" { preparing.push(engine.clone()); }
        }
        let id=uuid::Uuid::new_v4().to_string();
        let mut cells=Vec::new();
        for task in plan.tasks() { for engine in &plan.engines { for model in &plan.models { for graph in &plan.codegraph { for repeat in 0..plan.repeats {
            let supported=task.requires.iter().all(|r| match r.as_str() {
                "browser"=>settings.features().browser && connectors.iter().any(|s|s.id=="taucode-browser" && s.enabled),
                "computerUse"=>settings.features().computer_use && connectors.iter().any(|s|s.id=="taucode-computer" && s.enabled),
                _=>false,
            });
            cells.push(Cell {id:format!("cell-{}",cells.len()+1),task:task.id.clone(),engine:engine.clone(),model:model.clone(),codegraph:*graph,repeat,
                status:if supported {"queued"} else {"skipped"}.into(),turns:0,thread_id:None,
                output:if !supported {"Required feature/connector unavailable"} else {""}.into(),
                answer:String::new(),usage:Usage::default(),transcript:vec![],workdir:None,completed_at:None});
        }}}}}
        let graph_version=if plan.codegraph.contains(&true) {
            let output=tokio::time::timeout(Duration::from_secs(3),tokio::process::Command::new("codegraph").arg("--version").kill_on_drop(true).output()).await;
            output.ok().and_then(Result::ok).filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_owned())
        } else {None};
        let mut run=Run {id:id.clone(),created_at:chrono::Utc::now().to_rfc3339(),status:if preparing.is_empty() {"running"} else {"preparing"}.into(),
            message:preparing.first().map(|e|format!("Preparing {e}…")),
            environment:json!({"appVersion":env!("CARGO_PKG_VERSION"),"engines":runtime,"codegraphVersion":graph_version,"models":plan.models}),plan,cells};
        self.save(&run)?;
        let (cancel,_)=watch::channel(false);
        self.active.lock().await.insert(id.clone(),cancel.clone());
        let this=self.clone();
        let response=json!({"runId":id,"status":run.status,"message":run.message});
        tokio::spawn(async move {
            let result=match this.prepare_run(&mut run,&preparing,&cancel).await {
                Ok(())=>this.drive(run,settings,cancel).await,
                Err(error)=>{
                    run.status=if *cancel.borrow() {"cancelled"} else {"failed"}.into();
                    run.message=Some(error.clone());
                    for cell in &mut run.cells { if cell.status=="queued" {cell.status="cancelled".into();cell.output=error.clone();} }
                    this.save(&run)
                }
            };
            if let Err(error)=result { (this.emit)(AppServerEvent {workspace_id:"evals".into(),message:json!({"method":"eval/progress","params":{"runId":id,"status":"error","error":error}})}); }
            this.active.lock().await.remove(&id);
            this.prune().await;
        });
        Ok(response)
    }
    async fn prepare_run(&self,run:&mut Run,engines:&[String],cancel:&watch::Sender<bool>)->Result<(),String> {
        let mut cancelled=cancel.subscribe();
        for engine in engines {
            if *cancelled.borrow() {return Err("Run cancelled during preparation".into());}
            run.message=Some(format!("Preparing {engine}…"));self.save(run)?;
            let runtime_id=if engine=="claude" {"claude-acp"} else {engine};
            let mut prepare=(self.prepare)(self.data.clone(),runtime_id.into());
            tokio::select! {
                _=cancelled.changed()=>{
                    super::engine_runtime::cancel(runtime_id);
                    let _=tokio::time::timeout(Duration::from_secs(5),&mut prepare).await;
                    return Err("Run cancelled during preparation".into());
                },
                status=&mut prepare=>status.map_err(|e|format!("Engine {engine}: preparation failed — {e}"))?,
            };
        }
        if !engines.is_empty() {
            let runtime=super::engine_runtime::status(&self.data).await;
            // Preserve custom-engine handshake rows while refreshing installed runtimes.
            for row in run.environment["engines"].as_array_mut().into_iter().flatten() {
                if let Some(fresh)=runtime.as_array().and_then(|rows|rows.iter().find(|r|r["engine"]==row["engine"])) { *row=fresh.clone(); }
            }
            for engine in engines {
                let runtime_id=if engine=="claude" {"claude-acp"} else {engine};
                let status=runtime.as_array().and_then(|rows|rows.iter().find(|r|r["engine"]==runtime_id));
                if status.is_none_or(|s|s["state"]!="ready") {
                    return Err(format!("Engine {engine}: preparation ended in {} — {}",status.and_then(|s|s["state"].as_str()).unwrap_or("missing"),status.and_then(|s|s["message"].as_str()).unwrap_or("")));
                }
            }
        }
        run.status="running".into();run.message=None;self.save(run)
    }
    async fn drive(self:&Arc<Self>,mut run:Run,settings:AppSettings,cancel:watch::Sender<bool>)->Result<(),String> {
        let mut pending:VecDeque<_>=run.cells.iter().filter(|c|c.status=="queued").cloned().collect();
        let mut futures=FuturesUnordered::new();
        let (updates,mut rx)=mpsc::unbounded_channel();
        let mut cancelled=cancel.subscribe();
        let mut budget=false;
        loop {
            while !*cancel.borrow() && futures.len()<run.plan.concurrency {
                let Some(cell)=pending.pop_front() else {break};
                futures.push(self.cell(run.id.clone(),cell,settings.clone(),run.plan.keep_workdirs,cancel.subscribe(),updates.clone()));
            }
            if futures.is_empty() { break; }
            tokio::select! {
                Some(cell)=futures.next()=>{ let i=run.cells.iter().position(|c|c.id==cell.id).unwrap(); run.cells[i]=cell; },
                Some(cell)=rx.recv()=>{
                    let i=run.cells.iter().position(|c|c.id==cell.id).unwrap();
                    if matches!(run.cells[i].status.as_str(),"queued"|"running") { run.cells[i]=cell; }
                },
                _=cancelled.changed(), if !*cancelled.borrow()=>{},
            }
            let tokens=run.cells.iter().map(|c|c.usage.tokens()).sum::<u64>();
            let cost=run.cells.iter().filter_map(|c|c.usage.cost_usd).sum::<f64>();
            if run.plan.max_total_tokens.is_some_and(|n|tokens>=n) || run.plan.max_cost.is_some_and(|n|cost>=n) {
                budget=true; let _=cancel.send(true);
            }
            if let Err(error)=self.save(&run) { let _=cancel.send(true); while futures.next().await.is_some() {} return Err(error); }
        }
        for cell in &mut run.cells { if cell.status=="queued" { cell.status="cancelled".into(); cell.output=if budget {"Budget exceeded"} else {"Run cancelled"}.into(); } }
        run.status=if budget {"budget-stopped"} else if *cancel.borrow() {"cancelled"} else {"completed"}.into();
        self.save(&run)
    }
    async fn cell(&self,run:String,mut cell:Cell,settings:AppSettings,keep:Option<bool>,mut cancel:watch::Receiver<bool>,updates:mpsc::UnboundedSender<Cell>)->Cell {
        if *cancel.borrow() { cell.status="cancelled".into(); return cell; }
        let root=self.data.join("evals/work").join(run).join(&cell.id);
        let task=suite().tasks.into_iter().find(|t|t.id==cell.task).unwrap();
        let started=Instant::now();
        cell.status="running".into(); cell.workdir=Some(root.to_string_lossy().into()); let _=updates.send(cell.clone());
        let (tx,mut events)=mpsc::unbounded_channel();
        let mut driver:Option<Arc<dyn Driver>>=None;
        let mut workspace_guard=None;
        let mut deadline=Box::pin(tokio::time::sleep(Duration::from_secs(task.timeout_sec)));
        let result=tokio::select! {
            _=cancel.changed()=>Err("Cancelled".into()),
            _=&mut deadline=>Err("Task timed out".into()),
            result=async {
                process::materialize(&root,&task).await?;
                if cell.codegraph {
                    if !settings.features().connectors { return Err("Enable connectors to evaluate Codegraph".into()); }
                    if crate::codegraph::discover(self.data.clone()).is_none() { return Err("Codegraph unavailable".into()); }
                    let (code,out)=process::command(&root,"CODEGRAPH_DIR=.codegraph-taucode codegraph init",task.timeout_sec).await?;
                    if code!=0 { return Err(out); }
                }
                let mut cg=crate::types::default_mcp_servers().into_iter().find(|c|c.id=="codegraph").unwrap();
                cg.enabled=cell.codegraph;
                let entry=WorkspaceEntry {id:format!("eval-{}",uuid::Uuid::new_v4()),name:cell.task.clone(),path:root.to_string_lossy().into(),
                    kind:WorkspaceKind::Eval,parent_id:None,worktree:None,settings:WorkspaceSettings {mcp_servers:vec![cg],codegraph_enabled:Some(cell.codegraph),..Default::default()}};
                let workspace=entry.id.clone();
                workspace_guard=Some(WorkspaceGuard::register(entry.clone()));
                let backend=(self.factory)(entry,settings,Sink(tx)).await?;
                driver=Some(backend.clone());
                let server=if task.repo=="web-page" {Some(process::serve(&root).await?)} else {None};
                let prompt=task.prompt.replace("{{url}}",server.as_ref().map(|s|s.url.as_str()).unwrap_or(""));
                self.execute(&*backend,&workspace,&root,&task,&mut cell,prompt,&mut events,&updates).await
            }=>result,
        };
        if let Some(driver)=driver { let _=tokio::time::timeout(Duration::from_secs(5),driver.stop()).await; }
        drop(workspace_guard);
        if let Err(error)=result { cell.status=if *cancel.borrow() {"cancelled"} else {"failed"}.into(); cell.usage.errors.push(error.clone()); cell.output=error; }
        cell.usage.wall_ms=started.elapsed().as_millis() as u64;
        cell.completed_at=Some(chrono::Utc::now().to_rfc3339());
        if keep==Some(false) || (keep.is_none() && cell.status=="passed") {
            if let Err(error)=std::fs::remove_dir_all(&root) { if root.exists() { cell.usage.errors.push(format!("Cleanup: {error}")); } }
            if !root.exists() { cell.workdir=None; }
        }
        cell
    }
    #[allow(clippy::too_many_arguments)]
    async fn execute(&self,driver:&dyn Driver,workspace:&str,root:&Path,task:&Task,cell:&mut Cell,mut prompt:String,events:&mut mpsc::UnboundedReceiver<AppServerEvent>,updates:&mpsc::UnboundedSender<Cell>)->Result<(),String> {
        let start=Instant::now();
        let response=driver.request(workspace,"thread/start",json!({"cwd":root,"engine":cell.engine,"model":cell.model,"approvalPolicy":"on-request"})).await?;
        let id=response.pointer("/result/thread/id").or_else(||response.pointer("/thread/id")).and_then(Value::as_str).ok_or_else(||format!("Start failed: {response}"))?.to_owned();
        cell.thread_id=Some(id.clone()); cell.usage.engine_ready_ms=start.elapsed().as_millis() as u64;
        let mut transcript_bytes=0;
        let mut total=json!({});
        let mut known_cost=true;
        let mut zero_cost=false;
        for turn in 0..task.max_turns {
            let baseline=total.clone(); let mut metrics=json!({});
            let previous=cell.usage.clone();
            cell.turns=turn+1; cell.answer.clear();
            let turn_start=Instant::now();
            cell.transcript.push(json!({"method":"eval/prompt","params":{"text":prompt}}));
            let response=driver.request(workspace,"turn/start",json!({"threadId":id,"cwd":root,"accessMode":"full-access","model":cell.model,"input":[{"type":"text","text":prompt}]})).await?;
            if response.get("error").is_some() { return Err(response["error"].to_string()); }
            loop {
                let event=events.recv().await.ok_or("Engine event stream closed")?;
                let msg=event.message; let p=&msg["params"];
                if p["threadId"].as_str().is_some_and(|thread|thread!=id) { continue; }
                let method=msg["method"].as_str().unwrap_or("");
                if method=="turn/started" { zero_cost=p["zeroCost"]==true; }
                // Persist the full transcript for read-only inspection, bounded to 4 MB per cell.
                let size=msg.to_string().len();
                if transcript_bytes+size<=4*1024*1024 { transcript_bytes+=size; cell.transcript.push(msg.clone()); }
                if msg.get("id").is_some() {
                    let allowed=approval(root,p);
                    if !allowed { cell.usage.errors.push(format!("Denied approval outside eval scope: {method}")); }
                    driver.respond(msg["id"].clone(),json!({"decision":if allowed {"accept"} else {"decline"}})).await?;
                }
                if method=="item/agentMessage/delta" && cell.usage.first_token_ms.is_none() { cell.usage.first_token_ms=Some(turn_start.elapsed().as_millis() as u64); }
                if method=="item/completed" && p["item"]["type"]=="agentMessage" {
                    if let Some(text)=p["item"]["text"].as_str() { cell.answer=text.to_owned(); }
                }
                if method=="item/started" && matches!(p["item"]["type"].as_str(),Some("mcpToolCall"|"dynamicToolCall"|"commandExecution"|"fileChange")) { cell.usage.tool_calls+=1; }
                if method=="error" || (method=="item/completed" && (p["item"]["status"]=="failed" || p["item"]["success"]==false || !p["item"]["error"].is_null())) { cell.usage.errors.push(process::tail(&p.to_string())); }
                if method=="thread/tokenUsage/updated" {
                    metrics=taucode_diag_mcp::observe::normalize_usage(&p["tokenUsage"]["total"],&p["tokenUsage"]["last"],&baseline,zero_cost);
                    total=p["tokenUsage"]["total"].clone();
                    let mut snapshot=cell.clone();
                    snapshot.usage.tokens_in=previous.tokens_in; snapshot.usage.cached_tokens=previous.cached_tokens; snapshot.usage.tokens_out=previous.tokens_out;
                    snapshot.usage.tokens_in+=metrics["tokensIn"].as_u64().unwrap_or(0);
                    snapshot.usage.cached_tokens+=metrics["cachedTokens"].as_u64().unwrap_or(0);
                    snapshot.usage.tokens_out+=metrics["tokensOut"].as_u64().unwrap_or(0);
                    snapshot.usage.cost_usd=if known_cost {metrics["costUsd"].as_f64().map(|n|previous.cost_usd.unwrap_or(0.)+n)} else {None};
                    cell.usage=snapshot.usage.clone();
                    let _=updates.send(snapshot);
                }
                if method=="turn/completed" {
                    if p["turn"]["status"]=="failed" { cell.usage.errors.push(p["turn"]["error"].to_string()); }
                    break;
                }
            }
            cell.usage.tokens_in=previous.tokens_in+metrics["tokensIn"].as_u64().unwrap_or(0);
            cell.usage.cached_tokens=previous.cached_tokens+metrics["cachedTokens"].as_u64().unwrap_or(0);
            cell.usage.tokens_out=previous.tokens_out+metrics["tokensOut"].as_u64().unwrap_or(0);
            known_cost &= metrics["costUsd"].as_f64().is_some();
            cell.usage.cost_usd=if known_cost {Some(previous.cost_usd.unwrap_or(0.)+metrics["costUsd"].as_f64().unwrap())} else {None};
            let (passed,output)=process::check(root,task,&cell.answer).await?;
            cell.output=output;
            let _=updates.send(cell.clone());
            if passed { cell.status="passed".into(); return Ok(()); }
            prompt=format!("The check `{}` still fails: {}. Fix it.",task.check.command,cell.output);
        }
        cell.status="failed".into();
        Ok(())
    }
    async fn prune(&self) {
        let active=self.active.lock().await;
        let Ok(runs)=self.runs() else {return};
        let mut recent=Vec::new();
        for run in &runs {
            if active.contains_key(&run.id) || run.plan.keep_workdirs==Some(true) { continue; }
            for cell in &run.cells {
                let path=self.data.join("evals/work").join(&run.id).join(&cell.id);
                if path.is_dir() { recent.push((cell.completed_at.as_ref().unwrap_or(&run.created_at).clone(),run.id.clone(),cell.id.clone())); }
            }
        }
        recent.sort_by(|a,b|b.cmp(a));
        for mut run in runs {
            let mut changed=false;
            for (_,run_id,cell_id) in recent.iter().skip(20).filter(|(_,id,_)|id==&run.id) {
                let path=self.data.join("evals/work").join(run_id).join(cell_id);
                if std::fs::remove_dir_all(path).is_ok() {
                    if let Some(cell)=run.cells.iter_mut().find(|c|&c.id==cell_id) {cell.workdir=None;changed=true;}
                }
            }
            if changed { let _=self.save(&run); }
        }
    }
}

