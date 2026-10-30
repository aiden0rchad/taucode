import {ArrowLeft,FlaskConical,Maximize2} from "lucide-react";
import { formatModelRef } from "@settings/registry";
import {EvalsResults} from "./EvalsResults";
import {EvalsRunForm} from "./EvalsRunForm";
import {reasoningLabel,reasoningLevels} from "@/features/models/reasoning";
import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import type {AppSettings} from "@/types";
import {evalRequest,engineRuntime,type EngineRuntimeStatus} from "@services/tauri";
import {subscribeEvalProgress} from "@services/events";
import {SettingsGroup,SettingsFieldRow,SettingsSelect,SettingsButton} from "@/features/design-system/components/settings/SettingsPrimitives";
import {configKey,statusLabel,taskLabel,planCount,roleSettings,percent,type EvalCell,type EvalPlan,type EvalTask,type EvalHistory,type EvalResult,type EvalSummary} from "./evals";
import "./evals.css";

const number=(n:number|null)=>n===null?"unknown":n.toLocaleString(undefined,{maximumFractionDigits:2});
export function EvalsView({appSettings,onUpdateAppSettings,onOpenFull,onClose,full=false,initialRunId}:{appSettings:AppSettings;onUpdateAppSettings:(s:AppSettings)=>Promise<unknown>;onOpenFull?:(runId?:string)=>void;onClose?:()=>void;full?:boolean;initialRunId?:string}) {
 const models=(appSettings.providers??[]).filter(p=>p.enabled).flatMap(p=>p.models.filter(m=>m.enabled).map(m=>({id:p.id+"/"+m.id,name:formatModelRef(appSettings,{providerId:p.id,modelId:m.id}).title})));
 const engines=[...["pi","omp","codex","claude"].map(id=>({id,name:id,enabled:appSettings.engines?.find(e=>e.id===id)?.enabled??id!=="omp"})),...(appSettings.engines??[]).filter(e=>!["pi","omp","codex","claude"].includes(e.id))].filter(e=>e.enabled);
 const [plan,setPlan]=useState<EvalPlan>(()=>{
  const policy=appSettings.agentPolicy;
  const model=policy?.defaultModel;
  const engine=engines.find(e=>e.id===policy?.defaultEngine)??engines[0];
  const selected=models.find(m=>m.id===`${model?.providerId}/${model?.modelId}`)??models[0];
  const effort=appSettings.lastComposerReasoningEffort;
  const fallback:EvalPlan={engines:engine?[engine.id]:[],models:selected?[selected.id]:[],effort:effort&&reasoningLevels.includes(effort)?effort:undefined,tags:["smoke"],categories:[],codegraph:[false],repeats:1,concurrency:2};
  try {
   const saved:unknown=JSON.parse(localStorage.getItem("taucode.evals.plan.v1")??"null");
   if(!saved||typeof saved!=="object")return fallback;
   const p=saved as EvalPlan;
   if(![p.engines,p.models,p.tags,p.categories].every(a=>Array.isArray(a)&&a.every(v=>typeof v==="string"))
     ||!Array.isArray(p.codegraph)||!p.codegraph.length||!p.codegraph.every(v=>typeof v==="boolean")
     ||!Number.isInteger(p.repeats)||p.repeats<1||p.repeats>100
     ||!Number.isInteger(p.concurrency)||p.concurrency<1||p.concurrency>6
     ||[p.maxTotalTokens,p.maxCost].some(v=>v!==undefined&&(typeof v!=="number"||!Number.isFinite(v)||v<=0)))return fallback;
   return {engines:[...new Set(p.engines)].filter(id=>engines.some(e=>e.id===id)),models:[...new Set(p.models)].filter(id=>models.some(m=>m.id===id)),
    effort:p.effort&&reasoningLevels.includes(p.effort)?p.effort:undefined,tags:p.tags,categories:p.categories,codegraph:[...new Set(p.codegraph)],
    repeats:p.repeats,concurrency:p.concurrency,maxTotalTokens:p.maxTotalTokens,maxCost:p.maxCost};
  }catch{return fallback;}
 });
 useEffect(()=>{try{localStorage.setItem("taucode.evals.plan.v1",JSON.stringify(plan));}catch{/* Storage can be unavailable; the current draft still works. */}},[plan]);
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
 const resultsHeading=useRef<HTMLHeadingElement>(null);
 const [revealRun,setRevealRun]=useState<string|null>(null);
 const selectRun=async(id:string)=>{setExpanded(null);setTranscript(null);setPendingModel(null);setBaseline(null);await load(id);setRevealRun(id);};
 const refresh=useCallback(async()=>setHistory(await evalRequest<EvalHistory[]>("runs")),[]);
 const load=useCallback(async(id:string)=>{setResult(await evalRequest<EvalResult>("run",{runId:id}));},[]);
 useEffect(()=>{if(revealRun&&result?.run.id===revealRun){resultsHeading.current?.scrollIntoView?.({block:"start"});resultsHeading.current?.focus({preventScroll:true});setRevealRun(null);}},[revealRun,result?.run.id]);
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
 const cells=result?.run.cells??[];
 const finished=cells.filter(c=>!["running","queued"].includes(c.status)).length;
 const passed=cells.filter(c=>c.status==="passed").length;
 const failed=cells.filter(c=>["failed","error"].includes(c.status)).length;
 const queued=cells.filter(c=>c.status==="queued").length;
 const rows=useMemo(()=>[...(result?.summary??[])].sort((a,b)=>{
   const av=a[sort],bv=b[sort];
   if(av===null)return 1;if(bv===null)return -1;
   return (typeof av==="number"&&typeof bv==="number"?av-bv:String(av).localeCompare(String(bv)))*(ascending?1:-1);
 }),[result,sort,ascending]);
 return <section className="settings-page evals-page" aria-label="Model evaluations">
  <header className="evals-header"><div className="evals-heading"><span className="evals-heading-icon"><FlaskConical size={22} aria-hidden/></span><div><h2>Model evaluations</h2><p>Compare correctness, speed and token use on the same coding tasks.</p></div></div><div className="evals-actions">{onOpenFull&&<SettingsButton onClick={()=>onOpenFull(result?.run.id)}><Maximize2 size={14} aria-hidden/> Open full view</SettingsButton>}{onClose&&<SettingsButton onClick={onClose}><ArrowLeft size={14} aria-hidden/> Close full view</SettingsButton>}</div></header>
  <SettingsGroup className="evals-new-run" title="New run" collapsible={!!result}>
   <EvalsRunForm plan={plan} onChange={next=>{setConfirm(false);setPlan(next);}} tasks={tasks} engines={engines} models={models} runtimes={runtimes}/>
   <div className="evals-start"><div><strong role="status">{count.cells} tests · {count.tasks} tasks</strong><p>{plan.models.length} model{plan.models.length===1?"":"s"} × {plan.engines.length} engine{plan.engines.length===1?"":"s"} · {plan.repeats} attempt{plan.repeats===1?"":"s"} per task{plan.codegraph.length>1?" · CodeGraph on and off":""}</p><p>Uses real provider tokens in isolated test folders. {estimate===null?"Token estimate unavailable.":`Estimated tokens: ${number(estimate)}.`}</p></div><SettingsButton className="primary" disabled={busy||!!active||!count.cells} onClick={()=>setConfirm(true)}>Start run</SettingsButton></div>
   {!count.cells&&<p className="evals-help">Choose at least one model, engine and matching task to start.</p>}
   {confirm&&<div role="alert" className="evals-confirm"><p>Start {count.cells} tests using paid provider tokens?</p><div className="evals-actions"><SettingsButton className="primary" disabled={busy} onClick={()=>void run(async()=>{const started=await evalRequest<{runId:string}>("start",plan);setConfirm(false);await selectRun(started.runId);await refresh();})}>Confirm start</SettingsButton><SettingsButton onClick={()=>setConfirm(false)}>Cancel</SettingsButton></div></div>}
  </SettingsGroup>
  {result&&<section className="evals-run" aria-label="Run results">
   <div className="evals-run-heading"><div><h3 ref={resultsHeading} tabIndex={-1}>Results <span className={"evals-badge "+result.run.status}>{statusLabel(result.run.status)}</span></h3><p>{new Date(result.run.createdAt).toLocaleString()} · {cells.length} tests</p></div><div className="evals-actions">{active&&<SettingsButton disabled={busy} onClick={()=>void run(()=>evalRequest("cancel",{runId:active}))}>Cancel run</SettingsButton>}{!full&&onOpenFull&&<SettingsButton onClick={()=>onOpenFull(result.run.id)}>Open results</SettingsButton>}</div></div>
   <div className="evals-run-context"><span>Requested reasoning: {result.run.plan.effort?reasoningLabel(result.run.plan.effort):"Model default"}{result.run.environment.agentPolicy&&` · Team: ${result.run.environment.agentPolicy.subagents.enabled?result.run.environment.agentPolicy.subagents.mode==="chain-of-command"?"Chain of Command":"Subagents":"Off"}`}</span><span>{`App ${result.run.environment.appVersion??"unknown"} · ${result.run.environment.engines?.filter(e=>result.run.plan.engines.includes(e.engine==="claude-acp"?"claude":e.engine)).map(e=>e.engine+" "+(e.version??"unknown")).join(" · ")||"Engine versions unknown"}`}</span></div>
   {result.run.message&&<p role="status" className="evals-notice">{result.run.message}</p>}
   <div className="evals-progress" aria-label="Run progress"><div><strong>{active?`${finished} of ${cells.length} finished`:`${passed} passed · ${failed} failed`}</strong><span>{active?`${cells.filter(c=>c.status==="running").length} running · ${queued} queued`:`${cells.filter(c=>c.status==="cancelled").length} cancelled · ${cells.filter(c=>c.status==="skipped").length} skipped`}</span></div>{active&&<progress aria-label="Finished tests" value={finished} max={Math.max(1,cells.length)}/>}</div>
   {active&&<div className="evals-grid" aria-label="Test progress">{cells.filter(cell=>cell.status==="running").map(cell=><SettingsButton key={cell.id} className={"evals-cell "+cell.status} title={`${cell.task} · ${configKey(cell)} · repeat ${cell.repeat+1}`} onClick={()=>{setExpanded(configKey(cell));setTranscript(cell);}}><span aria-hidden>●</span><strong>{taskLabel(cell.task)}</strong><small>{formatModelRef(appSettings,cell.model).name}</small><small>{cell.engine} · Running · Attempt {cell.repeat+1}</small></SettingsButton>)}</div>}
   {!rows.length&&<div className="evals-empty"><strong>{active?"Waiting for the first result":"No completed test results"}</strong><p>{active?"Results appear as each test finishes. Running tests and queued work are shown below.":"This run ended before any test produced a result. No scores or costs can be compared."}</p></div>}
   <EvalsResults rows={rows} cells={result.run.cells} full={full} expanded={expanded} sort={sort} ascending={ascending} onSort={(key,high)=>{setAscending(sort===key?!ascending:!high);setSort(key);}} onConfig={key=>setExpanded(expanded===key?null:key)} onTranscript={setTranscript}/>
   {expanded&&<section aria-label="Configuration details">
    <div className="evals-actions"><SettingsSelect aria-label="Agent role" value={role} onChange={e=>setRole(e.target.value)}><option value="default">Orchestrator / default model</option>{appSettings.agentPolicy?.subagents.roles.map(r=><option key={r.name} value={r.name}>{r.name}</option>)}</SettingsSelect><SettingsButton onClick={()=>setPendingModel(rows.find(r=>r.config===expanded)?.model??null)}>Set as default for role</SettingsButton></div>
    {pendingModel&&<div role="alert"><p>Use {formatModelRef(appSettings,pendingModel).title} for {role}?</p><SettingsButton onClick={()=>void run(async()=>{await onUpdateAppSettings(roleSettings(appSettings,pendingModel,role));setPendingModel(null);})}>Confirm default</SettingsButton><SettingsButton onClick={()=>setPendingModel(null)}>Cancel</SettingsButton></div>}
    {result.run.cells.filter(c=>configKey(c)===expanded).map(c=><details key={c.id}><summary>{c.task} · {c.status} · {number(c.usage.engineReadyMs)} ms ready</summary><pre>{c.output||"No check output"}</pre>{c.usage.errors.map((error,i)=><p role="alert" key={i}>{error}</p>)}{c.workdir&&<p>Workdir: <code>{c.workdir}</code></p>}<SettingsButton onClick={()=>setTranscript(c)}>Open transcript (read-only)</SettingsButton></details>)}
   </section>}
   {transcript&&<section aria-label="Read-only transcript" className="evals-empty"><div className="evals-run-heading"><h3>{taskLabel(transcript.task)} <span className={"evals-badge "+transcript.status}>{statusLabel(transcript.status)}</span></h3><SettingsButton onClick={()=>setTranscript(null)}>Close transcript</SettingsButton></div><p>{formatModelRef(appSettings,transcript.model).title} · {transcript.engine} · Attempt {transcript.repeat+1}</p><h4>Check output</h4><pre tabIndex={0}>{transcript.output||"No check output recorded."}</pre>{transcript.answer&&<><h4>Model response</h4><pre tabIndex={0}>{transcript.answer}</pre></>}<details><summary>Raw event log</summary><pre tabIndex={0}>{JSON.stringify(transcript.transcript,null,2)}</pre></details></section>}
   {full&&rows.length>0&&<details className="evals-details evals-comparison"><summary>Compare with a previous run</summary><div className="evals-compare"><SettingsFieldRow className="settings-field-row-long" title="Previous run"><SettingsSelect value={baseline?.run.id??""} onChange={e=>void run(async()=>setBaseline(e.target.value?await evalRequest<EvalResult>("run",{runId:e.target.value}):null))}><option value="">Choose a run to compare</option>{history.filter(h=>h.id!==result.run.id&&h.summary.length>0).map(h=><option key={h.id} value={h.id}>{new Date(h.createdAt).toLocaleString()} · {statusLabel(h.status)}</option>)}</SettingsSelect></SettingsFieldRow></div>
   {baseline&&<div className="evals-table-scroll"><table aria-label="A/B deltas"><thead><tr><th>Configuration</th><th>Pass rate change</th><th>Median token change</th></tr></thead><tbody>{rows.map(row=>{const old=baseline.summary.find(b=>b.config===row.config);return <tr key={row.config}><th>{row.engine} · {formatModelRef(appSettings,row.model).title} · cg:{row.codegraph?"on":"off"}</th><td className={old&&row.passRate<old.passRate?"evals-regression":undefined}>{old?`${((row.passRate-old.passRate)*100).toFixed(1)} pp`:"New config"}</td><td className={old&&row.tokens>old.tokens?"evals-regression":undefined}>{old?number(row.tokens-old.tokens):"—"}</td></tr>;})}</tbody></table><table aria-label="Per-task deltas"><thead><tr><th>Task / configuration</th><th>A → B</th><th>Token delta</th></tr></thead><tbody>{result.run.cells.map(c=>{const a=baseline.run.cells.find(a=>a.task===c.task&&configKey(a)===configKey(c)&&a.repeat===c.repeat);const tokens=(v:EvalCell)=>v.usage.tokensIn+v.usage.cachedTokens+v.usage.tokensOut;return <tr key={c.id} className={a?.status==="passed"&&c.status!=="passed"?"evals-regression":undefined}><th>{c.task} / {c.engine} · {formatModelRef(appSettings,c.model).title} · cg:{c.codegraph?"on":"off"}</th><td>{a?.status??"new"} → {c.status}</td><td>{a?number(tokens(c)-tokens(a)):"—"}</td></tr>;})}</tbody></table></div>}
  </details>}
  </section>}
  <SettingsGroup title="Recent runs" className="evals-history-group">{history.length?<ul className="evals-history">{history.map(h=><li key={h.id} className={h.id===result?.run.id?"is-selected":undefined}><SettingsButton className="evals-history-open" data-eval-run={h.id} aria-current={h.id===result?.run.id?"true":undefined} onClick={()=>void run(()=>selectRun(h.id))}><span className="evals-history-title"><strong>{new Date(h.createdAt).toLocaleString()}</strong><span className={"evals-badge "+h.status}>{statusLabel(h.status)}</span></span><span>{h.plan.models.map(model=>formatModelRef(appSettings,model).name).join(" · ")}</span><small>{h.plan.engines.join(", ")} · {h.summary.length?`${percent(h.summary.reduce((n,s)=>n+s.passRate,0)/h.summary.length)} average pass rate (finished tests)`:["running","preparing"].includes(h.status)?"Awaiting results":"No completed test results"}</small></SettingsButton><SettingsButton className="ghost" aria-label={`Delete run from ${new Date(h.createdAt).toLocaleString()}`} disabled={["running","preparing"].includes(h.status)} onClick={()=>void run(async()=>{await evalRequest("delete",{runId:h.id});if(result?.run.id===h.id){setResult(null);setExpanded(null);setTranscript(null);}if(baseline?.run.id===h.id)setBaseline(null);await refresh();})}>Delete</SettingsButton></li>)}</ul>:<p className="evals-help">Your saved runs will appear here. Set up your first run above.</p>}</SettingsGroup>
  {error&&<p role="alert">{error}</p>}
 </section>;
}
