import { formatModelRef } from "@settings/registry";
import {EvalsResults} from "./EvalsResults";
import {EvalsRunForm} from "./EvalsRunForm";
import {useCallback,useEffect,useMemo,useState} from "react";
import type {AppSettings} from "@/types";
import {evalRequest,engineRuntime,type EngineRuntimeStatus} from "@services/tauri";
import {subscribeEvalProgress} from "@services/events";
import {SettingsPage,SettingsGroup,SettingsFieldRow,SettingsSelect,SettingsButton} from "@/features/design-system/components/settings/SettingsPrimitives";
import {configKey,planCount,roleSettings,percent,type EvalCell,type EvalPlan,type EvalTask,type EvalHistory,type EvalResult,type EvalSummary} from "./evals";
import "./evals.css";

const number=(n:number|null)=>n===null?"unknown":n.toLocaleString(undefined,{maximumFractionDigits:2});
export function EvalsView({appSettings,onUpdateAppSettings,onOpenFull,onClose,full=false,initialRunId}:{appSettings:AppSettings;onUpdateAppSettings:(s:AppSettings)=>Promise<unknown>;onOpenFull?:(runId?:string)=>void;onClose?:()=>void;full?:boolean;initialRunId?:string}) {
 const models=(appSettings.providers??[]).filter(p=>p.enabled).flatMap(p=>p.models.filter(m=>m.enabled).map(m=>({id:p.id+"/"+m.id,name:formatModelRef(appSettings,{providerId:p.id,modelId:m.id}).title})));
 const engines=[...["pi","omp","codex","claude"].map(id=>({id,name:id,enabled:appSettings.engines?.find(e=>e.id===id)?.enabled??id!=="omp"})),...(appSettings.engines??[]).filter(e=>!["pi","omp","codex","claude"].includes(e.id))].filter(e=>e.enabled);
 const [plan,setPlan]=useState<EvalPlan>({engines:engines.slice(0,1).map(e=>e.id),models:models.slice(0,1).map(m=>m.id),tags:["smoke"],categories:[],codegraph:[false],repeats:1,concurrency:2});
 const [runtimes,setRuntimes]=useState<EngineRuntimeStatus[]>([]);
 const [tasks,setTasks]=useState<EvalTask[]>([]);
 const [history,setHistory]=useState<EvalHistory[]>([]);
 const [result,setResult]=useState<EvalResult|null>(null);
 const [baseline,setBaseline]=useState<EvalResult|null>(null);
 const [error,setError]=useState("");
 const [confirm,setConfirm]=useState(false);
 const [busy,setBusy]=useState(false);
 const [estimate,setEstimate]=useState<number|null>(null);
 const [sort,setSort]=useState<keyof EvalSummary>("passRate");
 const [ascending,setAscending]=useState(false);
 const [expanded,setExpanded]=useState<string|null>(null);
 const [transcript,setTranscript]=useState<EvalCell|null>(null);
 const [role,setRole]=useState("default");
 const [pendingModel,setPendingModel]=useState<string|null>(null);
 const refresh=useCallback(async()=>setHistory(await evalRequest<EvalHistory[]>("runs")),[]);
 const load=useCallback(async(id:string)=>{setResult(await evalRequest<EvalResult>("run",{runId:id}));},[]);
 const run=async(fn:()=>Promise<unknown>)=>{setBusy(true);setError("");try{await fn();}catch(e){setError(String(e));}finally{setBusy(false);}};
 useEffect(()=>{void Promise.all([evalRequest<EvalTask[]>("suites").then(setTasks),refresh(),engineRuntime("status").then(setRuntimes)]).catch(e=>setError(String(e)));},[refresh]);
 useEffect(()=>{if(initialRunId)void load(initialRunId).catch(e=>setError(String(e)));},[initialRunId,load]);
 useEffect(()=>{let stale=false; setEstimate(null); void evalRequest<{estimatedTokens:number|null}>("plan",plan).then(p=>{if(!stale)setEstimate(p.estimatedTokens);}).catch(()=>{}); return()=>{stale=true;};},[plan]);
 const active=result&&["running","preparing"].includes(result.run.status)?result.run.id:null;
 useEffect(()=>subscribeEvalProgress(event=>{
   if(event.runId===result?.run.id) void load(event.runId).catch(e=>setError(String(e)));
   if(event.status!=="running") void refresh().catch(e=>setError(String(e)));
 }),[result?.run.id,load,refresh]);
 useEffect(()=>{if(!active)return;const timer=setInterval(()=>void load(active).catch(e=>setError(String(e))),2000);return()=>clearInterval(timer);},[active,load]);
 const count=planCount(tasks,plan);
 const rows=useMemo(()=>[...(result?.summary??[])].sort((a,b)=>{
   const av=a[sort],bv=b[sort];
   if(av===null)return 1;if(bv===null)return -1;
   return (typeof av==="number"&&typeof bv==="number"?av-bv:String(av).localeCompare(String(bv)))*(ascending?1:-1);
 }),[result,sort,ascending]);
 return <SettingsPage title="Evals" subtitle="Compare engines and models on reproducible coding tasks." className="evals-page">
  <div className="evals-actions">{onOpenFull&&<SettingsButton onClick={()=>onOpenFull(result?.run.id)}>Open full view</SettingsButton>}{onClose&&<SettingsButton onClick={onClose}>Close full view</SettingsButton>}</div>
  <SettingsGroup className="evals-new-run" title="New run" collapsible={full&&!!result} subtitle="Runs use real provider tokens. Fixtures are isolated from your projects.">
   <EvalsRunForm plan={plan} onChange={next=>{setConfirm(false);setPlan(next);}} tasks={tasks} engines={engines} models={models} runtimes={runtimes}/>
   <div className="evals-start"><p role="status">{count.cells} cells · {count.tasks} tasks · est. tokens {estimate===null?"unknown":number(estimate)}</p><SettingsButton className="primary" disabled={busy||!!active||!count.cells} onClick={()=>setConfirm(true)}>Start run</SettingsButton></div>
   {confirm&&<div role="alert" className="evals-confirm"><p>Start {count.cells} cells using paid provider tokens?</p><SettingsButton disabled={busy} onClick={()=>void run(async()=>{const started=await evalRequest<{runId:string}>("start",plan);setConfirm(false);await load(started.runId);await refresh();})}>Confirm start</SettingsButton><SettingsButton onClick={()=>setConfirm(false)}>Cancel</SettingsButton></div>}
  </SettingsGroup>
  {result&&<SettingsGroup title={`Run · ${result.run.status}`} subtitle={`App ${result.run.environment.appVersion??"unknown"} · ${result.run.environment.engines?.filter(e=>result.run.plan.engines.includes(e.engine==="claude-acp"?"claude":e.engine)).map(e=>e.engine+" "+(e.version??"unknown")).join(" · ")||"Engine versions unknown"}`} actions={<div className="evals-actions">{active&&<SettingsButton onClick={()=>void run(()=>evalRequest("cancel",{runId:active}))}>Cancel run</SettingsButton>}{!full&&onOpenFull&&<SettingsButton className="primary" onClick={()=>onOpenFull(result.run.id)}>Open results</SettingsButton>}</div>}>
   {result.run.message&&<p role="status">{result.run.message}</p>}
   {active&&<div className="evals-grid" aria-label="Cell progress">{result.run.cells.map(cell=><SettingsButton key={cell.id} className={"evals-cell "+cell.status} title={`${cell.task} · ${configKey(cell)} · repeat ${cell.repeat+1} · ${cell.status}`} onClick={()=>{setExpanded(configKey(cell));setTranscript(cell);}}><span aria-hidden>●</span><span>{cell.task}</span><small>{cell.engine} · {cell.status}</small></SettingsButton>)}</div>}
   <EvalsResults rows={rows} cells={result.run.cells} full={full} expanded={expanded} sort={sort} ascending={ascending} onSort={(key,high)=>{setAscending(sort===key?!ascending:!high);setSort(key);}} onConfig={key=>setExpanded(expanded===key?null:key)} onTranscript={setTranscript}/>
   {expanded&&<section aria-label="Configuration details">
    <div className="evals-actions"><SettingsSelect aria-label="Agent role" value={role} onChange={e=>setRole(e.target.value)}><option value="default">Orchestrator / default model</option>{appSettings.agentPolicy?.subagents.roles.map(r=><option key={r.name} value={r.name}>{r.name}</option>)}</SettingsSelect><SettingsButton onClick={()=>setPendingModel(rows.find(r=>r.config===expanded)?.model??null)}>Set as default for role</SettingsButton></div>
    {pendingModel&&<div role="alert"><p>Use {formatModelRef(appSettings,pendingModel).title} for {role}?</p><SettingsButton onClick={()=>void run(async()=>{await onUpdateAppSettings(roleSettings(appSettings,pendingModel,role));setPendingModel(null);})}>Confirm default</SettingsButton><SettingsButton onClick={()=>setPendingModel(null)}>Cancel</SettingsButton></div>}
    {result.run.cells.filter(c=>configKey(c)===expanded).map(c=><details key={c.id}><summary>{c.task} · {c.status} · {number(c.usage.engineReadyMs)} ms ready</summary><pre>{c.output||"No check output"}</pre>{c.usage.errors.map((error,i)=><p role="alert" key={i}>{error}</p>)}{c.workdir&&<p>Workdir: <code>{c.workdir}</code></p>}<SettingsButton onClick={()=>setTranscript(c)}>Open transcript (read-only)</SettingsButton></details>)}
   </section>}
   {transcript&&<section aria-label="Read-only transcript"><h3>{transcript.task} · {transcript.threadId??"No thread"}</h3><SettingsButton onClick={()=>setTranscript(null)}>Close transcript</SettingsButton><pre tabIndex={0}>{JSON.stringify(transcript.transcript,null,2)}</pre></section>}
   {full&&<div className="evals-compare"><SettingsFieldRow className="settings-field-row-long" title="Compare against run A"><SettingsSelect value={baseline?.run.id??""} onChange={e=>void run(async()=>setBaseline(e.target.value?await evalRequest<EvalResult>("run",{runId:e.target.value}):null))}><option value="">No comparison</option>{history.filter(h=>h.id!==result.run.id).map(h=><option key={h.id} value={h.id}>{h.createdAt} · {h.id.slice(0,8)}</option>)}</SettingsSelect></SettingsFieldRow></div>}
   {full&&baseline&&<div className="evals-table-scroll"><table aria-label="A/B deltas"><thead><tr><th>Configuration (B − A)</th><th>Pass delta</th><th>Token delta</th></tr></thead><tbody>{rows.map(row=>{const old=baseline.summary.find(b=>b.config===row.config);return <tr key={row.config}><th>{row.engine} · {formatModelRef(appSettings,row.model).title} · cg:{row.codegraph?"on":"off"}</th><td className={old&&row.passRate<old.passRate?"evals-regression":undefined}>{old?percent(row.passRate-old.passRate):"New config"}</td><td className={old&&row.tokens>old.tokens?"evals-regression":undefined}>{old?number(row.tokens-old.tokens):"—"}</td></tr>;})}</tbody></table><table aria-label="Per-task deltas"><thead><tr><th>Task / configuration</th><th>A → B</th><th>Token delta</th></tr></thead><tbody>{result.run.cells.map(c=>{const a=baseline.run.cells.find(a=>a.task===c.task&&configKey(a)===configKey(c)&&a.repeat===c.repeat);const tokens=(v:EvalCell)=>v.usage.tokensIn+v.usage.cachedTokens+v.usage.tokensOut;return <tr key={c.id} className={a?.status==="passed"&&c.status!=="passed"?"evals-regression":undefined}><th>{c.task} / {c.engine} · {formatModelRef(appSettings,c.model).title} · cg:{c.codegraph?"on":"off"}</th><td>{a?.status??"new"} → {c.status}</td><td>{a?number(tokens(c)-tokens(a)):"—"}</td></tr>;})}</tbody></table></div>}
  </SettingsGroup>}
  <SettingsGroup title="History"><ul className="evals-history">{history.map(h=><li key={h.id}><SettingsButton data-eval-run={h.id} onClick={()=>void run(()=>load(h.id))}>{new Date(h.createdAt).toLocaleString()} · {h.plan.engines.join(", ")} · {h.plan.models.map(model=>formatModelRef(appSettings,model).title).join(", ")} · {h.status} · {h.summary.length?percent(h.summary.reduce((n,s)=>n+s.passRate,0)/h.summary.length):"pending"}</SettingsButton><SettingsButton disabled={["running","preparing"].includes(h.status)} onClick={()=>void run(async()=>{await evalRequest("delete",{runId:h.id});if(result?.run.id===h.id)setResult(null);await refresh();})}>Delete</SettingsButton></li>)}</ul></SettingsGroup>
  {error&&<p role="alert">{error}</p>}
 </SettingsPage>;
}

