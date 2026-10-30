import type {AppSettings} from "@/types";
export type EvalTask = {id:string; category:string; tags:string[]; requires:string[]};
export type EvalPlan = {engines:string[]; models:string[]; effort?:string|null; tags:string[]; categories:string[]; codegraph:boolean[]; repeats:number; concurrency:number; maxTotalTokens?:number; maxCost?:number};
export type EvalCell = {id:string; task:string; engine:string; model:string; codegraph:boolean; repeat:number; status:string; turns:number; threadId:string|null; output:string; answer:string; transcript:unknown[]; workdir:string|null; usage:{tokensIn:number;cachedTokens:number;tokensOut:number;costUsd:number|null;toolCalls:number;errors:string[];engineReadyMs:number;firstTokenMs:number|null;wallMs:number}};
export type EvalSummary = {config:string; engine:string; model:string; codegraph:boolean; passRate:number; tokens:number; cacheRate:number; costUsd:number|null; firstTokenMs:number|null; wallMs:number; toolCalls:number; errors:number};
export type EvalRun = {id:string; createdAt:string; status:string; message?:string|null; plan:EvalPlan; cells:EvalCell[]; environment:{appVersion?:string;engines?:{engine:string;version?:string|null}[];agentPolicy?:AppSettings["agentPolicy"]}};
export type EvalHistory = Omit<EvalRun,"cells"|"environment"> & {summary:EvalSummary[]};
export type EvalResult = {run:EvalRun;summary:EvalSummary[]};
export const configKey=(c:Pick<EvalCell,"engine"|"model"|"codegraph">)=>[c.engine,c.model,c.codegraph].join("/");
export const percent=(n:number)=>`${(n*100).toFixed(1)}%`;
export const tokens=(n:number)=>`${(n/1000).toFixed(1)}k`;
export function metric(key:keyof EvalSummary,value:number|null) {
 if(value===null)return "unknown";
 if(key==="passRate"||key==="cacheRate")return percent(value);
 if(key==="tokens")return tokens(value);
 if(key==="wallMs"||key==="firstTokenMs")return `${(value/1000).toFixed(1)}s`;
 if(key==="costUsd")return `$${value.toFixed(4)}`;
 return value.toLocaleString();
}
export function planCount(tasks:EvalTask[],plan:EvalPlan) {
 const count=tasks.filter(t=>(!plan.tags.length||t.tags.some(tag=>plan.tags.includes(tag)))&&(!plan.categories.length||plan.categories.includes(t.category))).length;
 return {tasks:count,cells:count*plan.engines.length*plan.models.length*plan.codegraph.length*plan.repeats};
}
export function roleSettings(settings:AppSettings,model:string,role:string):AppSettings {
 const split=model.indexOf("/"); if(split<1||split===model.length-1) throw new Error("Invalid model reference");
 const ref={providerId:model.slice(0,split),modelId:model.slice(split+1)};
 const policy=settings.agentPolicy; if(!policy) throw new Error("Agent policy unavailable");
 if(role==="default") return {...settings,agentPolicy:{...policy,defaultModel:ref}};
 if(!policy.subagents.roles.some(r=>r.name===role)) throw new Error("Unknown role");
 return {...settings,agentPolicy:{...policy,subagents:{...policy.subagents,roles:policy.subagents.roles.map(r=>r.name===role?{...r,model:ref}:r)}}};
}

