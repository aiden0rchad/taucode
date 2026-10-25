import type {EvalResult,EvalPlan,EvalTask,EvalHistory} from "@/features/evals/evals";
import {planCount} from "@/features/evals/evals";
export function createEvalFixture(publish:(workspace:string,method:string,params:Record<string,unknown>)=>void) {
 const tasks:EvalTask[]=[{id:"ts-bugfix-range",category:"bugfix",tags:["smoke"],requires:[]},{id:"py-feature-frequency",category:"feature",tags:["smoke"],requires:[]}];
 const plan:EvalPlan={engines:["pi"],models:["openai/gpt-5"],tags:["smoke"],categories:[],codegraph:[false],repeats:1,concurrency:2};
 const runs:EvalResult[]=["eval-a","eval-b","eval-live"].map((id,index)=>({run:{id,createdAt:`2026-10-2${index+1}T12:00:00Z`,status:index===2?"running":"completed",plan,environment:{appVersion:"preview"},cells:tasks.map((t,i)=>({id:"cell-"+i,task:t.id,engine:"pi",model:"openai/gpt-5",codegraph:false,repeat:0,status:index===2?(i?"queued":"running"):index===1&&i===1?"failed":"passed",turns:1,threadId:"preview-thread-"+i,output:index===1&&i===1?"AssertionError: expected a frequency map":"All checks passed",answer:"Done",transcript:[{method:"item/completed",params:{item:{type:"agentMessage",text:"Implemented the task and ran its check."}}}],workdir:null,usage:{tokensIn:1000+index*500,cachedTokens:500,tokensOut:100,costUsd:0.01,toolCalls:2,errors:[],engineReadyMs:100,firstTokenMs:300,wallMs:1500}}))},summary:index===2?[]:[{config:"pi/openai/gpt-5/false",engine:"pi",model:"openai/gpt-5",codegraph:false,passRate:index===1?0.5:1,tokens:1600+index*500,cacheRate:0.33,costUsd:0.02,firstTokenMs:300,wallMs:1500,toolCalls:4,errors:0}]}));
 return (action:string,p:Record<string,unknown>)=>{
  if(action==="suites")return tasks;
  if(action==="runs")return runs.map(({run,summary}):EvalHistory=>({...run,summary})).reverse();
  if(action==="plan")return {...planCount(tasks,p as EvalPlan),estimatedTokens:3200};
  if(action==="run"||action==="status")return runs.find(r=>r.run.id===p.runId);
  if(action==="start") {const live=runs.find(r=>r.run.id==="eval-live")!;live.run.status="running";publish("evals","eval/progress",{runId:live.run.id,status:"running",cells:live.run.cells});return {runId:live.run.id};}
  if(action==="cancel") {const live=runs.find(r=>r.run.id===p.runId)!;live.run.status="cancelled";live.run.cells.forEach(c=>c.status="cancelled");publish("evals","eval/progress",{runId:live.run.id,status:"cancelled",cells:live.run.cells});return {};}
  if(action==="delete") {const i=runs.findIndex(r=>r.run.id===p.runId);if(i>=0)runs.splice(i,1);return {};}
  throw new Error("Unknown preview eval action");
 };
}

