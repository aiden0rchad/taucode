import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { registerExecutionLimits, UsageLedger } from "../extensions/lib/executionLimits.ts";
import type { Policy } from "../extensions/lib/policy.ts";

const usage = { input: 2, output: 3, cacheRead: 4, cacheWrite: 1 };
const policy: Policy = { mode: "chain-of-command", enabled: true, roles: [], allowedModels: [], maxConcurrent: 2, maxPerTurn: 8, maxDepth: 2 };
function temporary(t: any) {
  const path = mkdtempSync(join(tmpdir(), "taucode-limits-"));
  t.after(() => rmSync(path, { recursive: true, force: true }));
  return join(path, "usage.sqlite");
}

test("usage survives reopening, deduplicates responses, preserves unknown usage and resets by UTC day", t => {
  const file = temporary(t), yesterday = Date.now() - 86400_000;
  let db = new UsageLedger(file);
  db.record("yesterday", usage, yesterday);
  db.record("today", usage); db.record("today", usage);
  db.record("unknown", undefined);
  assert.deepEqual(db.totals(), { tokens: 10, unknown: 1 });
  db.close(); db = new UsageLedger(file);
  assert.deepEqual(db.totals(), { tokens: 10, unknown: 1 });
  db.record("unknown", usage);
  assert.deepEqual(db.totals(), { tokens: 20, unknown: 0 });
  db.record("unknown", undefined);
  assert.deepEqual(db.totals(), { tokens: 20, unknown: 0 });
  assert.deepEqual(db.totals(yesterday), { tokens: 10, unknown: 0 });
  db.close();
});

test("concurrent writers retain every response without counting duplicate frames twice", async t => {
  const file = temporary(t);
  // Create once, then exercise actual independent SQLite connections/processes.
  new UsageLedger(file).close();
  await Promise.all(Array.from({ length: 8 }, async (_, index) => {
    const child = spawn(process.execPath, ["--input-type=module", "-e", `import { UsageLedger } from ${JSON.stringify(new URL("../extensions/lib/executionLimits.ts", import.meta.url).href)}; const db=new UsageLedger(${JSON.stringify(file)}); for(let n=0;n<20;n++) { db.record(${JSON.stringify(String(index))}+':'+n,{input:2,output:3}); db.record(${JSON.stringify(String(index))}+':'+n,{input:2,output:3}); } db.close();`], { stdio: ["ignore", "ignore", "pipe"] });
    let error = ""; child.stderr.on("data", data => error += data);
    const [code] = await once(child, "exit"); assert.equal(code, 0, error);
  }));
  const db = new UsageLedger(file);
  assert.deepEqual(db.totals(), { tokens: 800, unknown: 0 }); db.close();
});

function harness(t: any, patch: Partial<Policy> = {}, child = false) {
  const file = temporary(t), settings = { ...policy, ...patch };
  const env = { TAUCODE_COMMAND_USAGE_DB: file, TAUCODE_SUBAGENT_DEPTH: child ? "1" : "0", TAUCODE_CHAT: "", TAUCODE_WORK: "", TAUCODE_RUN_DEADLINE: "0" };
  const old = Object.fromEntries(Object.keys(env).map(key => [key, process.env[key]])); Object.assign(process.env, env);
  const hooks = new Map<string, any[]>(), records: any[] = [], live: any[] = [];
  let aborts = 0;
  const ctx = { abort: () => aborts++, sessionManager: { getSessionId: () => "session" }, ui: { setStatus: (_key: string, record: string) => live.push(JSON.parse(record)) } };
  registerExecutionLimits({ on: (name: string, fn: any) => hooks.set(name, [...hooks.get(name) ?? [], fn]), appendEntry: (_type: string, record: any) => records.push(record) } as never, () => settings);
  const emit = async (name: string, value: any = {}) => { let result; for (const fn of hooks.get(name) ?? []) result = await fn(value, ctx); return result; };
  t.after(async () => { await emit("session_shutdown"); for (const [key, value] of Object.entries(old)) if (value === undefined) delete process.env[key]; else process.env[key] = value; });
  return { file, settings, records, live, emit, aborts: () => aborts };
}

test("root budget stops tools, persists a readable reason, and remains exhausted on the next run", async t => {
  const h = harness(t, { dailyTokenLimit: 10 });
  await h.emit("agent_start"); assert.equal(h.aborts(), 0);
  await h.emit("message_end", { message: { role: "assistant", timestamp: Date.now(), usage, stopReason: "toolUse" } });
  assert.equal(h.aborts(), 1);
  assert.equal((await h.emit("tool_call")).block, true);
  await h.emit("agent_end");
  assert.match(h.records[0].message, /daily token allowance/); assert.deepEqual(h.live, h.records);
  assert.equal(h.records[0].beforeUser, false);
  await h.emit("agent_start"); assert.equal(h.aborts(), 2);
  assert.equal(h.records[1].beforeUser, true);
  await h.emit("agent_end");
  h.settings.mode = "custom"; await h.emit("agent_start"); await h.emit("before_provider_request");
  assert.equal(h.aborts(), 2, "custom delegation is unchanged");
});

test("children inherit deadlines; unavailable accounting blocks capped runs without changing Chat or Work", async t => {
  const h = harness(t, { dailyTokenLimit: 10 }, true);
  process.env.TAUCODE_RUN_DEADLINE = String(Date.now() - 1);
  await h.emit("agent_start"); assert.equal(h.aborts(), 1);
  await h.emit("agent_end"); assert.equal(h.records.length, 0, "only the commander publishes the stop reason");
  process.env.TAUCODE_RUN_DEADLINE = "0";
  await h.emit("agent_start");
  await h.emit("message_end", { message: { role: "assistant", timestamp: Date.now(), stopReason: "stop" } });
  assert.equal(h.aborts(), 2);
  await h.emit("agent_end");
  process.env.TAUCODE_WORK = "1"; await h.emit("agent_start"); await h.emit("before_provider_request");
  assert.equal(h.aborts(), 2);
});
