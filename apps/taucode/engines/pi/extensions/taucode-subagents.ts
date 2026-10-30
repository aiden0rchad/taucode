// Derived from pi's examples/extensions/subagent (MIT, Copyright Mario Zechner).
// Keeps single/parallel/chain JSON streaming; roles come only from taucode policy.
import { execFileSync, spawn } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { subagentCheckpoint } from "./lib/recovery.ts";
import { acquireSubagentSlot, adoptSubagentSlot, checkLimits, commandInstructions, commandTargets, createSubagentBudget, inside, releaseSubagentProcessSlots, releaseSubagentSlot, reserveSubagent, resolveTask, type Policy } from "./lib/policy.ts";

const optionalString = Type.Optional(Type.Union([Type.String(), Type.Null()]));
const taskFields = { agent: Type.String(), task: Type.String(), model: optionalString, cwd: optionalString };
const Task = Type.Object(taskFields);
type TaskInput = { agent: string; task: string; model?: string | null; cwd?: string | null };
type Message = { role: string; toolCallId?: string; toolName?: string; content?: { type: string; text?: string }[]; stopReason?: string; errorMessage?: string; usage?: Record<string, any> };
type Result = TaskInput & { exitCode: number; status: string; startedAt?: number; endedAt?: number; messages: Message[]; stderr: string; stopReason?: string; usage: Record<string, number> };
const output = (r: Result) => r.messages.filter(m => m.role === "assistant").at(-1)?.content?.filter(c => c.type === "text").map(c => c.text).join("\n") || r.stderr;

export default function (pi: ExtensionAPI) {
  let count = 0;
  let active = 0;
  let turnBudget: string | undefined;
  const inheritedSlot = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0) > 0 ? process.env.TAUCODE_SUBAGENT_SLOT : undefined;
  const inheritedBudget = process.env.TAUCODE_SUBAGENT_BUDGET;
  if (inheritedSlot && (!inheritedBudget || !inside(inheritedBudget, join(process.env.PI_CODING_AGENT_DIR!, "taucode")) || !inside(inheritedSlot, inheritedBudget))) throw new Error("Invalid managed subagent slot");
  let ownSlot = inheritedSlot ? adoptSubagentSlot(inheritedSlot) : undefined;
  let delegating = 0;
  const clearBudget = () => { if (turnBudget) rmSync(turnBudget, { recursive: true, force: true }); turnBudget = undefined; };
  const finish = () => { clearBudget(); releaseSubagentSlot(ownSlot); ownSlot = undefined; };
  const waiters = new Set<() => void>();
  const completedCheckpoints = new Set<ReturnType<typeof subagentCheckpoint>>();
  let parentWatch: ReturnType<typeof setInterval> | undefined;
  pi.on("session_start", (_event, ctx) => {
    ctx.ui.setStatus("taucode-subagents", "ready");
    if (process.platform !== "win32") {
      const parent = process.ppid;
      clearInterval(parentWatch);
      parentWatch = setInterval(() => {
        if (process.ppid !== parent) { clearInterval(parentWatch); ctx.abort(); }
      }, 500).unref();
    }
  });
  pi.on("agent_start", () => { count = 0; clearBudget(); });
  pi.on("agent_end", () => { finish(); for (const checkpoint of completedCheckpoints) checkpoint.discard(); completedCheckpoints.clear(); });
  pi.on("session_shutdown", () => { clearInterval(parentWatch); finish(); });
  pi.on("before_agent_start", () => {
    const current: Policy = JSON.parse(readFileSync(join(process.env.PI_CODING_AGENT_DIR!, "taucode/policy.json"), "utf8"));
    const role = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0) === 0 ? "commander" : process.env.TAUCODE_SUBAGENT_ROLE ?? "unknown";
    const chain = current.enabled !== false && current.mode === "chain-of-command";
    const targets = commandTargets(role, current.roles);
    const visible = { ...current, models: undefined, ...(chain ? { roles: current.roles.filter(r => targets.includes(r.name)) } : {}) };
    return { message: { customType: "taucode-subagent-policy", display: false, content: `${chain ? commandInstructions(role, current.roles) + "\n" : ""}Subagent policy: ${JSON.stringify(visible)}` } };
  });
  pi.registerTool({
    name: "subagent",
    label: "Subagent",
    description: "Delegate using taucode policy roles only (no user/project agent directories). Single: agent + task; parallel: tasks; chain: chain with {previous} substitution. Optional model per task is provider/model. Chain of Command fixes each role's model and shares turn and concurrency limits across the command tree; commanders and colonels delegate, majors verify, enlisted implement. Waiting coordinators lend their concurrency slot to workers and reclaim one before resuming. Custom delegation limits each delegator's immediate children. Children inherit thread access and are non-interactive: read-only stays read-only; in ask mode full roles may write only inside the workspace, bash only on the read-only allowlist. Protected paths require interactive approval and are blocked in children.",
    parameters: Type.Object({ agent: optionalString, task: optionalString, model: taskFields.model, cwd: taskFields.cwd, tasks: Type.Optional(Type.Union([Type.Array(Task), Type.Null()])), chain: Type.Optional(Type.Union([Type.Array(Task), Type.Null()])) }),
    async execute(_id, params, signal, onUpdate, ctx) {
      const home = process.env.PI_CODING_AGENT_DIR!;
      const policy: Policy = JSON.parse(readFileSync(join(home, "taucode/policy.json"), "utf8"));
      const depth = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0);
      // Strict-schema providers send empty arrays/nulls for unused optional modes.
      const modes = [!!(params.agent || params.task), !!params.tasks?.length, !!params.chain?.length];
      if (modes.filter(Boolean).length !== 1 || "agentScope" in params) throw new Error("Provide exactly one of agent + task, tasks, or chain. Agent directories are disabled.");
      const mode = modes[2] ? "chain" : modes[1] ? "parallel" : "single";
      const tasks = (mode === "chain" ? params.chain : mode === "parallel" ? params.tasks : [params]) as TaskInput[];
      if (!tasks.length || tasks.some(task => !task.agent?.trim() || !task.task?.trim())) throw new Error("Provide at least one task with a non-empty agent and task");
      const results: Result[] = tasks.map(task => ({ ...task, exitCode: -1, status: "pending", messages: [], stderr: "", usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0, contextTokens: 0, turns: 0 } }));
      const snapshot = () => ({ content: [{ type: "text" as const, text: results.map(r => `${r.agent}: ${output(r) || r.status}`).join("\n\n") }], details: { mode, results } });
      const checkpoint = subagentCheckpoint(depth === 0 ? ctx.sessionManager?.getSessionFile() : undefined, _id);
      const update = () => { const result = structuredClone(snapshot()); checkpoint.update(result); onUpdate?.(result); };
      const parentActual = ctx.model ? `${ctx.model.provider}/${ctx.model.id}` : undefined;
      const parent = Object.entries(policy.models ?? {}).find(([, actual]) => actual === parentActual)?.[0] ?? parentActual;
      const thinking = pi.getThinkingLevel?.();
      const run = async (r: Result) => {
        let acquired = false;
        let slot: string | undefined;
        let budget: string | undefined;
        try {
          checkLimits(policy, depth, count);
          const parentRole = depth === 0 ? "commander" : process.env.TAUCODE_SUBAGENT_ROLE ?? "unknown";
          const { role, model, provider, modelId } = resolveTask(policy, r, parent, parentRole);
          const workspace = process.env.TAUCODE_WORKSPACE ?? ctx.cwd;
          const cwd = r.cwd || ctx.cwd;
          if (!inside(cwd, workspace)) throw new Error("Subagent cwd must be inside the workspace");
          if (signal?.aborted) throw new Error("Subagent aborted");
          if (policy.mode === "chain-of-command") {
            budget = depth === 0 ? turnBudget ??= createSubagentBudget(home, policy.maxConcurrent) : process.env.TAUCODE_SUBAGENT_BUDGET;
            if (!budget || !inside(budget, join(home, "taucode"))) throw new Error("Missing managed Chain of Command budget");
            reserveSubagent(budget, policy.maxPerTurn);
          }
          count++;
          while (active >= policy.maxConcurrent) {
            if (signal?.aborted) throw new Error("Subagent aborted");
            await new Promise<void>(resolve => {
              const wake = () => { waiters.delete(wake); signal?.removeEventListener("abort", wake); resolve(); };
              waiters.add(wake);
              signal?.addEventListener("abort", wake, { once: true });
            });
          }
          if (signal?.aborted) throw new Error("Subagent aborted");
          active++; acquired = true;
          if (budget) slot = await acquireSubagentSlot(budget, signal);
          r.model = model; r.status = "running"; r.startedAt = Date.now(); update();
          const cli = process.env.TAUCODE_PI_CLI ?? process.argv[1];
          if (!cli) throw new Error("Missing managed pi CLI");
          const args = [...process.execArgv, cli, "--mode", "json", "-p", "--no-session", ...(process.env.TAUCODE_OMP === "1" ? ["--no-lsp", "--no-pty", "--no-title", "--no-skills", "--no-rules"] : ["--no-approve"]), "--no-extensions", "-e", join(home, "extensions/taucode-gate.ts"), "-e", join(home, "extensions/taucode-subagents.ts"), "--provider", provider, "--model", modelId];
          const toolAccess = process.env.TAUCODE_TOOL_ACCESS === "read-only" ? "read-only" : role.toolAccess;
          if (thinking) args.push("--thinking", thinking);
          if (toolAccess === "read-only") args.push("--tools", process.env.TAUCODE_OMP === "1" ? "read,grep,glob" : "read,grep,find,ls");
          if (role.instructions) args.push("--append-system-prompt", role.instructions);
          args.push(`Task: ${r.task}`);
          r.exitCode = await new Promise<number>((resolve, reject) => {
            const child = spawn(process.execPath, args, { cwd, env: { ...process.env, TAUCODE_SUBAGENT_DEPTH: String(depth + 1), TAUCODE_SUBAGENT_ROLE: role.name, TAUCODE_SUBAGENT_BUDGET: budget, TAUCODE_SUBAGENT_SLOT: slot, TAUCODE_WORKSPACE: workspace, TAUCODE_TOOL_ACCESS: toolAccess }, stdio: ["ignore", "pipe", "pipe"], detached: depth === 0 && process.platform !== "win32" });
            const releaseOwners = (owners: Iterable<number>) => {
              try { if (budget) for (const pid of owners) releaseSubagentProcessSlots(budget, pid); }
              catch (error) { r.stderr += `\nConcurrency cleanup failed: ${error}`; r.stopReason = "error"; }
            };
            let stopping: Promise<void> | undefined;
            const abort = () => {
              if (stopping) return;
              const targets = new Set<number>();
              const owners = new Set<number>();
              if (child.pid) targets.add(depth === 0 && process.platform !== "win32" ? -child.pid : child.pid);
              if (child.pid) owners.add(child.pid);
              if (child.pid && process.platform !== "win32") {
                try {
                  // Snapshot before signalling: shell tools can lead separate groups,
                  // and their ancestry disappears as soon as a coordinator exits.
                  const rows = execFileSync("/bin/ps", ["-axo", "pid=,ppid=,pgid="], { encoding: "utf8", timeout: 1000 })
                    .trim().split("\n").map(line => line.trim().split(/\s+/).map(Number))
                    .filter(row => row.length === 3 && row.every(Number.isInteger) && row[0] > 0 && row[1] >= 0 && row[2] > 0);
                  const owned = new Set([child.pid]);
                  let size: number;
                  do {
                    size = owned.size;
                    for (const [pid, parent] of rows) if (owned.has(parent)) owned.add(pid);
                  } while (owned.size !== size);
                  for (const [pid, , group] of rows) if (owned.has(pid)) {
                    owners.add(pid);
                    targets.delete(pid);
                    targets.add(owned.has(group) ? -group : pid);
                  }
                } catch {
                  r.stderr += "\nProcess-tree inspection failed; stopping the known subagent process group only.";
                }
              }
              const kill = (sig: NodeJS.Signals) => {
                for (const target of targets) { try { process.kill(target, sig); } catch { /* already exited */ } }
              };
              stopping = new Promise(resolve => {
                kill("SIGTERM");
                setTimeout(() => { kill("SIGKILL"); releaseOwners(owners); resolve(); }, 1000);
              });
            };
            signal?.addEventListener("abort", abort, { once: true });
            const lines = createInterface({ input: child.stdout });
            lines.on("line", line => {
              let event;
              try { event = JSON.parse(line); } catch { return; }
              if (event.type === "message_end" || event.type === "tool_result_end") {
                const msg = event.message as Message;
                if (!msg) return;
                const previous = msg.role === "toolResult" && msg.toolCallId
                  ? r.messages.findIndex(m => m.role === "toolResult" && m.toolCallId === msg.toolCallId) : -1;
                if (previous >= 0) r.messages[previous] = msg;
                else r.messages.push(msg);
                if (msg.role === "assistant") {
                  r.usage.turns++;
                  for (const key of ["input", "output", "cacheRead", "cacheWrite"]) r.usage[key] += msg.usage?.[key] ?? 0;
                  r.usage.cost += msg.usage?.cost?.total ?? 0;
                  r.usage.contextTokens = msg.usage?.totalTokens ?? 0;
                  r.stopReason = msg.stopReason;
                  if (msg.errorMessage) r.stderr = msg.errorMessage;
                }
                update();
              } else if (["tool_execution_update", "tool_execution_end"].includes(event.type)) {
                const snapshot = event.partialResult ?? event.result;
                const details = snapshot?.details;
                const results = event.toolName === "subagent" ? details?.results
                  : ["write", "edit"].includes(event.toolName) && details?.xdev?.tool === "subagent" ? details.xdev.inner?.results : undefined;
                if (!Array.isArray(results)) return;
                const msg = { ...snapshot, role: "toolResult", toolName: event.toolName, toolCallId: event.toolCallId };
                const previous = r.messages.findIndex(m => m.role === "toolResult" && m.toolCallId === event.toolCallId);
                if (previous >= 0) r.messages[previous] = msg;
                else r.messages.push(msg);
                update();
              } else if (event.type === "message_update") {
                const partial = event.message as Message | undefined;
                if (partial) { r.messages.push(partial); update(); r.messages.pop(); }
              }
            });
            child.stderr.on("data", data => { r.stderr = (r.stderr + data.toString()).slice(-16000); });
            child.on("error", reject);
            child.on("close", async code => {
              // Do not release the slot while a detached descendant can still run.
              await stopping;
              if (child.pid) releaseOwners([child.pid]);
              signal?.removeEventListener("abort", abort); lines.close(); resolve(code ?? 1);
            });
            if (signal?.aborted) abort();
          });
          r.status = r.exitCode === 0 && !["error", "aborted"].includes(r.stopReason ?? "") && !signal?.aborted ? "completed" : "failed";
        } catch (e) {
          r.stderr = String(e); r.exitCode = 1; r.status = "failed";
        } finally {
          if (signal?.aborted) r.stopReason = "aborted";
          r.endedAt = Date.now();
          try { releaseSubagentSlot(slot); }
          catch (error) { r.stderr += `\nConcurrency cleanup failed: ${error}`; r.status = "failed"; r.exitCode = 1; }
          if (acquired) { active--; for (const wake of [...waiters]) wake(); }
          update();
        }
      };
      // A coordinator cannot occupy the only shared slot while awaiting a child.
      // Parallel tool calls share this suspension and reacquire only after the last.
      if (inheritedSlot && delegating++ === 0) { releaseSubagentSlot(ownSlot); ownSlot = undefined; }
      try {
      if (mode === "chain") {
        let previous = "";
        for (const r of results) {
          r.task = r.task.replaceAll("{previous}", previous);
          await run(r);
          if (r.status === "failed") {
            for (const pending of results.filter(v => v.status === "pending")) { pending.status = "failed"; pending.exitCode = 1; pending.stderr = "Previous chain step failed"; }
            break;
          }
          previous = output(r);
        }
      } else await Promise.all(results.map(run));
      const result = { ...snapshot(), isError: results.some(r => r.status === "failed") };
      checkpoint.update(structuredClone(result)); checkpoint.flush();
      // OMP can replace a cancelled tool's result with a generic abort error.
      // Keep one branch-aware recovery entry, outside the model's context.
      if (depth === 0 && signal?.aborted) pi.appendEntry("taucode-subagent-result", { toolCallId: _id, result });
      completedCheckpoints.add(checkpoint);
      return result;
      } finally {
        if (inheritedSlot && --delegating === 0 && !signal?.aborted) {
          try { ownSlot = await acquireSubagentSlot(inheritedBudget!, signal); }
          catch (error) { ctx.abort(); throw error; } // Never resume model work without a slot.
        }
      }
    },
  });
}
