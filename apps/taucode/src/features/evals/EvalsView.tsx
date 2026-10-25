import {useCallback,useEffect,useMemo,useState} from "react";
import type {AppSettings} from "@/types";
import {evalRequest} from "@services/tauri";
import {subscribeEvalProgress} from "@services/events";
import {SettingsPage,SettingsGroup,SettingsFieldRow,SettingsSelect,SettingsTextField,SettingsButton,SettingsSegmented} from "@/features/design-system/components/settings/SettingsPrimitives";
import {configKey,planCount,roleSettings,type EvalCell,type EvalPlan,type EvalTask,type EvalHistory,type EvalResult,type EvalSummary} from "./evals";
import "./evals.css";

const percent=(n:number)=>`${(n*100).toFixed(1)}%`;
const number=(n:number|null)=>n===null?"unknown":n.toLocaleString(undefined,{maximumFractionDigits:2});
export function EvalsView({appSettings,onUpdateAppSettings,onOpenFull,onClose}:{appSettings:AppSettings;onUpdateAppSettings:(s:AppSettings)=>Promise<unknown>;onOpenFull?:()=>void;onClose?:()=>void}) {
 const models=(appSettings.providers??[]).filter(p=>p.enabled).flatMap(p=>p.models.filter(m=>m.enabled).map(m=>({id:p.id+"/"+m.id,name:p.name+" / "+(m.displayName??m.id)})));
 const engines=[...["pi","omp","codex","claude"].map(id=>({id,name:id,enabled:appSettings.engines?.find(e=>e.id===id)?.enabled??id!=="omp"})),...(appSettings.engines??[]).filter(e=>!["pi","omp","codex","claude"].includes(e.id))].filter(e=>e.enabled);
 const [plan,setPlan]=useState<EvalPlan>({engines:engines.slice(0,1).map(e=>e.id),models:models.slice(0,1).map(m=>m.id),tags:["smoke"],categories:[],codegraph:[false],repeats:1,concurrency:2});
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
 useEffect(()=>{void Promise.all([evalRequest<EvalTask[]>("suites").then(setTasks),refresh()]).catch(e=>setError(String(e)));},[refresh]);
 useEffect(()=>{let stale=false; setEstimate(null); void evalRequest<{estimatedTokens:number|null}>("plan",plan).then(p=>{if(!stale)setEstimate(p.estimatedTokens);}).catch(()=>{}); return()=>{stale=true;};},[plan]);
 const active=result?.run.status==="running"?result.run.id:null;
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
 const columns: {key:keyof EvalSummary;label:string;high?:boolean}[]=[{key:"passRate",label:"Pass",high:true},{key:"tokens",label:"Tokens"},{key:"cacheRate",label:"Cache",high:true},{key:"costUsd",label:"Cost ($)"},{key:"firstTokenMs",label:"First token (ms)"},{key:"wallMs",label:"Wall (ms)"},{key:"toolCalls",label:"Tools"},{key:"errors",label:"Errors"}];
 const best=(key:keyof EvalSummary,high?:boolean)=>{const values=rows.map(r=>r[key]).filter((n):n is number=>typeof n==="number");return high?Math.max(...values):Math.min(...values);};
 const choose=(key:"engines"|"models"|"tags"|"categories",values:string[])=>{setConfirm(false);setPlan(p=>({...p,[key]:values}));};
 const selected=(event:React.ChangeEvent<HTMLSelectElement>)=>Array.from(event.target.selectedOptions,o=>o.value);
 return <SettingsPage title="Evals" subtitle="Compare engines and models on reproducible coding tasks." className="evals-page">
  <div className="evals-actions">{onOpenFull&&<SettingsButton onClick={onOpenFull}>Open full view</SettingsButton>}{onClose&&<SettingsButton onClick={onClose}>Close full view</SettingsButton>}</div>
  <SettingsGroup title="New run" subtitle="Runs use real provider tokens. Fixtures are isolated from your projects.">
   <div className="evals-fields">
    <SettingsFieldRow title="Engines"><SettingsSelect multiple value={plan.engines} onChange={e=>choose("engines",selected(e))}>{engines.map(e=><option key={e.id} value={e.id}>{e.name}</option>)}</SettingsSelect></SettingsFieldRow>
    <SettingsFieldRow title="Models"><SettingsSelect multiple value={plan.models} onChange={e=>choose("models",selected(e))}>{models.map(m=><option key={m.id} value={m.id}>{m.name}</option>)}</SettingsSelect></SettingsFieldRow>
    <SettingsFieldRow title="Tags" subtitle="No selection includes all"><SettingsSelect multiple value={plan.tags} onChange={e=>choose("tags",selected(e))}>{[...new Set(tasks.flatMap(t=>t.tags))].map(t=><option key={t}>{t}</option>)}</SettingsSelect></SettingsFieldRow>
    <SettingsFieldRow title="Categories"><SettingsSelect multiple value={plan.categories} onChange={e=>choose("categories",selected(e))}>{[...new Set(tasks.map(t=>t.category))].map(t=><option key={t}>{t}</option>)}</SettingsSelect></SettingsFieldRow>
    <SettingsFieldRow title="Codegraph"><SettingsSegmented label="Codegraph" value={plan.codegraph.length===2?"both":plan.codegraph[0]?"on":"off"} options={["off","on","both"].map(value=>({value,label:value}))} onChange={v=>{setConfirm(false);setPlan(p=>({...p,codegraph:v==="both"?[false,true]:[v==="on"]}));}} /></SettingsFieldRow>
    {(["repeats","concurrency","maxTotalTokens","maxCost"] as const).map(key=><SettingsFieldRow key={key} title={{repeats:"Repeats",concurrency:"Concurrency",maxTotalTokens:"Token cap",maxCost:"Cost cap ($)"}[key]}><SettingsTextField type="number" min={key==="maxCost"?0.01:1} max={key==="concurrency"?6:key==="repeats"?100:undefined} step={key==="maxCost"?0.01:1} value={plan[key]??""} placeholder="Unlimited" onChange={e=>{setConfirm(false);setPlan(p=>({...p,[key]:e.target.value?Number(e.target.value):undefined}));}} /></SettingsFieldRow>)}
   </div>
   <div className="evals-actions"><span role="status">{count.cells} cells · {count.tasks} tasks · est. tokens {estimate===null?"unknown":number(estimate)}</span><SettingsButton disabled={busy||!!active||!count.cells} onClick={()=>setConfirm(true)}>Start</SettingsButton></div>
   {confirm&&<div role="alert" className="evals-confirm"><p>Start {count.cells} cells using paid provider tokens?</p><SettingsButton disabled={busy} onClick={()=>void run(async()=>{const started=await evalRequest<{runId:string}>("start",plan);setConfirm(false);await load(started.runId);await refresh();})}>Confirm start</SettingsButton><SettingsButton onClick={()=>setConfirm(false)}>Cancel</SettingsButton></div>}
  </SettingsGroup>
  {result&&<SettingsGroup title={`Run · ${result.run.status}`} actions={active&&<SettingsButton onClick={()=>void run(()=>evalRequest("cancel",{runId:active}))}>Cancel run</SettingsButton>}>
   <div className="evals-grid" aria-label="Cell progress">{result.run.cells.map(cell=><button type="button" key={cell.id} className={"evals-cell "+cell.status} title={`${cell.task} · ${configKey(cell)} · repeat ${cell.repeat+1} · ${cell.status}`} onClick={()=>{setExpanded(configKey(cell));setTranscript(cell);}}><span aria-hidden>●</span><span>{cell.task}</span><small>{cell.engine} · {cell.status}</small></button>)}</div>
   <div className="evals-table-scroll"><table><thead><tr><th>Configuration</th>{columns.map(c=><th key={c.key} aria-sort={sort===c.key?(ascending?"ascending":"descending"):"none"}><SettingsButton onClick={()=>{setAscending(sort===c.key?!ascending:!c.high);setSort(c.key);}}>{c.label}</SettingsButton></th>)}</tr></thead><tbody>{rows.map(row=><tr key={row.config}><th><SettingsButton aria-expanded={expanded===row.config} onClick={()=>setExpanded(expanded===row.config?null:row.config)}>{row.engine} / {row.model}<small>Codegraph {row.codegraph?"on":"off"}</small></SettingsButton></th>{columns.map(c=><td key={c.key} className={row[c.key]!==null&&row[c.key]===best(c.key,c.high)?"evals-best":undefined}>{c.key==="passRate"||c.key==="cacheRate"?percent(row[c.key]):number(row[c.key] as number|null)}</td>)}</tr>)}</tbody></table></div>
   {expanded&&<section aria-label="Configuration details">
    <div className="evals-actions"><SettingsSelect aria-label="Agent role" value={role} onChange={e=>setRole(e.target.value)}><option value="default">Orchestrator / default model</option>{appSettings.agentPolicy?.subagents.roles.map(r=><option key={r.name} value={r.name}>{r.name}</option>)}</SettingsSelect><SettingsButton onClick={()=>setPendingModel(rows.find(r=>r.config===expanded)?.model??null)}>Set as default for role</SettingsButton></div>
    {pendingModel&&<div role="alert"><p>Use {pendingModel} for {role}?</p><SettingsButton onClick={()=>void run(async()=>{await onUpdateAppSettings(roleSettings(appSettings,pendingModel,role));setPendingModel(null);})}>Confirm default</SettingsButton><SettingsButton onClick={()=>setPendingModel(null)}>Cancel</SettingsButton></div>}
    {result.run.cells.filter(c=>configKey(c)===expanded).map(c=><details key={c.id}><summary>{c.task} · {c.status} · {number(c.usage.engineReadyMs)} ms ready</summary><pre>{c.output||"No check output"}</pre>{c.usage.errors.map((error,i)=><p role="alert" key={i}>{error}</p>)}{c.workdir&&<p>Workdir: <code>{c.workdir}</code></p>}<SettingsButton onClick={()=>setTranscript(c)}>Open transcript (read-only)</SettingsButton></details>)}
   </section>}
   {transcript&&<section aria-label="Read-only transcript"><h3>{transcript.task} · {transcript.threadId??"No thread"}</h3><SettingsButton onClick={()=>setTranscript(null)}>Close transcript</SettingsButton><pre tabIndex={0}>{JSON.stringify(transcript.transcript,null,2)}</pre></section>}
   <SettingsFieldRow title="Compare against run A"><SettingsSelect value={baseline?.run.id??""} onChange={e=>void run(async()=>setBaseline(e.target.value?await evalRequest<EvalResult>("run",{runId:e.target.value}):null))}><option value="">No comparison</option>{history.filter(h=>h.id!==result.run.id).map(h=><option key={h.id} value={h.id}>{h.createdAt} · {h.id.slice(0,8)}</option>)}</SettingsSelect></SettingsFieldRow>
   {baseline&&<div className="evals-table-scroll"><table aria-label="A/B deltas"><thead><tr><th>Configuration (B − A)</th><th>Pass delta</th><th>Token delta</th></tr></thead><tbody>{rows.map(row=>{const old=baseline.summary.find(b=>b.config===row.config);return <tr key={row.config}><th>{row.config}</th><td className={old&&row.passRate<old.passRate?"evals-regression":undefined}>{old?percent(row.passRate-old.passRate):"New config"}</td><td className={old&&row.tokens>old.tokens?"evals-regression":undefined}>{old?number(row.tokens-old.tokens):"—"}</td></tr>;})}</tbody></table><table aria-label="Per-task deltas"><thead><tr><th>Task / configuration</th><th>A → B</th><th>Token delta</th></tr></thead><tbody>{result.run.cells.map(c=>{const a=baseline.run.cells.find(a=>a.task===c.task&&configKey(a)===configKey(c)&&a.repeat===c.repeat);const tokens=(v:EvalCell)=>v.usage.tokensIn+v.usage.cachedTokens+v.usage.tokensOut;return <tr key={c.id} className={a?.status==="passed"&&c.status!=="passed"?"evals-regression":undefined}><th>{c.task} / {configKey(c)}</th><td>{a?.status??"new"} → {c.status}</td><td>{a?number(tokens(c)-tokens(a)):"—"}</td></tr>;})}</tbody></table></div>}
  </SettingsGroup>}
  <SettingsGroup title="History"><ul className="evals-history">{history.map(h=><li key={h.id}><SettingsButton onClick={()=>void run(()=>load(h.id))}>{new Date(h.createdAt).toLocaleString()} · {h.plan.engines.join(", ")} · {h.plan.models.join(", ")} · {h.status} · {h.summary.length?percent(h.summary.reduce((n,s)=>n+s.passRate,0)/h.summary.length):"pending"}</SettingsButton><SettingsButton disabled={h.status==="running"} onClick={()=>void run(async()=>{await evalRequest("delete",{runId:h.id});if(result?.run.id===h.id)setResult(null);await refresh();})}>Delete</SettingsButton></li>)}</ul></SettingsGroup>
  {error&&<p role="alert">{error}</p>}
 </SettingsPage>;
}

