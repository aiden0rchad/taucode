import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { commandInstructions, resolveTask, type Policy } from "../extensions/lib/policy.ts";
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
      let tool; subagents({ on() {}, registerTool(t) { tool = t; } });
      const result = await tool.execute("nested", { tasks: [1,2].map(n => ({agent:"enlisted",task:"task "+n})) }, undefined, undefined, {cwd:process.cwd(),model:{provider:"taucode-cloud",id:"mid"}});
      console.log(JSON.stringify({type:"tool_execution_end",toolName:"subagent",toolCallId:"nested",result}));
    }
    console.log(JSON.stringify({type:"message_end",message:{role:"assistant",content:[{type:"text",text:JSON.stringify({role,provider:args[args.indexOf("--provider")+1],model:args[args.indexOf("--model")+1]})}],stopReason:"stop"}}));`);
  let tool: any;
  const events: Record<string, (...args: any[]) => any> = {};
  subagents({ on: (name: string, fn: any) => { events[name] = fn; }, registerTool: (value: any) => { tool = value; } } as any);
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
    assert.deepEqual(JSON.parse(child.messages[0].content[0].text), { role: "enlisted", provider: "taucode-local", model: "small" });
  }
  assert.match((await tool.execute("qa", { agent: "major", task: "review" }, undefined, undefined, ctx)).details.results[0].stderr, /across the command tree/);
  events.agent_end();
  assert.deepEqual(readdirSync(join(home, "taucode")), ["policy.json"]);
  events.agent_start();
  const qa = await tool.execute("qa", { agent: "major", task: "review", model: "cloud/strong", cwd, tasks: [], chain: [] }, undefined, undefined, ctx);
  assert.equal(qa.isError, false);
  assert.deepEqual(JSON.parse(qa.details.results[0].messages[0].content[0].text), { role: "major", provider: "taucode-cloud", model: "strong" });
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
