import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import {Puzzle,Sparkles,Terminal} from "lucide-react";
import {SettingsButton} from "@/features/design-system/components/settings/SettingsPrimitives";
import {configKey,metric,tokens,type EvalCell,type EvalSummary} from "./evals";

export function EvalEngineIcon({engine}:{engine:string}) {
 const Icon=engine==="codex"?Terminal:engine==="claude"?Sparkles:Puzzle;
 return <span className="evals-engine-icon" aria-hidden>{engine==="pi"?"π":engine==="omp"?"ϖ":<Icon size={16}/>}</span>;
}
const columns: {key:keyof EvalSummary;label:string;high?:boolean}[]=[
 {key:"passRate",label:"Pass",high:true},{key:"tokens",label:"Tokens (eff.)"},{key:"cacheRate",label:"Cache",high:true},
 {key:"costUsd",label:"Cost"},{key:"toolCalls",label:"Tools"},{key:"firstTokenMs",label:"1st token"},{key:"wallMs",label:"Wall"},{key:"errors",label:"Errors"},
];
export function EvalsResults({rows,cells,full,sort,ascending,expanded,onSort,onConfig,onTranscript}:{rows:EvalSummary[];cells:EvalCell[];full:boolean;sort:keyof EvalSummary;ascending:boolean;expanded:string|null;onSort:(key:keyof EvalSummary,high?:boolean)=>void;onConfig:(key:string)=>void;onTranscript:(cell:EvalCell)=>void}) {
 const settings = useFeatureSettings();
 const best=(row:EvalSummary,key:keyof EvalSummary,high?:boolean)=>{
  const values=rows.map(r=>r[key]).filter((v):v is number=>typeof v==="number");
  return row[key]!==null&&row[key]===(high?Math.max(...values):Math.min(...values))?"evals-best":undefined;
 };
 const config=(row:EvalSummary)=><><span className="evals-config-engine"><EvalEngineIcon engine={row.engine}/>{row.engine}</span><span className="evals-config-model">{formatModelRef(settings, row.model).name}</span><span className="evals-cg">cg:{row.codegraph?"on":"off"}</span></>;
 const grouped=new Map<string,EvalCell[]>();
 for(const cell of cells){const key=JSON.stringify([cell.task,configKey(cell)]);const group=grouped.get(key)??[];group.push(cell);grouped.set(key,group);}
 const configs=[...new Map(cells.map(c=>[configKey(c),c])).entries()];
 return <>
  {!full?<div className="evals-summary-cards" aria-label="Configuration summaries">{rows.map(row=><article className="evals-summary-card" aria-label={`${row.engine} / ${formatModelRef(settings, row.model).name} · cg:${row.codegraph?"on":"off"}`} key={row.config}>
   <SettingsButton className="evals-config" aria-label={row.engine+" / "+formatModelRef(settings, row.model).title+" · cg:"+(row.codegraph?"on":"off")} aria-expanded={expanded===row.config} onClick={()=>onConfig(row.config)}>{config(row)}</SettingsButton>
   <div className={"evals-pass-rate "+(best(row,"passRate",true)??"")}><strong>{metric("passRate",row.passRate)}</strong><span>pass rate</span></div>
   <dl className="evals-metrics">{columns.filter(c=>["tokens","cacheRate","costUsd","toolCalls","wallMs"].includes(c.key)).map(c=><div key={c.key}><dt>{c.key==="tokens"?"Median tokens":c.key==="wallMs"?"Median wall":c.label}</dt><dd className={best(row,c.key,c.high)}>{metric(c.key,row[c.key] as number|null)}</dd></div>)}</dl>
  </article>)}</div>:<>
   <div className="evals-table-scroll" tabIndex={0} role="region" aria-label="Scrollable configuration comparison"><table aria-label="Configuration comparison"><thead><tr><th scope="col">Configuration</th>{columns.map(c=><th scope="col" key={c.key} aria-sort={sort===c.key?(ascending?"ascending":"descending"):"none"}><SettingsButton onClick={()=>onSort(c.key,c.high)}>{c.label}</SettingsButton></th>)}</tr></thead><tbody>{rows.map(row=><tr key={row.config}><th scope="row"><SettingsButton className="evals-config" aria-label={row.engine+" / "+formatModelRef(settings, row.model).title+" · cg:"+(row.codegraph?"on":"off")} aria-expanded={expanded===row.config} onClick={()=>onConfig(row.config)}>{config(row)}</SettingsButton></th>{columns.map(c=><td key={c.key} className={best(row,c.key,c.high)}>{metric(c.key,row[c.key] as number|null)}</td>)}</tr>)}</tbody></table></div>
   <h3 className="evals-matrix-title">Per-task results</h3>
   <div className="evals-table-scroll" tabIndex={0} role="region" aria-label="Scrollable task results"><table aria-label="Task by configuration"><thead><tr><th scope="col">Task</th>{configs.map(([key,c])=><th scope="col" key={key}>{c.engine} / {formatModelRef(settings, c.model).name}<small>cg:{c.codegraph?"on":"off"}</small></th>)}</tr></thead><tbody>{[...new Set(cells.map(c=>c.task))].map(task=><tr key={task}><th scope="row">{task}</th>{configs.map(([key])=><td key={key}>{(grouped.get(JSON.stringify([task,key]))??[]).map((c,_i,repeats)=><SettingsButton key={c.id} className={"evals-task-result "+c.status} aria-label={`${task} · ${c.engine} · ${formatModelRef(settings,c.model).title} · cg:${c.codegraph?"on":"off"} · repeat ${c.repeat+1} · ${c.status}`} title={c.output.slice(-2000)||c.status} onClick={()=>onTranscript(c)}><span aria-hidden>●</span>{c.turns?tokens(c.usage.tokensIn+c.usage.cachedTokens+c.usage.tokensOut):"—"}{repeats.length>1&&<small>#{c.repeat+1}</small>}</SettingsButton>)}</td>)}</tr>)}</tbody></table></div>
  </>}
 </>;
}
