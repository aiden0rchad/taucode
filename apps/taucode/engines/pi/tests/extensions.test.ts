import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, symlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { decide, normalizeRule, protectedPath, readOnlyCommand, resolveTask, checkLimits, type Policy } from "../extensions/lib/policy.ts";
import gate from "../extensions/taucode-gate.ts";
import subagents from "../extensions/taucode-subagents.ts";

const policy: Policy = { maxConcurrent: 2, maxPerTurn: 3, maxDepth: 2, allowedModels: ["local/small", "local/large"], defaultModel: "local/small", roles: [{ name: "worker", description: "Work", toolAccess: "full", instructions: "Keep it small" }, { name: "scout", description: "Read", toolAccess: "read-only", instructions: "", model: "local/large" }] };
const root = mkdtempSync(join(tmpdir(), "taucode-extensions-"));
const home = join(root, "agent");
const cwd = join(root, "workspace");
mkdirSync(join(home, "taucode/threads"), { recursive: true });
mkdirSync(cwd);
process.on("exit", () => rmSync(root, { recursive: true, force: true }));

test("gate decision table, exact session rules, shell injection and protected paths", () => {
  const bash = { tool: "bash", command: "npm test", cwd };
  const write = { tool: "write", path: "file.ts", cwd };
  for (const call of [bash, write]) {
    assert.equal(decide({ accessMode: "full-access", sessionAllow: [] }, call, home, true), "allow");
    assert.equal(decide({ accessMode: "read-only", sessionAllow: [] }, call, home, true), "block");
    assert.equal(decide({ accessMode: "ask", sessionAllow: [] }, call, home, true), "ask");
    assert.equal(decide({ accessMode: "ask", sessionAllow: [] }, call, home, false), "block");
    assert.equal(decide({ accessMode: "ask", sessionAllow: [normalizeRule(call)] }, call, home, true), "allow");
  }
  assert.deepEqual(normalizeRule({ ...write, path: "./a/../file.ts" }), normalizeRule(write));
  assert.equal(decide({ accessMode: "ask", sessionAllow: [normalizeRule(bash)] }, { ...bash, command: "npm test; rm x" }, home, true), "ask");
  for (const command of ["ls -la", "cat file.ts", "rg -n name src", "grep -n name file", "git status --short", "git diff --stat", "git log --oneline"]) assert.ok(readOnlyCommand(command), command);
  for (const command of ["ls; rm x", "cat $(touch x)", "rg --pre=sh x", "git log --output=file", "git diff --ext-diff", "git diff *", "rg *", "ls > file", "cat x\nrm x", "FOO=bar ls", "ls `id`", "cat <(id)"]) assert.ok(!readOnlyCommand(command), command);
  for (const path of [".env", ".env.local", ".git/config", home]) {
    assert.ok(protectedPath(join(cwd, path), home) || path === home);
    assert.equal(decide({ accessMode: "full-access", sessionAllow: [] }, { ...write, path }, home, true), "ask");
    assert.equal(decide({ accessMode: "full-access", sessionAllow: [] }, { ...write, path }, home, false), "block");
  }
  symlinkSync(home, join(cwd, "linked"));
  assert.equal(decide({ accessMode: "full-access", sessionAllow: [] }, { ...write, path: "linked/new-file" }, home, true), "ask");
  symlinkSync(join(home, "not-created"), join(cwd, "dangling"));
  assert.equal(decide({ accessMode: "full-access", sessionAllow: [] }, { ...write, path: "dangling" }, home, true), "ask");
  const child = { workspace: cwd, toolAccess: "full" };
  const ask = { accessMode: "ask" as const, sessionAllow: [normalizeRule(bash)] };
  assert.equal(decide(ask, write, home, false, child), "allow");
  assert.equal(decide(ask, { ...write, path: "../outside" }, home, false, child), "block");
  assert.equal(decide(ask, bash, home, false, child), "block");
  assert.equal(decide(ask, { ...bash, command: "ls" }, home, false, child), "allow");
  assert.equal(decide(ask, write, home, false, { ...child, toolAccess: "read-only" }), "block");
});

test("policy model precedence, allow-list, registry mapping and limit boundaries", () => {
  assert.equal(resolveTask(policy, { agent: "scout", model: "local/small" }).model, "local/small");
  assert.equal(resolveTask(policy, { agent: "scout" }).model, "local/large");
  assert.equal(resolveTask(policy, { agent: "worker" }).model, "local/small");
  assert.equal(resolveTask({ ...policy, defaultModel: undefined }, { agent: "worker" }, "local/large").model, "local/large");
  assert.throws(() => resolveTask(policy, { agent: "worker", model: "account/gpt" }), /Allowed models: local\/small, local\/large/);
  assert.throws(() => resolveTask(policy, { agent: "project-agent" }), /Unknown role/);
  assert.equal(resolveTask({ ...policy, models: { "local/small": "taucode-local/small" } }, { agent: "worker" }).provider, "taucode-local");
  assert.throws(() => resolveTask({ ...policy, models: {} }, { agent: "worker" }), /registry/);
  checkLimits(policy, 1, 2);
  assert.throws(() => checkLimits(policy, 2, 0), /depth/);
  assert.throws(() => checkLimits(policy, 0, 3), /per turn/);
  assert.throws(() => checkLimits({ ...policy, enabled: false }, 0, 0), /disabled/);
  assert.throws(() => checkLimits({ ...policy, maxConcurrent: 0 }, 0, 0), /maxConcurrent/);
});

test("gate approval marker, persistence, live changes and noninteractive children", async () => {
  process.env.PI_CODING_AGENT_DIR = home;
  process.env.TAUCODE_SESSION_KEY = "thread";
  delete process.env.TAUCODE_SUBAGENT_DEPTH;
  const file = join(home, "taucode/threads/thread.json");
  const write = (mode: string) => writeFileSync(file, JSON.stringify({ accessMode: mode, sessionAllow: [] }));
  let handler: any;
  gate({ on: (name: string, fn: any) => { if (name === "tool_call") handler = fn; } } as any);
  const event = { toolName: "write", toolCallId: "call", input: { path: "file.ts", content: "x" } };
  const ctx = { cwd, hasUI: true, ui: { select: async (title: string, options: string[]) => {
    assert.deepEqual(options, ["Allow", "Allow for session", "Block"]);
    assert.equal(JSON.parse(title.slice("taucode:approval:".length)).toolCallId, "call");
    return "Allow for session";
  } } };
  write("ask");
  assert.equal(await handler(event, ctx), undefined);
  assert.equal(JSON.parse(readFileSync(file, "utf8")).sessionAllow.length, 1);
  assert.equal(await handler(event, { ...ctx, ui: { select: () => assert.fail("already allowed") } }), undefined);
  write("read-only");
  assert.equal((await handler(event, ctx)).block, true);
  write("ask");
  assert.equal((await handler(event, { ...ctx, ui: { select: async () => { write("read-only"); return "Allow"; } } })).block, true);
  write("ask");
  process.env.TAUCODE_SUBAGENT_DEPTH = "1";
  process.env.TAUCODE_WORKSPACE = cwd;
  process.env.TAUCODE_TOOL_ACCESS = "full";
  assert.equal(await handler(event, { ...ctx, hasUI: false }), undefined);
  assert.equal((await handler({ ...event, input: { path: ".env" } }, { ...ctx, hasUI: false })).block, true);
  delete process.env.TAUCODE_SUBAGENT_DEPTH;
  delete process.env.TAUCODE_TOOL_ACCESS;
});

test("OMP virtual MCP writes use only the inner connector gate and retain denials", async (t) => {
  const keys = ["TAUCODE_OMP", "TAUCODE_CHAT", "PI_CODING_AGENT_DIR", "TAUCODE_SESSION_KEY", "TAUCODE_SUBAGENT_DEPTH", "TAUCODE_TOOL_ACCESS"];
  const previous = keys.map(key => process.env[key]);
  t.after(() => keys.forEach((key, i) => previous[i] === undefined ? delete process.env[key] : process.env[key] = previous[i]));
  process.env.TAUCODE_OMP = "1";
  delete process.env.TAUCODE_CHAT;
  process.env.PI_CODING_AGENT_DIR = home;
  process.env.TAUCODE_SESSION_KEY = "virtual";
  delete process.env.TAUCODE_SUBAGENT_DEPTH;
  const access = join(home, "taucode/threads/virtual.json");
  const setMode = (accessMode: string) => writeFileSync(access, JSON.stringify({ accessMode, sessionAllow: [] }));
  const connectors = [{ id: "taucode-browser", enabled: true, approval: "ask", toolDenylist: [] as string[] }];
  const setConnectors = () => writeFileSync(join(home, "taucode/connectors.json"), JSON.stringify(connectors));
  setMode("ask"); setConnectors();
  let handler: any, prompts = 0;
  gate({ on: (name: string, fn: any) => { if (name === "tool_call") handler = fn; } } as any);
  const ctx = { cwd, hasUI: true, ui: { select: async () => { prompts++; return "Allow"; } } };
  const outer = { toolName: "write", toolCallId: "virtual", input: { path: "xd://mcp__taucode_browser_browser_click", content: "{}" } };
  const inner = { toolName: "mcp__taucode_browser_browser_click", toolCallId: "virtual", input: {} };
  assert.equal(await handler(outer, ctx), undefined);
  assert.equal(prompts, 0);
  assert.equal(await handler(inner, ctx), undefined);
  assert.equal(prompts, 1);
  // Ordinary files, edits and other virtual devices still use their own gate.
  for (const event of [{ ...outer, input: { path: "file.ts" } }, { ...outer, toolName: "edit" }, { ...outer, input: { path: "xd://other_device" } }]) {
    assert.equal(await handler(event, ctx), undefined);
  }
  assert.equal(prompts, 4);
  setMode("read-only");
  assert.equal((await handler(inner, ctx)).block, true);
  setMode("ask");
  process.env.TAUCODE_SUBAGENT_DEPTH = "1";
  process.env.TAUCODE_TOOL_ACCESS = "read-only";
  assert.equal((await handler(inner, ctx)).block, true);
  delete process.env.TAUCODE_SUBAGENT_DEPTH;
  connectors[0].toolDenylist.push("browser_click"); setConnectors();
  assert.equal((await handler(inner, ctx)).block, true);
  assert.equal(prompts, 4);
});

test("subagents single, parallel, chain, concurrency, per-run limit, depth and abort without providers", async () => {
  process.env.PI_CODING_AGENT_DIR = home;
  process.env.TAUCODE_WORKSPACE = cwd;
  const cli = join(root, "fake-cli.mjs");
  writeFileSync(cli, `const args = process.argv.slice(2); if (args.at(-1).includes("nested")) {
    const child = {agent:"scout",task:"nested",status:"running",messages:[],usage:{input:1,output:2,cost:0.01}};
    console.log(JSON.stringify({type:"tool_execution_update",toolName:"subagent",toolCallId:"nested-call",partialResult:{details:{results:[child]}}}));
    child.status = "completed";
    const message = {role:"toolResult",toolName:"subagent",toolCallId:"nested-call",details:{results:[child]}};
    console.log(JSON.stringify({type:"message_end",message}));
    console.log(JSON.stringify({type:"tool_execution_end",toolName:"subagent",toolCallId:"nested-call",result:message}));
  } else if (args.at(-1).includes("hang")) { process.on("SIGTERM", () => {}); setInterval(() => {}, 1000); } else setTimeout(() => { console.log(JSON.stringify({ type: "message_end", message: { role: "assistant", content: [{type:"text",text:JSON.stringify({args,depth:process.env.TAUCODE_SUBAGENT_DEPTH,access:process.env.TAUCODE_TOOL_ACCESS})}], stopReason:"stop",usage:{input:2,output:3,cost:{total:0.01}} } })); }, 40);`);
  process.env.TAUCODE_PI_CLI = cli;
  writeFileSync(join(home, "taucode/policy.json"), JSON.stringify(policy));
  let tool: any, reset: any;
  subagents({ on: (name: string, fn: any) => { if (name === "agent_start") reset = fn; }, registerTool: (t: any) => { tool = t; }, appendEntry() {} } as any);
  const ctx = { cwd, model: { provider: "local", id: "small" } };
  const task = { agent: "scout", task: "read" };
  let peak = 0;
  const result = await tool.execute("call", { tasks: [task, task, task, task] }, undefined, (r: any) => { peak = Math.max(peak, r.details.results.filter((v: any) => v.status === "running").length); }, ctx);
  assert.equal(peak, 2);
  assert.deepEqual(result.details.results.map((r: any) => r.status), ["completed", "completed", "completed", "failed"]);
  assert.match(result.details.results[3].stderr, /per turn/);
  const child = JSON.parse(result.details.results[0].messages[0].content[0].text);
  assert.equal(child.depth, "1"); assert.equal(child.access, "read-only");
  assert.ok(child.args.includes("read,grep,find,ls")); assert.ok(child.args.includes("--no-session"));
  assert.equal(child.args[child.args.indexOf("--provider") + 1], "local");
  reset();
  const chain = await tool.execute("chain", { chain: [task, { ...task, task: "previous: {previous}" }] }, undefined, undefined, ctx);
  assert.match(chain.details.results[1].task, /previous:.*args/);
  reset();
  let sawRunningNested = false;
  const nested = await tool.execute("nested", { ...task, task: "nested" }, undefined, (r: any) => {
    sawRunningNested ||= r.details.results[0].messages.some((m: any) => m.details?.results?.[0]?.status === "running");
  }, ctx);
  assert.ok(sawRunningNested);
  assert.equal(nested.details.results[0].messages.length, 1);
  assert.equal(nested.details.results[0].messages[0].details.results[0].status, "completed");
  reset(); process.env.TAUCODE_SUBAGENT_DEPTH = "2";
  assert.match((await tool.execute("depth", task, undefined, undefined, ctx)).details.results[0].stderr, /depth/);
  delete process.env.TAUCODE_SUBAGENT_DEPTH;
  reset();
  const controller = new AbortController();
  const hanging = tool.execute("abort", { agent: "worker", task: "hang" }, controller.signal, undefined, ctx);
  setTimeout(() => controller.abort(), 100);
  assert.equal((await hanging).details.results[0].status, "failed");
  await assert.rejects(tool.execute("invalid", { ...task, agentScope: "project" }, undefined, undefined, ctx), /directories are disabled/);
});
