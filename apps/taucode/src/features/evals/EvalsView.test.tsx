// @vitest-environment jsdom
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {EvalsView} from "./EvalsView";
import {planCount,roleSettings,type EvalResult} from "./evals";
import {createFixtures} from "@/dev/uiPreview/fixtures";
import {createEvalFixture} from "@/dev/uiPreview/evals";
import {evalRequest} from "@services/tauri";
let progress:(event:{runId:string;status:string})=>void;
vi.mock("@services/tauri",()=>({evalRequest:vi.fn()}));
vi.mock("@services/events",()=>({subscribeEvalProgress:vi.fn(fn=>{progress=fn;return()=>{};})}));
const settings=createFixtures().settings;
let fixture:ReturnType<typeof createEvalFixture>;
beforeEach(()=>{fixture=createEvalFixture((_w,_m,p)=>progress?.(p as {runId:string;status:string}));vi.mocked(evalRequest).mockImplementation(async(a,p)=>fixture(a,p as Record<string,unknown>) as never);});
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("counts the matrix and requires explicit confirmation before spending",async()=>{
 render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 await screen.findByText(/2 cells · 2 tasks/);
 fireEvent.click(screen.getByRole("button",{name:"Start"}));
 expect(vi.mocked(evalRequest).mock.calls.some(([a])=>a==="start")).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Confirm start"}));
 await screen.findByText("Run · running");
 expect(screen.getByLabelText("Cell progress").textContent).toContain("running");
 fixture("cancel",{runId:"eval-live"});
 await screen.findByText("Run · cancelled");
});
it("shows A/B deltas, sortable highlighted comparisons and read-only transcripts",async()=>{
 render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 const history=await screen.findByRole("button",{name:/10\/22\/2026.*completed/});
 fireEvent.click(history);
 await screen.findByText("Run · completed");
 fireEvent.change(screen.getByLabelText("Compare against run A"),{target:{value:"eval-a"}});
 await screen.findByRole("table",{name:"A/B deltas"});
 expect(screen.getByRole("table",{name:"A/B deltas"}).querySelector(".evals-regression")?.textContent).toBe("-50.0%");
 fireEvent.click(screen.getByRole("button",{name:"Tokens"}));
 expect(screen.getByRole("button",{name:"Tokens"}).closest("th")?.getAttribute("aria-sort")).toBe("ascending");
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
 base.summary.push({...base.summary[0],config:"omp/openai/gpt-5/false",engine:"omp",tokens:100,passRate:0.5});
 render(<EvalsView appSettings={settings} onUpdateAppSettings={vi.fn()} />);
 fireEvent.click(await screen.findByRole("button",{name:/10\/21\/2026.*completed/}));
 await screen.findByText("Run · completed");
 fireEvent.click(screen.getByRole("button",{name:"Tokens"}));
 const table=screen.getByRole("button",{name:"Tokens"}).closest("table")!;
 expect(within(table).getAllByRole("row")[1].textContent).toContain("omp");
});

