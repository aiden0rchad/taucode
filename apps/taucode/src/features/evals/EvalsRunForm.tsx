import { SettingsIconButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import {useEffect,useId,useLayoutEffect,useRef,useState,type CSSProperties} from "react";
import {createPortal} from "react-dom";
import {Check,Minus,Plus,Search,X} from "lucide-react";
import {EvalEngineIcon} from "./EvalsResults";
import type {EngineRuntimeStatus} from "@services/tauri";
import {PopoverSurface} from "@/features/design-system/components/popover/PopoverPrimitives";
import {SettingsButton,SettingsFieldRow,SettingsSegmented,SettingsSelect,SettingsTextField} from "@/features/design-system/components/settings/SettingsPrimitives";
import {reasoningLabel,reasoningLevels} from "@/features/models/reasoning";
import type {EvalPlan,EvalTask} from "./evals";

type Model={id:string;name:string};
function ModelPicker({models,selected,onChange}:{models:Model[];selected:string[];onChange:(ids:string[])=>void}) {
 const settings = useFeatureSettings();
 const [open,setOpen]=useState(false);
 const [query,setQuery]=useState("");
 const [position,setPosition]=useState<CSSProperties>({});
 const trigger=useRef<HTMLDivElement>(null);
 const panel=useRef<HTMLDivElement>(null);
 const id=useId();
 const filtered=models.filter(m=>(m.name+" "+m.id).toLowerCase().includes(query.toLowerCase()));
 const toggle=(id:string)=>onChange(selected.includes(id)?selected.filter(m=>m!==id):[...selected,id]);
 useLayoutEffect(()=>{
  if(!open)return;
  const place=()=>{const rect=trigger.current!.getBoundingClientRect();const below=window.innerHeight-rect.bottom-16;const above=rect.top-16;setPosition({position:"fixed",left:Math.max(12,Math.min(rect.left,window.innerWidth-440)),width:Math.min(416,window.innerWidth-24),...(below>=Math.min(320,above)?{top:rect.bottom+6}:{bottom:window.innerHeight-rect.top+6}),maxHeight:Math.max(120,Math.min(360,Math.max(below,above))),zIndex:trigger.current?.closest(".ds-modal")?"var(--z-tooltip)":"var(--z-popover)"});};
  place();window.addEventListener("resize",place);window.addEventListener("scroll",place,true);
  return()=>{window.removeEventListener("resize",place);window.removeEventListener("scroll",place,true);};
 },[open]);
 useEffect(()=>{
  if(!open)return;
  panel.current?.querySelector("input")?.focus();
  const dismiss=(event:PointerEvent)=>{if(!panel.current?.contains(event.target as Node)&&!trigger.current?.contains(event.target as Node))setOpen(false);};
  document.addEventListener("pointerdown",dismiss);return()=>document.removeEventListener("pointerdown",dismiss);
 },[open]);
 return <div className="evals-models">
  <div className="evals-chips" aria-label="Selected models">{selected.map(id=><SettingsButton key={id} className="evals-chip" aria-label={`Remove model ${models.find(m=>m.id===id)?.name??formatModelRef(settings,id).name}`} onClick={()=>toggle(id)}><span>{models.find(m=>m.id===id)?.name??formatModelRef(settings,id).name}</span><X size={14} aria-hidden /></SettingsButton>)}</div>
  <div ref={trigger}><SettingsButton aria-haspopup="dialog" aria-expanded={open} aria-controls={open?id:undefined} onClick={()=>{setQuery("");setOpen(!open);}}><Search size={14} aria-hidden /> Choose models</SettingsButton></div>
  {open&&createPortal(<PopoverSurface ref={panel} id={id} role="dialog" aria-label="Choose models" className="settings-page evals-model-popover" style={position} onKeyDown={e=>{if(e.key==="Escape"){e.preventDefault();e.stopPropagation();setOpen(false);trigger.current?.querySelector("button")?.focus();}}}>
    <div className="evals-model-search"><SettingsTextField aria-label="Search models" placeholder="Search provider or model" value={query} onChange={e=>setQuery(e.target.value)} /><SettingsIconButton aria-label="Close model picker" onClick={()=>{setOpen(false);trigger.current?.querySelector("button")?.focus();}}><X size={14} aria-hidden /></SettingsIconButton></div>
    <div className="evals-model-options" role="listbox" aria-label="Models" aria-multiselectable="true">{filtered.map(m=><SettingsButton role="option" aria-selected={selected.includes(m.id)} key={m.id} onClick={()=>toggle(m.id)}><span><strong>{m.name}</strong></span><Check size={16} aria-hidden style={{visibility:selected.includes(m.id)?"visible":"hidden"}} /></SettingsButton>)}{!filtered.length&&<p>No matching models</p>}</div>
  </PopoverSurface>,document.body)}
 </div>;
}
export function EvalsRunForm({plan,onChange,tasks,engines,models,runtimes}:{plan:EvalPlan;onChange:(p:EvalPlan)=>void;tasks:EvalTask[];engines:{id:string;name:string}[];models:Model[];runtimes:EngineRuntimeStatus[]}) {
 const toggle=(key:"engines"|"tags"|"categories",id:string)=>onChange({...plan,[key]:plan[key].includes(id)?plan[key].filter(v=>v!==id):[...plan[key],id]});
 return <div className="evals-fields">
  <section className="evals-setup-section" aria-label="Models and engines">
   <h4><span>1</span> Models & engines</h4>
   <p>Each model runs the same tasks on every selected engine.</p>
   <SettingsFieldRow className="settings-field-row-long" title="Models"><ModelPicker models={models} selected={plan.models} onChange={ids=>onChange({...plan,models:ids})}/></SettingsFieldRow>
   <SettingsFieldRow className="settings-field-row-long" title="Engines"><div className="evals-chips" role="group" aria-label="Engines">{engines.map(engine=>{
    const status=runtimes.find(r=>r.engine===(engine.id==="claude"?"claude-acp":engine.id));
    const state=status?.state??"unknown";
    return <SettingsButton key={engine.id} className="evals-chip" aria-pressed={plan.engines.includes(engine.id)} title={status?.message} onClick={()=>toggle("engines",engine.id)}><EvalEngineIcon engine={engine.id}/><span>{engine.name}</span><span className={"evals-status-dot "+state} aria-hidden /><small>{state==="pending"?"will install":state}</small></SettingsButton>;
   })}</div></SettingsFieldRow>
   <SettingsFieldRow className="settings-field-row-long" title="Reasoning" subtitle="Uses the nearest supported level for each model."><SettingsSelect aria-label="Reasoning" value={plan.effort??""} onChange={e=>onChange({...plan,effort:e.target.value||undefined})}><option value="">Model default</option>{reasoningLevels.map(level=><option key={level} value={level}>{reasoningLabel(level)}</option>)}</SettingsSelect></SettingsFieldRow>
  </section>
  <section className="evals-setup-section" aria-label="Test tasks">
   <h4><span>2</span> Test tasks</h4>
   <p>Use Smoke for a quick check. Clear a filter to include all tasks.</p>
   {(["tags","categories"] as const).map(key=><SettingsFieldRow className="settings-field-row-long" key={key} title={key==="tags"?"Task sets":"Task types"}><div className="evals-chips" role="group" aria-label={key==="tags"?"Tags":"Categories"}>{[...new Set(tasks.flatMap(t=>key==="tags"?t.tags:[t.category]))].map(value=><SettingsButton key={value} className="evals-chip" aria-pressed={plan[key].includes(value)} onClick={()=>toggle(key,value)}>{value==="computerUse"?"Computer use":value.charAt(0).toUpperCase()+value.slice(1)}</SettingsButton>)}</div></SettingsFieldRow>)}
   <SettingsFieldRow className="settings-field-row-long" title="CodeGraph" subtitle="Compare with or without code search context."><SettingsSegmented label="Codegraph" value={plan.codegraph.length===2?"both":plan.codegraph[0]?"on":"off"} options={[{value:"off",label:"Off"},{value:"on",label:"On"},{value:"both",label:"Compare both"}]} onChange={v=>onChange({...plan,codegraph:v==="both"?[false,true]:[v==="on"]})}/></SettingsFieldRow>
  </section>
  <section className="evals-setup-section evals-limits" aria-label="Run limits">
   <h4><span>3</span> Run limits</h4>
   <p>Caps apply to the whole run. Leave blank for no cap; usage already in flight can exceed it.</p>
   <div className="evals-numeric-fields">{(["repeats","concurrency"] as const).map(key=>{
    const title=key==="repeats"?"Repeats":"Concurrency";const max=key==="repeats"?100:6;
    return <SettingsFieldRow className="settings-field-row-long" key={key} title={title} subtitle={key==="repeats"?"Attempts per task and configuration":"Tests running at the same time"}><div className="evals-stepper" role="group" aria-label={title}><SettingsIconButton aria-label={`Decrease ${title.toLowerCase()}`} disabled={plan[key]<=1} onClick={()=>onChange({...plan,[key]:plan[key]-1})}><Minus size={14} aria-hidden/></SettingsIconButton><SettingsTextField aria-label={title} type="number" min={1} max={max} value={plan[key]} onChange={e=>{const n=Number(e.target.value);if(Number.isInteger(n)&&n>=1&&n<=max)onChange({...plan,[key]:n});}}/><SettingsIconButton aria-label={`Increase ${title.toLowerCase()}`} disabled={plan[key]>=max} onClick={()=>onChange({...plan,[key]:plan[key]+1})}><Plus size={14} aria-hidden/></SettingsIconButton></div></SettingsFieldRow>;
   })}
   {(["maxTotalTokens","maxCost"] as const).map(key=><SettingsFieldRow className="settings-field-row-long" key={key} title={key==="maxCost"?"Cost cap":"Token cap"} subtitle={key==="maxCost"?"Requires provider pricing":"Includes cached input tokens"}><div className="evals-budget"><SettingsTextField type="number" min={key==="maxCost"?0.01:1} step={key==="maxCost"?0.01:1} value={plan[key]??""} placeholder="No cap" onChange={e=>onChange({...plan,[key]:e.target.value?Number(e.target.value):undefined})}/><span>{key==="maxCost"?"USD":"tokens"}</span></div></SettingsFieldRow>)}
   </div>
  </section>
 </div>;
}
