import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, readdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { acquireSubagentSlot, commandInstructions, commandTargets, createSubagentBudget, releaseSubagentSlot, resolveTask, readPolicy, type Policy } from "../extensions/lib/policy.ts";
import subagents from "../extensions/taucode-subagents.ts";

const policy: Policy = {
  mode: "chain-of-command", enabled: true, maxConcurrent: 2, maxPerTurn: 5, maxDepth: 2,
  allowedModels: ["cloud/mid", "cloud/strong", "local/small"], defaultModel: "local/small",
  models: { "cloud/mid": "taucode-cloud/mid", "cloud/strong": "taucode-cloud/strong", "local/small": "taucode-local/small" },
  roles: [
    { name: "colonel", description: "Delegate", model: "cloud/mid", toolAccess: "full", instructions: "" },
    { name: "major", description: "QA", model: "cloud/strong", toolAccess: "full", instructions: "" },
    { name: "enlisted", description: "Implement", toolAccess: "full", instructions: "" },
    { name: "custom", description: "Legacy", toolAccess: "full", instructions: "" },
  ],
};

test("command roles enforce delegation edges and configured cloud/local models", () => {
  for (const parent of ["commander", "colonel", "major", "enlisted", "unknown"]) {
    for (const role of policy.roles) {
      const allowed = parent === "commander" && role.name !== "custom" || parent === "colonel" && ["major", "enlisted"].includes(role.name);
      const resolve = () => resolveTask(policy, { agent: role.name }, "cloud/strong", parent);
      if (allowed) assert.ok(resolve()); else assert.throws(resolve, /cannot delegate/);
    }
  }
  assert.equal(resolveTask(policy, { agent: "enlisted" }).provider, "taucode-local");
  assert.equal(resolveTask(policy, { agent: "major" }).modelId, "strong");
  assert.throws(() => resolveTask(policy, { agent: "enlisted", model: "cloud/strong" }), /configured enlisted model/);
  assert.equal(resolveTask(policy, { agent: "enlisted", model: "local/small" }).model, "local/small");
  assert.match(commandInstructions("commander"), /independent verification/);
  assert.match(commandInstructions("major"), /Do not claim a check ran when it did not/);
});

test("nested subprocesses share one turn budget, reset it, route models and fail closed", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "taucode-command-"));
  const home = join(root, "agent"), cwd = join(root, "workspace"), cli = join(root, "fake-cli.mjs");
  mkdirSync(join(home, "taucode"), { recursive: true }); mkdirSync(cwd);
  const variables = { PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: cwd, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: "0", TAUCODE_SUBAGENT_ROLE: "", TAUCODE_SUBAGENT_BUDGET: "" };
  const previous = Object.fromEntries(Object.keys(variables).map(key => [key, process.env[key]]));
  Object.assign(process.env, variables);
  t.after(() => { for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value; rmSync(root, { recursive: true, force: true }); });
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify(policy));
  // The fixture runs the actual extension again inside each coordinator process.
  writeFileSync(cli, `import subagents from ${JSON.stringify(new URL("../extensions/taucode-subagents.ts", import.meta.url).href)};
    const args = process.argv.slice(2), role = process.env.TAUCODE_SUBAGENT_ROLE;
    if (role === "colonel") {
      let tool; subagents({ on() {}, registerTool(t) { tool = t; }, getThinkingLevel() { return args[args.indexOf("--thinking")+1]; } });
      const result = await tool.execute("nested", { tasks: [1,2].map(n => ({agent:"enlisted",task:"task "+n})) }, undefined, undefined, {cwd:process.cwd(),model:{provider:"taucode-cloud",id:"mid"}});
      console.log(JSON.stringify({type:"tool_execution_end",toolName:"subagent",toolCallId:"nested",result}));
    }
    console.log(JSON.stringify({type:"message_end",message:{role:"assistant",content:[{type:"text",text:JSON.stringify({role,provider:args[args.indexOf("--provider")+1],model:args[args.indexOf("--model")+1],thinking:args[args.indexOf("--thinking")+1]})}],stopReason:"stop"}}));`);
  let tool: any;
  let thinking = "low";
  const events: Record<string, (...args: any[]) => any> = {};
  subagents({ on: (name: string, fn: any) => { events[name] = fn; }, registerTool: (value: any) => { tool = value; }, getThinkingLevel: () => thinking } as any);
  const ctx = { cwd, model: { provider: "taucode-cloud", id: "strong" } };
  events.agent_start();
  const prompt = events.before_agent_start().message.content;
  assert.match(prompt, /commander/); assert.doesNotMatch(prompt, /"custom"/); assert.doesNotMatch(prompt, /taucode-local/);
  const result = await tool.execute("parallel", { tasks: [1,2].map(n => ({ agent: "colonel", task: "batch " + n })) }, undefined, undefined, ctx);
  const children = result.details.results.flatMap((r: any) => r.messages.filter((m: any) => m.toolName === "subagent").flatMap((m: any) => m.details.results));
  assert.equal(children.length, 4);
  assert.equal(children.filter((r: any) => r.status === "completed").length, 3);
  assert.match(children.find((r: any) => r.status === "failed").stderr, /across the command tree/);
  for (const child of children.filter((r: any) => r.status === "completed")) {
    assert.deepEqual(JSON.parse(child.messages[0].content[0].text), { role: "enlisted", provider: "taucode-local", model: "small", thinking: "low" });
    assert.ok(child.startedAt > 0 && child.endedAt >= child.startedAt, "persist actual child timing");
  }
  assert.match((await tool.execute("qa", { agent: "major", task: "review" }, undefined, undefined, ctx)).details.results[0].stderr, /across the command tree/);
  events.agent_end();
  assert.deepEqual(readdirSync(join(home, "taucode")), ["policy.json"]);
  thinking = "off";
  events.agent_start();
  const qa = await tool.execute("qa", { agent: "major", task: "review", model: "cloud/strong", cwd, tasks: [], chain: [] }, undefined, undefined, ctx);
  assert.equal(qa.isError, false);
  assert.deepEqual(JSON.parse(qa.details.results[0].messages[0].content[0].text), { role: "major", provider: "taucode-cloud", model: "strong", thinking: "off" });
  const nullable = await tool.execute("nullable", { agent: null, task: null, model: null, cwd: null, tasks: [{ agent: "major", task: "review", model: null, cwd: null }], chain: [] }, undefined, undefined, ctx);
  assert.equal(nullable.isError, false);
  assert.equal(nullable.details.mode, "parallel");
  for (const input of [{ chain: [] }, { agent: "major", task: "" }, { tasks: [{ agent: "major", task: "review" }], chain: [{ agent: "major", task: "review" }] }]) await assert.rejects(tool.execute("invalid", input, undefined, undefined, ctx));
  events.agent_end();
  process.env.TAUCODE_SUBAGENT_DEPTH = "1";
  process.env.TAUCODE_SUBAGENT_ROLE = "colonel";
  assert.match((await tool.execute("no-budget", { agent: "enlisted", task: "work" }, undefined, undefined, ctx)).details.results[0].stderr, /Missing managed/);
  process.env.TAUCODE_SUBAGENT_DEPTH = "0";
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify({ ...policy, enabled: false }));
  assert.match((await tool.execute("single", { agent: "enlisted", task: "work" }, undefined, undefined, ctx)).details.results[0].stderr, /disabled/);
  assert.deepEqual(readdirSync(join(home, "taucode")), ["policy.json"]);
});

test("OMP virtual delegation forwards child progress and replaces it with the final message", async t => {
  const root = mkdtempSync(join(tmpdir(), "taucode-virtual-progress-")), home = join(root, "agent"), cli = join(root, "fake-cli.mjs");
  mkdirSync(join(home, "taucode"), { recursive: true });
  const variables = { PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: root, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: "0" };
  const previous = Object.fromEntries(Object.keys(variables).map(key => [key, process.env[key]]));
  Object.assign(process.env, variables);
  t.after(() => { for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value; rmSync(root, { recursive: true, force: true }); });
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify(policy));
  writeFileSync(cli, `const transport=process.argv.at(-1).slice(6);
    const child={agent:"enlisted",status:"running",messages:[],usage:{input:5}};
    const result={content:[],details:{xdev:{tool:"subagent",mode:"execute",inner:{results:[child]}}}};
    console.log(JSON.stringify({type:"tool_execution_update",toolName:transport,toolCallId:"nested",partialResult:result}));
    child.status="completed";
    console.log(JSON.stringify({type:"tool_execution_end",toolName:transport,toolCallId:"nested",result}));
    console.log(JSON.stringify({type:"message_end",message:{...result,role:"toolResult",toolName:transport,toolCallId:"nested"}}));
    console.log(JSON.stringify({type:"message_end",message:{role:"assistant",content:[{type:"text",text:"Done"}],stopReason:"stop"}}));`);
  let tool: any;
  subagents({ on() {}, registerTool(value: any) { tool = value; } } as any);
  for (const transport of ["write", "edit"]) {
    const updates: any[] = [];
    const result = await tool.execute("virtual", { agent: "colonel", task: transport }, undefined, (r: any) => updates.push(r), { cwd: root, model: { provider: "taucode-cloud", id: "strong" } });
    assert.equal(result.isError, false);
    assert.ok(updates.some(u => u.details.results[0].messages.some((m: any) => m.details?.xdev?.inner?.results?.[0]?.status === "running")), "nested progress must reach the host before final completion");
    const nested = result.details.results[0].messages.filter((m: any) => m.toolCallId === "nested");
    assert.equal(nested.length, 1, "final message replaces the progress snapshot");
    assert.equal(nested[0].details.xdev.inner.results[0].status, "completed");
    assert.equal(nested[0].details.xdev.inner.results[0].usage.input, 5);
  }
});

test("shared concurrency bounds the whole tree, lends coordinator slots and restores them after overlapping calls", { timeout: 15000 }, async t => {
  const root = mkdtempSync(join(tmpdir(), "taucode-concurrency-"));
  const home = join(root, "agent"), cli = join(root, "fake-cli.mjs"), trace = join(root, "trace.jsonl"), aborted = join(root, "aborted");
  mkdirSync(join(home, "taucode"), { recursive: true });
  const variables = { PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: root, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: "0", TAUCODE_SUBAGENT_ROLE: "", TAUCODE_SUBAGENT_BUDGET: "", TAUCODE_SUBAGENT_SLOT: "" };
  const previous = Object.fromEntries(Object.keys(variables).map(key => [key, process.env[key]]));
  Object.assign(process.env, variables);
  const events: Record<string, () => void> = {};
  const controller = new AbortController();
  t.after(() => { controller.abort(); events.agent_end?.(); for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value; rmSync(root, { recursive: true, force: true }); });
  writeFileSync(cli, `import { appendFileSync, rmSync, writeFileSync } from "node:fs"; import { join } from "node:path";
    import { setTimeout as delay } from "node:timers/promises";
    import subagents from ${JSON.stringify(new URL("../extensions/taucode-subagents.ts", import.meta.url).href)};
    let tool; subagents({on() {},registerTool(t) {tool=t;}});
    const record = delta => appendFileSync(${JSON.stringify(trace)},JSON.stringify({pid:process.pid,delta})+"\\n");
    if (process.argv.at(-1).includes("fail")) process.exit(2);
    record(1); await delay(30);
    if (process.env.TAUCODE_SUBAGENT_ROLE === "colonel") {
      record(-1);
      const results = await Promise.all([1,2].map(n => tool.execute("nested-"+n,{tasks:[1,2].map(i=>({agent:"enlisted",task:process.argv.at(-1).includes("lose budget")?"lose budget":"work "+n+"-"+i}))},undefined,undefined,{cwd:process.cwd(),model:{provider:"taucode-cloud",id:"mid"},abort(){writeFileSync(${JSON.stringify(aborted)},"aborted");}})));
      if (results.some(r=>r.isError)) {console.error(JSON.stringify(results));process.exit(1);}
      record(1);
    }
    if (process.argv.at(-1).includes("lose budget")) rmSync(join(process.env.TAUCODE_SUBAGENT_BUDGET,"concurrency"),{force:true});
    await delay(60); record(-1);
    console.log(JSON.stringify({type:"message_end",message:{role:"assistant",content:[{type:"text",text:"Done"}],stopReason:"stop"}}));`);
  let tool: any;
  subagents({ on: (name: string, fn: any) => { events[name] = fn; }, registerTool: (value: any) => { tool = value; } } as any);
  const ctx = { cwd: root, model: { provider: "taucode-cloud", id: "strong" } };
  for (const maxConcurrent of [2, 1]) {
    writeFileSync(join(home, "taucode/policy.json"), JSON.stringify({ ...policy, maxConcurrent, maxPerTurn: 16 }));
    writeFileSync(trace, ""); events.agent_start();
    const result = await tool.execute("tree", { tasks: [1, 2].map(n => ({ agent: "colonel", task: "batch " + n })) }, controller.signal, undefined, ctx);
    assert.equal(result.isError, false, JSON.stringify(result));
    let active = 0, peak = 0;
    for (const line of readFileSync(trace, "utf8").trim().split("\n")) {
      active += JSON.parse(line).delta; peak = Math.max(peak, active);
      assert.ok(active >= 0 && active <= maxConcurrent, `active=${active}, limit=${maxConcurrent}`);
    }
    assert.equal(active, 0); assert.equal(peak, maxConcurrent);
    assert.equal((await tool.execute("failure", { agent: "major", task: "fail" }, controller.signal, undefined, ctx)).isError, true);
    assert.equal((await tool.execute("recovery", { agent: "major", task: "review" }, controller.signal, undefined, ctx)).isError, false);
    const budget = readdirSync(join(home, "taucode")).find(name => name.startsWith("subagent-turn-"))!;
    assert.equal(readdirSync(join(home, "taucode", budget)).filter(name => /^slot-\d+-free$/.test(name)).length, maxConcurrent);
    events.agent_end();
    assert.deepEqual(readdirSync(join(home, "taucode")), ["policy.json"]);
  }
  events.agent_start();
  assert.equal((await tool.execute("lost-budget", { agent: "colonel", task: "lose budget" }, controller.signal, undefined, ctx)).isError, true);
  assert.ok(existsSync(aborted), "a coordinator aborts its model operation if it cannot reacquire a slot");
});

test("shared slots cancel queued claims and stale releases cannot free a replacement lease", async t => {
  const home = mkdtempSync(join(tmpdir(), "taucode-slot-"));
  mkdirSync(join(home, "taucode"));
  t.after(() => rmSync(home, { recursive: true, force: true }));
  const budget = createSubagentBudget(home, 1);
  const first = await acquireSubagentSlot(budget);
  const controller = new AbortController();
  const waiting = acquireSubagentSlot(budget, controller.signal);
  controller.abort();
  await assert.rejects(waiting, /abort/i);
  releaseSubagentSlot(first);
  const second = await acquireSubagentSlot(budget);
  releaseSubagentSlot(first);
  assert.ok(existsSync(second));
  assert.ok(!existsSync(join(budget, "slot-0-free")));
  releaseSubagentSlot(second);
  assert.ok(existsSync(join(budget, "slot-0-free")));
  const abandoned = await acquireSubagentSlot(budget);
  execFileSync(process.execPath, ["--input-type=module", "-e", `import { adoptSubagentSlot } from ${JSON.stringify(new URL("../extensions/lib/policy.ts", import.meta.url).href)}; adoptSubagentSlot(${JSON.stringify(abandoned)});`]);
  const recovered = await acquireSubagentSlot(budget);
  releaseSubagentSlot(recovered);
  assert.deepEqual(readdirSync(budget), ["concurrency", "slot-0-free"]);
});

test("specialists use their configured models and rank without adding delegation privileges", () => {
  const specialists: Policy = { ...policy, roles: [...policy.roles,
    { name: "researcher", description: "Research", commandRank: "enlisted", model: "cloud/mid", toolAccess: "read-only", instructions: "Cite files" },
    { name: "test-engineer", description: "Test", commandRank: "major", model: "cloud/strong", toolAccess: "full", instructions: "Verify" },
    { name: "lead", description: "Coordinate", commandRank: "colonel", toolAccess: "full", instructions: "Delegate" },
    { name: "commander", description: "Invalid", commandRank: "colonel", toolAccess: "full", instructions: "" },
  ] };
  assert.deepEqual(commandTargets("commander", specialists.roles), ["colonel", "major", "enlisted", "researcher", "test-engineer", "lead"]);
  assert.deepEqual(commandTargets("lead", specialists.roles), ["major", "enlisted", "researcher", "test-engineer"]);
  for (const parent of ["commander", "colonel", "lead"]) {
    const resolved = resolveTask(specialists, { agent: "researcher" }, undefined, parent);
    assert.equal(resolved.model, "cloud/mid");
    assert.equal(resolved.role.toolAccess, "read-only");
    assert.throws(() => resolveTask(specialists, { agent: "researcher", model: "local/small" }, undefined, parent), /configured researcher model/);
  }
  for (const parent of ["researcher", "test-engineer", "custom"]) {
    assert.deepEqual(commandTargets(parent, specialists.roles), []);
    assert.throws(() => resolveTask(specialists, { agent: "enlisted" }, undefined, parent), /cannot delegate/);
  }
  assert.match(commandInstructions("test-engineer", specialists.roles), /test-engineer, with major rank/);
  assert.match(commandInstructions("researcher", specialists.roles), /cannot delegate/);
  assert.throws(() => resolveTask({ ...specialists, allowedModels: ["local/small"] }, { agent: "researcher" }), /not allowed/);
  assert.throws(() => resolveTask({ ...specialists, models: {} }, { agent: "researcher" }), /not enabled/);
});

test("a preset reaches child processes with its instructions, read-only tools and shared budget", async t => {
  const root = mkdtempSync(join(tmpdir(), "taucode-specialist-"));
  const home = join(root, "agent"), cli = join(root, "fake-cli.mjs");
  mkdirSync(join(home, "taucode"), { recursive: true });
  const variables = { PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: root, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: "0", TAUCODE_SUBAGENT_ROLE: "", TAUCODE_SUBAGENT_BUDGET: "", TAUCODE_TOOL_ACCESS: "full", TAUCODE_OMP: "1" };
  const previous = Object.fromEntries(Object.keys(variables).map(key => [key, process.env[key]]));
  Object.assign(process.env, variables);
  const events: Record<string, (...args: any[]) => any> = {};
  t.after(() => { events.agent_end?.(); for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value; rmSync(root, { recursive: true, force: true }); });
  const researcher = { name: "researcher", description: "Research", commandRank: "enlisted", model: "cloud/mid", toolAccess: "read-only", instructions: "Cite exact files" };
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify({ ...policy, maxPerTurn: 1, roles: [...policy.roles, researcher] }));
  writeFileSync(cli, `import subagents from ${JSON.stringify(new URL("../extensions/taucode-subagents.ts", import.meta.url).href)};
    const args = process.argv.slice(2), events = {}; subagents({on(name, fn) {events[name] = fn;},registerTool() {}});
    console.log(JSON.stringify({type:"message_end",message:{role:"assistant",content:[{type:"text",text:JSON.stringify({args,role:process.env.TAUCODE_SUBAGENT_ROLE,access:process.env.TAUCODE_TOOL_ACCESS,prompt:events.before_agent_start().message.content})}],stopReason:"stop"}}));`);
  let tool: any;
  subagents({ on: (name: string, fn: any) => { events[name] = fn; }, registerTool: (value: any) => { tool = value; } } as any);
  events.agent_start();
  assert.match(events.before_agent_start().message.content, /"name":"researcher"/);
  const ctx = { cwd: root, model: { provider: "taucode-cloud", id: "strong" } };
  const result = await tool.execute("research", { agent: "researcher", task: "Inspect code" }, undefined, undefined, ctx);
  assert.equal(result.isError, false);
  const child = JSON.parse(result.details.results[0].messages[0].content[0].text);
  assert.equal(child.role, "researcher"); assert.equal(child.access, "read-only");
  assert.equal(child.args[child.args.indexOf("--tools") + 1], "read,grep,glob");
  assert.equal(child.args[child.args.indexOf("--model") + 1], "mid");
  assert.equal(child.args[child.args.indexOf("--append-system-prompt") + 1], researcher.instructions);
  assert.match(child.prompt, /researcher, with enlisted rank/);
  assert.match(child.prompt, /"roles":\[\]/);
  assert.match((await tool.execute("extra", { agent: "researcher", task: "Again" }, undefined, undefined, ctx)).details.results[0].stderr, /max per turn/);
});

for (const { detached, depth, nested = false } of [{ detached: false, depth: 0 }, { detached: true, depth: 0 }, { detached: true, depth: 1 }, { detached: true, depth: 0, nested: true }]) test(`abort at depth ${depth}${nested ? " with nested delegation" : ""} kills a ${detached ? "detached" : "grouped"} stubborn descendant without killing its parent or starting queued work`, { skip: process.platform === "win32", timeout: 10000 }, async t => {
  const root = mkdtempSync(join(tmpdir(), "taucode-command-abort-"));
  const home = join(root, "agent"), cwd = join(root, "workspace"), cli = join(root, "fake-cli.mjs");
  const ready = join(root, "ready.json"), queued = join(root, "queued"), controller = new AbortController();
  mkdirSync(join(home, "taucode"), { recursive: true }); mkdirSync(cwd);
  const budget = createSubagentBudget(home, 1);
  const variables = { PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: cwd, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: String(depth), TAUCODE_SUBAGENT_ROLE: depth ? "colonel" : "", TAUCODE_SUBAGENT_BUDGET: budget };
  const previous = Object.fromEntries(Object.keys(variables).map(key => [key, process.env[key]]));
  Object.assign(process.env, variables);
  let pids: number[] = [];
  let tool: any;
  const saved: any[] = [];
  const events: Record<string, () => void> = {};
  t.after(() => {
    controller.abort();
    for (const pid of pids) { try { process.kill(pid, "SIGKILL"); } catch { /* already gone */ } }
    events.agent_end?.();
    for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value;
    rmSync(root, { recursive: true, force: true });
  });
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify({ ...policy, maxConcurrent: 1 }));
  const stubborn = `import { writeFileSync } from 'node:fs';
    process.on('SIGTERM', () => {});
    writeFileSync(${JSON.stringify(ready)}, JSON.stringify([process.ppid, process.pid]));
    setInterval(() => {}, 1000);`;
  writeFileSync(cli, `import { spawn } from 'node:child_process'; import { writeFileSync } from 'node:fs';
    import subagents from ${JSON.stringify(new URL("../extensions/taucode-subagents.ts", import.meta.url).href)};
    let tool; if (${nested}) subagents({on() {},registerTool(t) {tool=t;}});
    if (${nested} && process.env.TAUCODE_SUBAGENT_ROLE === 'colonel') {
      process.on('SIGTERM', () => process.exit(0));
      await tool.execute('nested',{agent:'enlisted',task:'wait for stop'},undefined,undefined,{cwd:process.cwd(),model:{provider:'taucode-cloud',id:'mid'}});
    } else if (process.argv.at(-1).includes('wait for stop')) {
      process.on('SIGTERM', () => process.exit(0));
      spawn(process.execPath, ['--input-type=module', '-e', ${JSON.stringify(stubborn)}], {stdio:'ignore',detached:${detached}});
      setInterval(() => {}, 1000);
    } else writeFileSync(${JSON.stringify(queued)}, 'started');`);
  subagents({ on: (name: string, fn: any) => { events[name] = fn; }, registerTool: (value: any) => { tool = value; }, appendEntry: (customType: string, data: any) => saved.push({ customType, data: structuredClone(data) }) } as any);
  events.agent_start();
  const sessionFile = join(root, "session.jsonl");
  const ctx = { cwd, model: { provider: "taucode-cloud", id: "strong" }, sessionManager: { getSessionFile: () => sessionFile } };
  const running = tool.execute("cancel-tree", { tasks: [{ agent: depth ? "enlisted" : "colonel", task: "wait for stop" }, { agent: "major", task: "queued" }] }, controller.signal, undefined, ctx);
  for (let i = 0; i < 150 && !existsSync(ready); i++) await delay(20);
  assert.ok(existsSync(ready), "descendant started and installed its SIGTERM handler");
  pids = JSON.parse(readFileSync(ready, "utf8"));
  assert.ok(!existsSync(queued));
  assert.equal(existsSync(`${sessionFile}.agents`), depth === 0, "only the root writes crash recovery evidence");
  controller.abort();
  const result = await running;
  assert.equal(result.isError, true);
  assert.ok(result.details.results.every((r: any) => r.status === "failed" && r.stopReason === "aborted"));
  assert.ok(result.details.results[0].startedAt > 0);
  assert.ok(result.details.results[0].endedAt >= result.details.results[0].startedAt);
  assert.equal(result.details.results[1].startedAt, undefined, "queued work has no invented start time");
  assert.equal(saved.length, depth === 0 ? 1 : 0, "persist one root cancellation result outside model context");
  if (depth === 0) {
    assert.equal(saved[0].customType, "taucode-subagent-result");
    assert.deepEqual(saved[0].data, { toolCallId: "cancel-tree", result });
  }
  assert.ok(!existsSync(queued), "queued work must not start after cancellation");
  // An orphan can briefly remain a zombie until the OS reaps it; it cannot run.
  const alive = (pid: number) => {
    try { const state = execFileSync("ps", ["-o", "stat=", "-p", String(pid)], { encoding: "utf8" }).trim(); return !!state && !state.startsWith("Z"); }
    catch { return false; }
  };
  for (let i = 0; i < 100 && pids.some(alive); i++) await delay(20);
  assert.ok(pids.every(pid => !alive(pid)), "the stopped command tree has no running descendants");
  for (const name of readdirSync(join(home, "taucode")).filter(name => name.startsWith("subagent-turn-"))) {
    assert.equal(readdirSync(join(home, "taucode", name)).filter(name => /^slot-\d+-free$/.test(name)).length, 1, "cancelled descendants release their adopted slots");
  }
  events.agent_end(); events.agent_start();
  if (depth === 0) assert.equal(readdirSync(`${sessionFile}.agents`).length, 0, "durable completion clears temporary recovery snapshots");
  assert.equal((await tool.execute("recovery", { agent: "major", task: "next turn" }, undefined, undefined, ctx)).isError, false);
  assert.ok(existsSync(queued), "a new turn can use the released concurrency slot");
});

for (const runner of [process.execPath, new URL("../../omp/node_modules/.bin/bun", import.meta.url).pathname]) test(`a killed commander preserves progress and its orphaned coordinator stops nested commands (${runner})`, { skip: process.platform === "win32" || !existsSync(runner), timeout: 10000 }, async t => {
  const root = mkdtempSync(join(tmpdir(), "taucode-command-crash-"));
  const home = join(root, "agent"), cli = join(root, "child.mjs"), ready = join(root, "ready.json"), session = join(root, "session.jsonl");
  mkdirSync(join(home, "taucode"), { recursive: true });
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify(policy));
  const extension = JSON.stringify(new URL("../extensions/taucode-subagents.ts", import.meta.url).href);
  writeFileSync(cli, `import subagents from ${extension}; import {spawn} from 'node:child_process'; import {writeFileSync} from 'node:fs';
    const controller = new AbortController(), events = {}; let tool;
    subagents({on(n,f){events[n]=f;},registerTool(t){tool=t;}});
    const ctx={cwd:process.cwd(),model:{provider:'taucode-cloud',id:'mid'},ui:{setStatus(){}},abort(){controller.abort();}};
    events.session_start({},ctx); events.agent_start();
    if(process.env.TAUCODE_SUBAGENT_ROLE==='colonel') {
      const result=await tool.execute('nested',{agent:'enlisted',task:'wait'},controller.signal,r=>console.log(JSON.stringify({type:'tool_execution_update',toolCallId:'nested',toolName:'subagent',partialResult:r})),ctx);
      events.agent_end(); events.session_shutdown();
    } else {
      const child=spawn(process.execPath,['--input-type=module','-e',"process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"],{detached:true,stdio:'ignore'});
      writeFileSync(${JSON.stringify(ready)},JSON.stringify([process.ppid,process.pid,child.pid]));
      console.log(JSON.stringify({type:'message_end',message:{role:'assistant',content:[{type:'text',text:'CRASH_PARTIAL_EVIDENCE'}],stopReason:'toolUse'}}));
      process.on('SIGTERM',()=>{}); setInterval(()=>{},1000);
    }`);
  const driver = join(root, "driver.mjs");
  writeFileSync(driver, `import subagents from ${extension}; const events={};let tool;
    subagents({on(n,f){events[n]=f;},registerTool(t){tool=t;},appendEntry(){}});
    events.agent_start(); await tool.execute('root-call',{agent:'colonel',task:'delegate'},undefined,undefined,{cwd:process.cwd(),model:{provider:'taucode-cloud',id:'strong'},sessionManager:{getSessionFile:()=>${JSON.stringify(session)}}});`);
  const { spawn } = await import("node:child_process");
  const parent = spawn(runner, [driver], { cwd: root, env: { ...process.env, PI_CODING_AGENT_DIR: home, TAUCODE_WORKSPACE: root, TAUCODE_PI_CLI: cli, TAUCODE_SUBAGENT_DEPTH: "0", TAUCODE_SUBAGENT_ROLE: "", TAUCODE_SUBAGENT_BUDGET: "", TAUCODE_SUBAGENT_SLOT: "" }, stdio: "ignore" });
  let pids: number[] = [];
  t.after(() => { parent.kill("SIGKILL"); for (const pid of pids) { try { process.kill(pid, "SIGKILL"); } catch {} } rmSync(root, {recursive:true,force:true}); });
  const folder = `${session}.agents`;
  let saved = "";
  for (let i = 0; i < 200; i++) {
    if (existsSync(ready)) pids = JSON.parse(readFileSync(ready, "utf8"));
    if (existsSync(folder)) { const name = readdirSync(folder).find(n => n.endsWith(".json")); if (name) saved = readFileSync(join(folder, name), "utf8"); }
    if (pids.length && saved.includes("CRASH_PARTIAL_EVIDENCE")) break;
    await delay(20);
  }
  assert.match(saved, /CRASH_PARTIAL_EVIDENCE/);
  parent.kill("SIGKILL");
  const alive = (pid: number) => { try { const state = execFileSync("ps", ["-o", "stat=", "-p", String(pid)], {encoding:"utf8"}).trim(); return !!state && !state.startsWith("Z"); } catch { return false; } };
  for (let i = 0; i < 200 && pids.some(alive); i++) await delay(20);
  assert.ok(pids.every(pid => !alive(pid)), "coordinator, worker and detached command must stop after losing their parent");
  assert.match(readFileSync(join(folder, readdirSync(folder)[0]), "utf8"), /CRASH_PARTIAL_EVIDENCE/);
});

test("conversation snapshots isolate team models from other conversations and project edits", (t) => {
  const home = mkdtempSync(join(tmpdir(), "taucode-policy-"));
  const previousHome = process.env.PI_CODING_AGENT_DIR, previousId = process.env.TAUCODE_SESSION_KEY;
  t.after(() => {
    if (previousHome === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = previousHome;
    if (previousId === undefined) delete process.env.TAUCODE_SESSION_KEY; else process.env.TAUCODE_SESSION_KEY = previousId;
    rmSync(home, { recursive: true, force: true });
  });
  process.env.PI_CODING_AGENT_DIR = home;
  mkdirSync(join(home, "taucode/policies"), { recursive: true });
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify(policy));
  writeFileSync(join(home, "taucode/policies/one.json"), JSON.stringify({ ...policy, defaultModel: "cloud/mid" }));
  writeFileSync(join(home, "taucode/policies/two.json"), JSON.stringify({ ...policy, defaultModel: "local/small" }));
  for (const [id, model] of [["one", "cloud/mid"], ["two", "local/small"]]) {
    process.env.TAUCODE_SESSION_KEY = id;
    assert.equal(readPolicy().defaultModel, model);
    writeFileSync(join(home, "taucode/policy.json"), JSON.stringify({ ...policy, defaultModel: "cloud/strong" }));
    assert.equal(readPolicy().defaultModel, model);
  }
  delete process.env.TAUCODE_SESSION_KEY;
  assert.equal(readPolicy().defaultModel, "cloud/strong");
  process.env.TAUCODE_SESSION_KEY = "../escape";
  assert.throws(readPolicy, /Invalid conversation ID/);
  process.env.TAUCODE_SESSION_KEY = "one";
  writeFileSync(join(home, "taucode/policies/one.json"), "broken");
  assert.throws(readPolicy, /JSON/);
});
