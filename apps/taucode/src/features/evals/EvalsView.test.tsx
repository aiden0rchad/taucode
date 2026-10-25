// @vitest-environment jsdom
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {EvalsView} from "./EvalsView";
import {planCount,roleSettings,metric,type EvalResult} from "./evals";
import {createFixtures} from "@/dev/uiPreview/fixtures";
import {createEvalFixture} from "@/dev/uiPreview/evals";
import {evalRequest,engineRuntime} from "@services/tauri";
const evalStyles=(await vi.importActual<{readFileSync:(path:string,encoding:"utf8")=>string}>("node:fs")).readFileSync("src/features/evals/evals.css","utf8");
const settingsStyles=(await vi.importActual<{readFileSync:(path:string,encoding:"utf8")=>string}>("node:fs")).readFileSync("src/styles/settings.css","utf8");
let progress:(event:{runId:string;status:string})=>void;
vi.mock("@services/tauri",()=>({evalRequest:vi.fn(),engineRuntime:vi.fn()}));
vi.mock("@services/events",()=>({subscribeEvalProgress:vi.fn(fn=>{progress=fn;return()=>{};})}));
const settings=createFixtures().settings;
let fixture:ReturnType<typeof createEvalFixture>;
beforeEach(()=>{vi.mocked(engineRuntime).mockResolvedValue([{engine:"pi",state:"pending",message:"Ready to set up on first use"}]);fixture=createEvalFixture((_w,_m,p)=>progress?.(p as {runId:string;status:string}));vi.mocked(evalRequest).mockImplementation(async(a,p)=>fixture(a,p as Record<string,unknown>) as never);});
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("counts the matrix and requires explicit confirmation before spending",async()=>{
 render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 await screen.findByText(/2 cells · 2 tasks/);
 fireEvent.click(screen.getByRole("button",{name:"Start run"}));
 expect(vi.mocked(evalRequest).mock.calls.some(([a])=>a==="start")).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Confirm start"}));
 await screen.findByText("Run · running");
 expect(screen.getByLabelText("Cell progress").textContent).toContain("running");
 fixture("cancel",{runId:"eval-live"});
 await screen.findByText("Run · cancelled");
});
it("shows A/B deltas, sortable highlighted comparisons and read-only transcripts",async()=>{
 render(<EvalsView full appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 const history=await screen.findByRole("button",{name:/10\/22\/2026.*completed/});
 fireEvent.click(history);
 await screen.findByText("Run · completed");
 fireEvent.change(screen.getByLabelText("Compare against run A"),{target:{value:"eval-a"}});
 await screen.findByRole("table",{name:"A/B deltas"});
 expect(within(screen.getByRole("table",{name:"A/B deltas"})).getByText("-50.0%").className).toBe("evals-regression");
 fireEvent.click(screen.getByRole("button",{name:"Tokens (eff.)"}));
 expect(screen.getByRole("button",{name:"Tokens (eff.)"}).closest("th")?.getAttribute("aria-sort")).toBe("ascending");
 expect(document.querySelector(".evals-best")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:/pi \/ openai\/gpt-5/}));
 fireEvent.click(screen.getAllByRole("button",{name:"Open transcript (read-only)"})[0]);
 expect(screen.getByLabelText("Read-only transcript").textContent).toContain("Implemented the task");
});
it("confirms role writes and preserves other policy fields",async()=>{
 const update=vi.fn().mockResolvedValue(undefined);
 render(<EvalsView appSettings={settings} onUpdateAppSettings={update} />);
 fireEvent.click(await screen.findByRole("button",{name:/10\/21\/2026.*completed/}));
 await screen.findByText("Run · completed");
 fireEvent.click(screen.getByRole("button",{name:/pi \/ openai\/gpt-5/}));
 fireEvent.click(screen.getByRole("button",{name:"Set as default for role"}));
 expect(update).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Confirm default"}));
 await waitFor(()=>expect(update).toHaveBeenCalledTimes(1));
 expect(update.mock.calls[0][0].agentPolicy.defaultModel).toEqual({providerId:"openai",modelId:"gpt-5"});
 expect(update.mock.calls[0][0].agentPolicy.subagents).toEqual(settings.agentPolicy?.subagents);
});
it("handles filters and named role model refs containing slashes",()=>{
 const plan={engines:["pi","omp"],models:["p/m","p/n"],codegraph:[false,true],repeats:2,concurrency:2,tags:["smoke"],categories:["bugfix"]};
 expect(planCount([{id:"a",tags:["smoke"],category:"bugfix",requires:[]},{id:"b",tags:[],category:"bugfix",requires:[]}],plan)).toEqual({tasks:1,cells:16});
 const policy=settings.agentPolicy!;
 const s={...settings,agentPolicy:{...policy,subagents:{...policy.subagents,roles:[{name:"worker",description:"",instructions:"keep",toolAccess:"full" as const,model:null}]}}};
 const next=roleSettings(s,"lab/org/model","worker");
 expect(next.agentPolicy?.subagents.roles[0]).toMatchObject({instructions:"keep",model:{providerId:"lab",modelId:"org/model"}});
 expect(s.agentPolicy.subagents.roles[0].model).toBeNull();
});
it("sorts multiple configurations independently",async()=>{
 const base=fixture("run",{runId:"eval-a"}) as EvalResult;
 base.summary[1]={...base.summary[1],tokens:100,passRate:0.5};
 render(<EvalsView full appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 fireEvent.click(await screen.findByRole("button",{name:/10\/21\/2026.*completed/}));
 await screen.findByText("Run · completed");
 fireEvent.click(screen.getByRole("button",{name:"Tokens (eff.)"}));
 const table=screen.getByRole("button",{name:"Tokens (eff.)"}).closest("table")!;
 expect(within(table).getAllByRole("row")[1].textContent).toContain("omp");
});


it.each([1024,1440])("uses chips, searchable models and untruncated labels at %ipx",async width=>{
 vi.stubGlobal("innerWidth",width);
 const {container}=render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 await screen.findByText(/2 cells · 2 tasks/);
 const form=container.querySelector(".evals-new-run")!;
 expect(form.querySelector("select[multiple]")).toBeNull();
 for(const label of form.querySelectorAll("label")) expect(label.scrollWidth).toBeLessThanOrEqual(label.clientWidth);
 expect(screen.getByText("will install")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"Choose models"}));
 const dialog=screen.getByRole("dialog",{name:"Choose models"});
 fireEvent.change(within(dialog).getByRole("textbox",{name:"Search models"}),{target:{value:"gpt-5-mini"}});
 const options=within(dialog).getAllByRole("option");
 expect(options.every(option=>option.textContent?.includes("gpt-5-mini"))).toBe(true);
 fireEvent.click(options[0]);
 expect(options[0].getAttribute("aria-selected")).toBe("true");
 fireEvent.keyDown(dialog,{key:"Escape"});
 expect(screen.queryByRole("dialog",{name:"Choose models"})).toBeNull();
 expect(document.activeElement).toBe(screen.getByRole("button",{name:"Choose models"}));
 await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"Increase repeats"}));});
 expect((screen.getByRole("spinbutton",{name:"Repeats"}) as HTMLInputElement).value).toBe("2");
 vi.unstubAllGlobals();
});
it("keeps preparing runs active and displays the installation message",async()=>{
 const base=fixture("run",{runId:"eval-live"}) as EvalResult;
 base.run.status="preparing";base.run.message="Preparing pi…";
 render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()}/>);
 fireEvent.click(await screen.findByRole("button",{name:/10\/23\/2026.*preparing/}));
 await screen.findByText("Preparing pi…");
 expect((screen.getByRole("button",{name:"Start run"}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.getByRole("button",{name:"Cancel run"})).toBeTruthy();
});

it("renders one Settings summary card per configuration and opens that run",async()=>{
 const open=vi.fn();
 render(<EvalsView initialRunId="eval-b" appSettings={settings} onUpdateAppSettings={vi.fn()} onOpenFull={open}/>);
 const cards=await screen.findByLabelText("Configuration summaries");
 const result=fixture("run",{runId:"eval-b"}) as EvalResult;
 expect(within(cards).getAllByRole("article")).toHaveLength(result.summary.length);
 for(const row of result.summary) expect(within(cards).getByRole("article",{name:`${row.engine} / ${row.model} · cg:off`})).toBeTruthy();
 expect(within(cards).getByText("50.0%")).toBeTruthy();
 expect(within(cards).getByText("unknown")).toBeTruthy();
 expect(screen.getByText("App preview · pi 1.4.0 · omp 18.12.6")).toBeTruthy();
 expect(screen.queryByRole("table",{name:"Configuration comparison"})).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"Open results"}));
 expect(open).toHaveBeenCalledWith("eval-b");
});

it.each([1024,1440])("keeps full results headers unwrapped at %ipx and opens matrix transcripts",async width=>{
 vi.stubGlobal("innerWidth",width);
 const result=fixture("run",{runId:"eval-b"}) as EvalResult;
 result.run.cells[1].output="x".repeat(3000)+"AssertionError: frequency map";
 result.run.cells.push({...result.run.cells[0],id:"repeat-2",repeat:1});
 render(<><style>{settingsStyles+evalStyles}</style><EvalsView full initialRunId="eval-b" appSettings={settings} onUpdateAppSettings={vi.fn()}/></>);
 const table=await screen.findByRole("table",{name:"Configuration comparison"});
 expect(within(table).getAllByRole("columnheader").map(h=>h.textContent)).toEqual(["Configuration","Pass","Tokens (eff.)","Cache","Cost","Tools","1st token","Wall","Errors"]);
 for(const header of table.querySelectorAll("th,th button")) expect(getComputedStyle(header).whiteSpace).toBe("nowrap");
 for(const header of table.querySelectorAll("thead th")) {
  const style=getComputedStyle(header);
  expect(style.fontSize).toBe("11px");expect(style.textTransform).toBe("uppercase");
 }
 for(const button of table.querySelectorAll("thead button")) {
  const style=getComputedStyle(button);
  expect(style.backgroundColor).toBe("rgba(0, 0, 0, 0)");expect(style.borderRadius).toBe("0");expect(style.boxShadow).toBe("none");
 }
 expect(getComputedStyle(table.querySelector("th")!).position).toBe("sticky");
 expect(getComputedStyle(table.parentElement!).overflowX).toBe("auto");
 expect(within(table).getByText("2.1k")).toBeTruthy();
 expect(within(table).getByText("1.5s")).toBeTruthy();
 expect(screen.queryByLabelText("Configuration summaries")).toBeNull();
 const matrix=screen.getByRole("table",{name:"Task by configuration"});
 expect(within(matrix).getAllByRole("row")).toHaveLength(3);
 expect(within(matrix).getAllByRole("button")).toHaveLength(5);
 const failed=within(matrix).getByRole("button",{name:/failed/});
 expect(failed.title).toHaveLength(2000);
 expect(failed.title.endsWith("AssertionError: frequency map")).toBe(true);
 fireEvent.click(failed);
 expect(screen.getByLabelText("Read-only transcript").textContent).toContain("Implemented the task");
 vi.unstubAllGlobals();
});

it("formats comparison units without converting unknown values to zero",()=>{
 expect(metric("tokens",36600)).toBe("36.6k");
 expect(metric("firstTokenMs",18500)).toBe("18.5s");
 expect(metric("costUsd",null)).toBe("unknown");
 expect(metric("costUsd",0)).toBe("$0.0000");
 expect(metric("cacheRate",0.375)).toBe("37.5%");
});
