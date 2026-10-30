import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import {Puzzle,Sparkles,Terminal} from "lucide-react";
import {SettingsButton} from "@/features/design-system/components/settings/SettingsPrimitives";
import {configKey,metric,tokens,statusLabel,taskLabel,type EvalCell,type EvalSummary} from "./evals";

export function EvalEngineIcon({engine}:{engine:string}) {
 const Icon=engine==="codex"?Terminal:engine==="claude"?Sparkles:Puzzle;
 return <span className="evals-engine-icon" aria-hidden>{engine==="pi"?"π":engine==="omp"?"ϖ":<Icon size={16}/>}</span>;
}
const columns: {key:keyof EvalSummary;label:string;high?:boolean}[]=[
 {key:"passRate",label:"Pass rate",high:true},{key:"tokens",label:"Median tokens"},{key:"cacheRate",label:"Cached input",high:true},
 {key:"costUsd",label:"Total cost"},{key:"toolCalls",label:"Tool calls"},{key:"firstTokenMs",label:"First response"},{key:"wallMs",label:"Median time"},{key:"errors",label:"Errors"},
];
export function EvalsResults({rows,cells,full,sort,ascending,expanded,onSort,onConfig,onTranscript}:{rows:EvalSummary[];cells:EvalCell[];full:boolean;sort:keyof EvalSummary;ascending:boolean;expanded:string|null;onSort:(key:keyof EvalSummary,high?:boolean)=>void;onConfig:(key:string)=>void;onTranscript:(cell:EvalCell)=>void}) {
 const settings = useFeatureSettings();
 const best=(row:EvalSummary,key:keyof EvalSummary,high?:boolean)=>{
  const values=rows.map(r=>r[key]).filter((v):v is number=>typeof v==="number");
  return row[key]!==null&&row[key]===(high?Math.max(...values):Math.min(...values))?"evals-best":undefined;
 };
 const config=(row:EvalSummary)=><><strong className="evals-config-model">{formatModelRef(settings, row.model).name}</strong><span className="evals-config-engine"><EvalEngineIcon engine={row.engine}/>{row.engine}<span className="evals-cg">CodeGraph {row.codegraph?"on":"off"}</span></span></>;
 const grouped=new Map<string,EvalCell[]>();
 for(const cell of cells){const key=JSON.stringify([cell.task,configKey(cell)]);const group=grouped.get(key)??[];group.push(cell);grouped.set(key,group);}
 const configs=[...new Map(cells.map(c=>[configKey(c),c])).entries()];
 return <>
  {rows.length>0&&<>
   <div className="evals-summary-cards" aria-label="Configuration summaries">{rows.map(row=>{
    const checked=cells.filter(c=>configKey(c)===row.config&&!["queued","running","cancelled","skipped"].includes(c.status));
    return <article className="evals-summary-card" aria-label={`${row.engine} / ${formatModelRef(settings, row.model).name} · cg:${row.codegraph?"on":"off"}`} key={row.config}>
     <SettingsButton className="evals-config" aria-label={row.engine+" / "+formatModelRef(settings, row.model).title+" · cg:"+(row.codegraph?"on":"off")} aria-expanded={expanded===row.config} onClick={()=>onConfig(row.config)}>{config(row)}</SettingsButton>
     <div className="evals-pass-rate"><strong>{metric("passRate",row.passRate)}</strong><span>{checked.filter(c=>c.status==="passed").length} of {checked.length} finished tests passed</span></div>
     <dl className="evals-metrics">{columns.filter(c=>["tokens","costUsd","wallMs"].includes(c.key)).map(c=><div key={c.key}><dt>{c.label}</dt><dd>{metric(c.key,row[c.key] as number|null)}</dd></div>)}</dl>
    </article>;
   })}</div>
   <p className="evals-help">Results include finished tests only. Tokens include cached input; unknown cost means the provider did not report pricing.</p>
   {full&&<details className="evals-details"><summary>Detailed metrics</summary><div className="evals-table-scroll" tabIndex={0} role="region" aria-label="Scrollable configuration comparison"><table aria-label="Configuration comparison"><thead><tr><th scope="col">Configuration</th>{columns.map(c=><th scope="col" key={c.key} aria-sort={sort===c.key?(ascending?"ascending":"descending"):"none"}><SettingsButton onClick={()=>onSort(c.key,c.high)}>{c.label}{sort===c.key&&<span aria-hidden>{ascending?" ↑":" ↓"}</span>}</SettingsButton></th>)}</tr></thead><tbody>{rows.map(row=><tr key={row.config}><th scope="row">{formatModelRef(settings,row.model).name}<small>{row.engine} · CodeGraph {row.codegraph?"on":"off"}</small></th>{columns.map(c=><td key={c.key} className={best(row,c.key,c.high)}>{metric(c.key,row[c.key] as number|null)}</td>)}</tr>)}</tbody></table></div><p className="evals-help">First response and elapsed time are medians per test. Cached input is a share of input tokens. Cost, tool calls and errors are totals.</p></details>}
  </>}
  {full&&cells.length>0&&<>
   <h3 className="evals-matrix-title">Task results <span>Choose a result to inspect its transcript</span></h3>
   <div className="evals-table-scroll" tabIndex={0} role="region" aria-label="Scrollable task results"><table className="evals-matrix" aria-label="Task by configuration"><thead><tr><th scope="col">Task</th>{configs.map(([key,c])=><th scope="col" key={key}>{formatModelRef(settings,c.model).name}<small>{c.engine} · CodeGraph {c.codegraph?"on":"off"}</small></th>)}</tr></thead><tbody>{[...new Set(cells.map(c=>c.task))].map(task=><tr key={task}><th scope="row" title={task}>{taskLabel(task)}</th>{configs.map(([key])=><td key={key}>{(grouped.get(JSON.stringify([task,key]))??[]).map((c,_i,repeats)=><SettingsButton key={c.id} className={"evals-task-result "+c.status} aria-label={`${task} · ${c.engine} · ${formatModelRef(settings,c.model).title} · cg:${c.codegraph?"on":"off"} · repeat ${c.repeat+1} · ${c.status}`} title={c.output.slice(-2000)||statusLabel(c.status)} onClick={()=>onTranscript(c)}><span className="evals-result-status"><span aria-hidden>●</span> {statusLabel(c.status)}{repeats.length>1&&<small> #{c.repeat+1}</small>}</span>{c.turns>0&&<small>{tokens(c.usage.tokensIn+c.usage.cachedTokens+c.usage.tokensOut)} tokens · {metric("wallMs",c.usage.wallMs)}</small>}</SettingsButton>)}</td>)}</tr>)}</tbody></table></div>
  </>}
 </>;
}
