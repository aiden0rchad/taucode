// Derived from pi's examples/extensions/subagent (MIT, Copyright Mario Zechner).
// Keeps single/parallel/chain JSON streaming; roles come only from taucode policy.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createInterface } from "node:readline";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { checkLimits, inside, resolveTask, type Policy } from "./lib/policy.ts";

const taskFields = { agent: Type.String(), task: Type.String(), model: Type.Optional(Type.String()), cwd: Type.Optional(Type.String()) };
const Task = Type.Object(taskFields);
type TaskInput = { agent: string; task: string; model?: string; cwd?: string };
type Message = { role: string; content?: { type: string; text?: string }[]; stopReason?: string; errorMessage?: string; usage?: Record<string, any> };
type Result = TaskInput & { exitCode: number; status: string; messages: Message[]; stderr: string; stopReason?: string; usage: Record<string, number> };
const output = (r: Result) => r.messages.filter(m => m.role === "assistant").at(-1)?.content?.filter(c => c.type === "text").map(c => c.text).join("\n") || r.stderr;

export default function (pi: ExtensionAPI) {
  let count = 0;
  let active = 0;
  const waiters = new Set<() => void>();
  pi.on("session_start", (_event, ctx) => { ctx.ui.setStatus("taucode-subagents", "ready"); });
  pi.on("agent_start", () => { count = 0; });
  pi.on("before_agent_start", () => {
    const current: Policy = JSON.parse(readFileSync(join(process.env.PI_CODING_AGENT_DIR!, "taucode/policy.json"), "utf8"));
    return { message: { customType: "taucode-subagent-policy", display: false, content: `Subagent policy: ${JSON.stringify(current)}` } };
  });
  pi.registerTool({
    name: "subagent",
    label: "Subagent",
    description: "Delegate using taucode policy roles only (no user/project agent directories). Single: agent + task; parallel: tasks; chain: chain with {previous} substitution. Optional model per task is provider/model. Policy enforces concurrent, per-turn, depth and model limits. Children inherit thread access and are non-interactive: read-only stays read-only; in ask mode full roles may write only inside the workspace, bash only on the read-only allowlist. Protected paths require interactive approval and are blocked in children.",
    parameters: Type.Object({ agent: Type.Optional(taskFields.agent), task: Type.Optional(taskFields.task), model: taskFields.model, cwd: taskFields.cwd, tasks: Type.Optional(Type.Array(Task)), chain: Type.Optional(Type.Array(Task)) }),
    async execute(_id, params, signal, onUpdate, ctx) {
      const home = process.env.PI_CODING_AGENT_DIR!;
      const policy: Policy = JSON.parse(readFileSync(join(home, "taucode/policy.json"), "utf8"));
      const depth = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0);
      const modes = [!!(params.agent && params.task), !!params.tasks?.length, !!params.chain?.length];
      if (modes.filter(Boolean).length !== 1 || "agentScope" in params) throw new Error("Provide exactly one of agent + task, tasks, or chain. Agent directories are disabled.");
      const mode = params.chain ? "chain" : params.tasks ? "parallel" : "single";
      const tasks = (params.chain ?? params.tasks ?? [params]) as TaskInput[];
      const results: Result[] = tasks.map(task => ({ ...task, exitCode: -1, status: "pending", messages: [], stderr: "", usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0, contextTokens: 0, turns: 0 } }));
      const snapshot = () => ({ content: [{ type: "text" as const, text: results.map(r => `${r.agent}: ${output(r) || r.status}`).join("\n\n") }], details: { mode, results } });
      const update = () => onUpdate?.(structuredClone(snapshot()));
      const parentActual = ctx.model ? `${ctx.model.provider}/${ctx.model.id}` : undefined;
      const parent = Object.entries(policy.models ?? {}).find(([, actual]) => actual === parentActual)?.[0] ?? parentActual;
      const run = async (r: Result) => {
        let acquired = false;
        try {
          checkLimits(policy, depth, count);
          const { role, model, provider, modelId } = resolveTask(policy, r, parent);
          const workspace = process.env.TAUCODE_WORKSPACE ?? ctx.cwd;
          const cwd = r.cwd ?? ctx.cwd;
          if (!inside(cwd, workspace)) throw new Error("Subagent cwd must be inside the workspace");
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
          r.model = model; r.status = "running"; update();
          const cli = process.env.TAUCODE_PI_CLI ?? process.argv[1];
          if (!cli) throw new Error("Missing managed pi CLI");
          const args = [...process.execArgv, cli, "--mode", "json", "-p", "--no-session", "--no-approve", "--no-extensions", "-e", join(home, "extensions/taucode-gate.ts"), "-e", join(home, "extensions/taucode-subagents.ts"), "--provider", provider, "--model", modelId];
          const toolAccess = process.env.TAUCODE_TOOL_ACCESS === "read-only" ? "read-only" : role.toolAccess;
          if (toolAccess === "read-only") args.push("--tools", "read,grep,find,ls");
          if (role.instructions) args.push("--append-system-prompt", role.instructions);
          args.push(`Task: ${r.task}`);
          r.exitCode = await new Promise<number>((resolve, reject) => {
            const child = spawn(process.execPath, args, { cwd, env: { ...process.env, TAUCODE_SUBAGENT_DEPTH: String(depth + 1), TAUCODE_WORKSPACE: workspace, TAUCODE_TOOL_ACCESS: toolAccess }, stdio: ["ignore", "pipe", "pipe"], detached: depth === 0 && process.platform !== "win32" });
            let timer: ReturnType<typeof setTimeout> | undefined;
            const kill = (sig: NodeJS.Signals) => { try { if (depth === 0 && process.platform !== "win32" && child.pid) process.kill(-child.pid, sig); else child.kill(sig); } catch { /* already exited */ } };
            const abort = () => { kill("SIGTERM"); timer = setTimeout(() => kill("SIGKILL"), 1000); };
            signal?.addEventListener("abort", abort, { once: true });
            const lines = createInterface({ input: child.stdout });
            lines.on("line", line => {
              let event;
              try { event = JSON.parse(line); } catch { return; }
              if (event.type === "message_end" || event.type === "tool_result_end") {
                const msg = event.message as Message;
                if (!msg) return;
                r.messages.push(msg);
                if (msg.role === "assistant") {
                  r.usage.turns++;
                  for (const key of ["input", "output", "cacheRead", "cacheWrite"]) r.usage[key] += msg.usage?.[key] ?? 0;
                  r.usage.cost += msg.usage?.cost?.total ?? 0;
                  r.usage.contextTokens = msg.usage?.totalTokens ?? 0;
                  r.stopReason = msg.stopReason;
                  if (msg.errorMessage) r.stderr = msg.errorMessage;
                }
                update();
              } else if (event.type === "message_update") {
                const partial = event.message as Message | undefined;
                if (partial) { r.messages.push(partial); update(); r.messages.pop(); }
              }
            });
            child.stderr.on("data", data => { r.stderr = (r.stderr + data.toString()).slice(-16000); });
            child.on("error", reject);
            child.on("close", code => { clearTimeout(timer); signal?.removeEventListener("abort", abort); lines.close(); resolve(code ?? 1); });
            if (signal?.aborted) abort();
          });
          r.status = r.exitCode === 0 && !["error", "aborted"].includes(r.stopReason ?? "") && !signal?.aborted ? "completed" : "failed";
        } catch (e) {
          r.stderr = String(e); r.exitCode = 1; r.status = "failed";
        } finally {
          if (acquired) { active--; for (const wake of [...waiters]) wake(); }
          update();
        }
      };
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
      return { ...snapshot(), isError: results.some(r => r.status === "failed") };
    },
  });
}
