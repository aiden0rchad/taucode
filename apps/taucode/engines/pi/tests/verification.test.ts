import assert from "node:assert/strict";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { acceptanceCriteria, registerVerification, verificationProblem, verifyReport } from "../extensions/lib/verification.ts";
import type { Policy } from "../extensions/lib/policy.ts";

const policy: Policy = { enabled: true, mode: "chain-of-command", roles: [], maxConcurrent: 2, maxDepth: 2, maxPerTurn: 8, allowedModels: [] };
function fixture(t: any, role = "commander") {
  const cwd = mkdtempSync(join(tmpdir(), "taucode-qa-"));
  const env = { TAUCODE_SUBAGENT_DEPTH: role === "commander" ? "0" : "1", TAUCODE_SUBAGENT_ROLE: role, TAUCODE_VERIFY_CRITERIA: '["tests pass"]', TAUCODE_CHAT: "", TAUCODE_WORK: "" };
  const previous = Object.fromEntries(Object.keys(env).map(key => [key, process.env[key]]));
  Object.assign(process.env, env);
  t.after(() => { rmSync(cwd, { recursive: true, force: true }); for (const [key, value] of Object.entries(previous)) if (value === undefined) delete process.env[key]; else process.env[key] = value; });
  const hooks = new Map<string, any[]>(), tools = new Map<string, any>(), records: any[] = [], live: any[] = [];
  const ctx = { cwd, ui: { setStatus: (_key: string, value: string) => live.push(JSON.parse(value)) } };
  const current = structuredClone(policy);
  registerVerification({ on: (name: string, fn: any) => hooks.set(name, [...hooks.get(name) ?? [], fn]), registerTool: (tool: any) => tools.set(tool.name, tool), appendEntry: (_type: string, record: any) => records.push(record) } as never, () => current);
  const emit = async (name: string, event: any = {}) => { let result; for (const fn of hooks.get(name) ?? []) result = await fn(event, ctx); return result; };
  return { cwd, ctx, tools, records, live, current, emit };
}
const evidence = new Map([["E1", { id: "E1", toolCallId: "check", tool: "bash", passed: true, output: "3 tests pass" }]]);
const checks = [{ criterion: "tests pass", passed: true, evidence: ["E1"], note: "Ran the test suite" }];

test("reports require all criteria, real successful evidence, safe paths and unchanged artifacts", t => {
  const { cwd } = fixture(t);
  writeFileSync(join(cwd, "source.ts"), "original");
  assert.deepEqual(acceptanceCriteria({ task: "review" }), ["review"]);
  for (const criteria of [[], [""], ["a", "a"], ["x".repeat(8001)]]) assert.throws(() => acceptanceCriteria({ task: "review", acceptanceCriteria: criteria }));
  const report = verifyReport(checks, ["source.ts"], evidence, ["tests pass"], cwd);
  assert.equal(verificationProblem([report]), undefined);
  for (const invalid of [[], [{ ...checks[0], evidence: ["invented"] }], [{ ...checks[0], criterion: "other" }]]) assert.throws(() => verifyReport(invalid, [], evidence, ["tests pass"], cwd));
  assert.throws(() => verifyReport(checks, [], new Map([["E1", { ...evidence.get("E1")!, passed: false }]]), ["tests pass"], cwd), /Failed tool/);
  symlinkSync(tmpdir(), join(cwd, "outside"));
  assert.throws(() => verifyReport(checks, ["outside/secret"], evidence, ["tests pass"], cwd), /inside/);
  writeFileSync(join(cwd, "source.ts"), "changed");
  assert.match(verificationProblem([report])!, /changed/);
  rmSync(join(cwd, "source.ts"));
  assert.match(verificationProblem([report])!, /Cannot recheck/);
});

test("major records observed tool results, rejects empty/failed evidence and stays disabled in Work", async t => {
  const f = fixture(t, "major"), tool = f.tools.get("report_verification");
  await f.emit("agent_start");
  assert.match((await f.emit("before_agent_start")).message.content, /tests pass/);
  assert.equal(await f.emit("tool_result", { toolName: "write" }), undefined);
  const output = await f.emit("tool_result", { toolName: "bash", toolCallId: "check", content: [{ type: "text", text: "3 tests pass" }], isError: false });
  assert.match(output.content.at(-1).text, /E1/);
  const report = await tool.execute("report", { checks, artifacts: [] }, undefined, undefined, f.ctx);
  assert.equal(report.details.taucodeVerification.evidence[0].toolCallId, "check");
  writeFileSync(join(f.cwd, "code.ts"), "checked");
  await f.emit("agent_start");
  await f.emit("tool_result", { toolName: "read", toolCallId: "file", input: { path: "code.ts" }, content: [{ type: "text", text: "checked" }] });
  const fileReport = await tool.execute("report", { checks, artifacts: [] }, undefined, undefined, f.ctx);
  assert.equal(fileReport.details.taucodeVerification.artifacts[0].path, join(f.cwd, "code.ts"), "reviewed files cannot be omitted");
  writeFileSync(join(f.cwd, "code.ts"), "changed");
  await assert.rejects(tool.execute("report", { checks, artifacts: [] }, undefined, undefined, f.ctx), /changed since QA read/);
  for (const [content, isError] of [[[], false], [[{ type: "text", text: "failed" }], true]] as const) {
    await f.emit("agent_start");
    await f.emit("tool_result", { toolName: "bash", toolCallId: "check", content, isError });
    await assert.rejects(tool.execute("report", { checks, artifacts: [] }, undefined, undefined, f.ctx), /Failed tool/);
  }
  process.env.TAUCODE_WORK = "1";
  await assert.rejects(tool.execute("report", { checks, artifacts: [] }, undefined, undefined, f.ctx), /Only a Chain/);
});

test("root completion requires QA; retries repair verdicts, edits invalidate QA, aborts stay aborted", async t => {
  const f = fixture(t);
  assert.equal(f.tools.size, 0, "report tool is only exposed to majors");
  const report = verifyReport(checks, [], evidence, ["tests pass"], f.cwd);
  const result = (value = report) => ({ agent: "major", task: "review", acceptanceCriteria: ["tests pass"], status: "completed", startedAt: 20, endedAt: 30, messages: [{ role: "toolResult", toolName: "report_verification", details: { taucodeVerification: value } }] });
  const qa = (value = report, id = "qa") => f.emit("tool_execution_end", { toolName: "subagent", toolCallId: id, result: { details: { results: [result(value)] } } });
  await f.emit("agent_start"); await f.emit("agent_end"); assert.equal(f.records.length, 0, "no-tools replies need no QA");
  await f.emit("tool_execution_end", { toolName: "read", result: {} }); await f.emit("agent_end");
  assert.equal(f.records.at(-1).status, "unverified");
  await qa({ ...report, checks: [{ ...checks[0], passed: false }] }); await f.emit("agent_end");
  assert.equal(f.records.at(-1).status, "unverified");
  await qa(report, "retry"); await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "verified");
  assert.deepEqual(f.live.at(-1), f.records.at(-1));
  await f.emit("tool_execution_end", { toolName: "write", result: {} }); await f.emit("agent_end");
  assert.equal(f.records.at(-1).status, "unverified");
  await f.emit("agent_start");
  await f.emit("tool_execution_end", { toolName: "subagent", toolCallId: "batch", result: { details: { results: [result(), { agent: "enlisted", endedAt: 25, messages: [] }] } } });
  await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "unverified", "parallel implementation invalidates QA started too early");
  await qa(report, "fresh-review"); await f.emit("agent_end");
  assert.equal(f.records.at(-1).status, "unverified", "a review started before the change cannot repair it");
  await f.emit("tool_execution_end", { toolName: "subagent", toolCallId: "fresh", result: { details: { results: [{ ...result(), startedAt: 26 }] } } });
  await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "verified");
  const count = f.records.length;
  await f.emit("message_end", { message: { role: "assistant", stopReason: "aborted" } }); await f.emit("agent_end");
  assert.equal(f.records.length, count, "do not overwrite cancellation");
  await f.emit("agent_start");
  await f.emit("message_end", { message: { role: "assistant", stopReason: "error" } });
  await f.emit("tool_execution_end", { toolName: "read", result: {} });
  await f.emit("message_end", { message: { role: "assistant", stopReason: "stop" } });
  await f.emit("agent_end");
  assert.equal(f.records.length, count + 1, "a recovered transient error must not bypass verification");
  assert.equal(f.records.at(-1).status, "unverified");
  await f.emit("agent_start"); process.env.TAUCODE_CHAT = "1"; await qa(); await f.emit("agent_end");
  assert.equal(f.records.length, count + 1);
});

test("nested OMP reports survive virtual transport; malformed and stale child reports fail closed", async t => {
  const f = fixture(t);
  writeFileSync(join(f.cwd, "code.ts"), "one");
  const report = verifyReport(checks, ["code.ts"], evidence, ["tests pass"], f.cwd);
  const major = { agent: "major", task: "review", acceptanceCriteria: ["tests pass"], status: "completed", messages: [{ role: "toolResult", toolName: "write", details: { xdev: { tool: "report_verification", inner: { taucodeVerification: report } } } }] };
  const nested = () => f.emit("tool_execution_end", { toolName: "write", toolCallId: "nested", result: { details: { xdev: { inner: { results: [{ agent: "colonel", messages: [{ details: { results: [major] } }] }] } } } } });
  await nested(); await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "verified");
  writeFileSync(join(f.cwd, "code.ts"), "two");
  await nested(); await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "unverified");
  major.messages[0].details.xdev.inner.taucodeVerification = {} as any;
  await nested(); await f.emit("agent_end"); assert.equal(f.records.at(-1).status, "unverified");
});
