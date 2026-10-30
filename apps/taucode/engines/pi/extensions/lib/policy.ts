import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, basename, resolve, relative, isAbsolute, sep } from "node:path";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";

export type Role = { name: string; description: string; model?: string; toolAccess: "read-only" | "full"; instructions: string; commandRank?: "colonel" | "major" | "enlisted" | null };
export type Policy = { enabled?: boolean; mode?: "custom" | "chain-of-command"; maxConcurrent: number; maxPerTurn: number; maxDepth: number; allowedModels: string[]; defaultModel?: string; roles: Role[]; models?: Record<string, string> };
export type Rule = { tool: string; command?: string; path?: string; cwd?: string };
export type Access = { accessMode: "read-only" | "ask" | "full-access"; sessionAllow: Rule[]; kind?: "work"; workspace?: string; readOnlyFolders?: string[]; skillRoot?: string };
export type Call = { tool: string; command?: string; path?: string; cwd: string };

// Resolve existing ancestors too: a new file below a symlink must not escape the workspace.
export function canonical(path: string): string {
  path = resolve(path);
  try {
    if (lstatSync(path).isSymbolicLink()) return canonical(resolve(dirname(path), readlinkSync(path)));
    return realpathSync(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  const parent = dirname(path);
  return parent === path ? path : resolve(canonical(parent), basename(path));
}
export function inside(path: string, root: string): boolean {
  const rel = relative(canonical(root), canonical(path));
  return rel === "" || (!isAbsolute(rel) && rel !== ".." && !rel.startsWith(`..${sep}`));
}
export function protectedPath(path: string, home: string): boolean {
  return [resolve(path), canonical(path)].some(p => p.split(sep).some(part => part === ".git" || part.startsWith(".env")) || inside(p, home));
}
export function readOnlyCommand(command: string): boolean {
  // Deliberately small grammar: no shell expansion, redirects, pipelines or executable flags.
  if (!/^[\w\s./:@,=+-]+$/.test(command) || /[\r\n]/.test(command)) return false;
  const [tool, ...args] = command.trim().split(/\s+/);
  if (tool === "git") {
    const [sub, ...rest] = args;
    return ["status", "diff", "log"].includes(sub) && rest.every(a => !a.startsWith("-") || /^(--(?:short|stat|name-only|name-status|oneline|no-ext-diff|no-textconv|cached|staged)|-[0-9]+)$/.test(a));
  }
  const flags: Record<string, RegExp> = { ls: /^-[alhRdt]+$/, cat: /^-[nbEsTv]+$/, rg: /^(-[niIlwFc]+|--(?:files|hidden|no-ignore))$/, grep: /^-[niIlwFErc]+$/, pwd: /^-[LP]$/ };
  return !!flags[tool] && args.every(a => !a.startsWith("-") || a === "--" || flags[tool].test(a));
}
export function normalizeRule(call: Call): Rule {
  // Exact normalized command is the safest command prefix (no wildcard or shell suffix).
  return call.tool === "bash"
    ? { tool: "bash", command: call.command?.trim(), cwd: canonical(call.cwd) }
    : { tool: call.tool, path: canonical(resolve(call.cwd, call.path ?? "")) };
}
export function safeReadOnlyCommand(command: string): string {
  if (!readOnlyCommand(command) || !command.trim().startsWith("git ")) return command;
  const [_, sub, ...args] = command.trim().split(/\s+/);
  return ["git", "--no-pager", "--no-optional-locks", "-c", "core.fsmonitor=false", sub, ...(["diff", "log"].includes(sub) ? ["--no-ext-diff", "--no-textconv"] : []), ...args].join(" ");
}
// Work file boundaries also apply to children. Full access authorizes shell
// execution; the shared protected-path and child restrictions still apply.
export function workDecision(access: Access, call: Call, interactive: boolean, child = false): "allow" | "ask" | "block" | undefined {
  if (access.kind !== "work") return;
  if (!access.workspace || !inside(call.cwd, access.workspace)) return "block";
  const path = resolve(call.cwd, call.path ?? ".");
  const readable = (p: string) => [access.workspace!, ...(access.readOnlyFolders ?? []), ...(access.skillRoot ? [access.skillRoot] : [])].some(root => inside(p, root));
  if (["read", "ls", "find", "grep"].includes(call.tool)) return readable(path) ? "allow" : "block";
  if (call.tool === "bash") {
    if (readOnlyCommand(call.command ?? "")) return (call.command ?? "").trim().split(/\s+/).slice(1).filter(p => !p.startsWith("-")).every(p => readable(resolve(call.cwd, p))) ? "allow" : "block";
    if (access.accessMode === "read-only") return "block";
    if (access.accessMode === "full-access") return;
    return interactive && !child ? "ask" : "block";
  }
  if (!inside(path, access.workspace)) return "block";
  if (access.accessMode === "read-only") return "block";
  const outputs = resolve(access.workspace, "outputs");
  const inOutputs = canonical(outputs) === resolve(canonical(access.workspace), "outputs") && inside(path, outputs);
  if (existsSync(path) && !inOutputs && access.accessMode !== "full-access") return interactive && !child ? "ask" : "block";
}
export function decide(access: Access, call: Call, home: string, interactive: boolean, child?: { workspace: string; toolAccess: string }): "allow" | "ask" | "block" {
  const work = workDecision(access, call, interactive, !!child);
  if (work) return work;
  const path = resolve(call.cwd, call.path ?? "");
  const readonly = call.tool === "bash" && readOnlyCommand(call.command ?? "");
  if (access.accessMode === "read-only" || child?.toolAccess === "read-only") return readonly ? "allow" : "block";
  const command = (call.command ?? "").replace(/\$(?:TAUCODE_WORK_PYTHON\b|\{TAUCODE_WORK_PYTHON\})/g, "");
  const protectedWrite = call.tool === "bash"
    ? !readonly && /(^|[\s/'"])(\.git(?:[/\s'";]|$)|\.env)|PI_CODING_AGENT_DIR|TAUCODE_|taucode\/threads/.test(command) || command.includes(home)
    : protectedPath(path, home);
  if (protectedWrite) return interactive && !child ? "ask" : "block";
  if (child && access.accessMode === "ask") return readonly || (call.tool !== "bash" && inside(path, child.workspace)) ? "allow" : "block";
  if (access.accessMode === "full-access") return "allow";
  if (access.accessMode !== "ask") return "block";
  const rule = JSON.stringify(normalizeRule(call));
  if (access.sessionAllow.some(r => JSON.stringify(r) === rule)) return "allow";
  return interactive ? "ask" : "block";
}
export function commandRank(name: string, roles: Role[]): string | undefined {
  if (["colonel", "major", "enlisted"].includes(name)) return name;
  const rank = roles.find(role => role.name === name)?.commandRank;
  return name !== "commander" && rank && ["colonel", "major", "enlisted"].includes(rank) ? rank : undefined;
}
export function commandTargets(role: string, roles: Role[]): string[] {
  const rank = role === "commander" ? role : commandRank(role, roles);
  const allowed = rank === "commander" ? ["colonel", "major", "enlisted"] : rank === "colonel" ? ["major", "enlisted"] : [];
  return roles.filter(role => allowed.includes(commandRank(role.name, roles) ?? "")).map(role => role.name);
}
export function commandInstructions(role: string, roles: Role[] = []): string {
  const instructions: Record<string, string> = {
    commander: "You are the commander in Chain of Command mode. Define the goal, constraints and acceptance criteria. Delegate bounded work to colonels or enlisted agents using their configured models. Prefer a configured specialist when its description matches the task. Use a major-rank role for independent verification before reporting completion; give it the acceptance criteria and artifact paths. If the agent budget prevents QA, report that limitation instead of claiming verification. Keep handoffs concise and include paths and evidence; avoid repeating whole transcripts. You own integration and the final response.",
    colonel: "You are a colonel. Break the assigned goal into bounded, non-overlapping tasks for enlisted agents. Prefer a configured specialist when its description matches the task. Reserve budget for a major-rank role to verify the result, or explicitly return unverified work to the commander. Integrate results and report artifacts, checks and remaining risks. Do not delegate recursively to another colonel.",
    major: "You are a QA major. Independently check the supplied acceptance criteria against the actual artifacts. Run the available checks within your tool permissions. Report pass/fail evidence and unresolved risks. Do not claim a check ran when it did not. You cannot delegate; return necessary fixes to your commander or colonel.",
    enlisted: "You are an enlisted implementation agent. Complete only the bounded assigned task, preserve unrelated work, and run relevant checks. Return concise artifact paths, changes, evidence and blockers. You cannot delegate.",
  };
  const rank = role === "commander" ? role : commandRank(role, roles);
  return (rank && instructions[rank] ? (role !== rank ? `Your role is ${role}, with ${rank} rank. ` : "") + instructions[rank] : "Unknown Chain of Command role. Delegation is blocked.");
}
// Atomic directory creation shares a finite budget across independent child processes.
export function reserveSubagent(directory: string, limit: number): void {
  for (let i = 0; i < limit; i++) {
    try { mkdirSync(resolve(directory, String(i))); return; }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error; }
  }
  throw new Error(`Subagent max per turn (${limit}) reached across the command tree`);
}
export function createSubagentBudget(home: string, limit: number): string {
  if (!Number.isInteger(limit) || limit < 1) throw new Error("Invalid managed concurrency limit");
  const directory = mkdtempSync(resolve(home, "taucode/subagent-turn-"));
  try {
    writeFileSync(resolve(directory, "concurrency"), String(limit));
    for (let i = 0; i < limit; i++) writeFileSync(resolve(directory, `slot-${i}-free`), "");
    return directory;
  } catch (error) { rmSync(directory, { recursive: true, force: true }); throw error; }
}
// Renaming a free token to a unique lease is atomic across processes. A stale
// cleanup can only release its own lease, never a later occupant of that slot.
export async function acquireSubagentSlot(directory: string, signal?: AbortSignal): Promise<string> {
  const limit = Number(readFileSync(resolve(directory, "concurrency"), "utf8"));
  if (!Number.isInteger(limit) || limit < 1) throw new Error("Invalid managed concurrency limit");
  while (true) {
    if (signal?.aborted) throw new Error("Subagent aborted");
    for (let i = 0; i < limit; i++) {
      const lease = resolve(directory, `slot-${i}-${process.pid}-${randomUUID()}`);
      try { renameSync(resolve(directory, `slot-${i}-free`), lease); return lease; }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
    }
    for (const name of readdirSync(directory)) {
      const owner = /^slot-\d+-(\d+)-/.exec(name);
      if (owner) try { process.kill(Number(owner[1]), 0); }
      catch (error) { if ((error as NodeJS.ErrnoException).code === "ESRCH") releaseSubagentSlot(resolve(directory, name)); }
    }
    await delay(25, undefined, { signal });
  }
}
export function adoptSubagentSlot(lease: string): string {
  const slot = /^slot-(\d+)-\d+-/.exec(basename(lease));
  if (!slot) throw new Error("Invalid managed subagent slot");
  const owned = resolve(dirname(lease), `slot-${slot[1]}-${process.pid}-${randomUUID()}`);
  renameSync(lease, owned);
  return owned;
}
export function releaseSubagentSlot(lease?: string): void {
  if (!lease) return;
  const slot = /^slot-(\d+)-\d+-/.exec(basename(lease));
  if (!slot) throw new Error("Invalid managed subagent slot");
  try { renameSync(lease, resolve(dirname(lease), `slot-${slot[1]}-free`)); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
}
export function releaseSubagentProcessSlots(directory: string, pid: number): void {
  let names: string[];
  try { names = readdirSync(directory); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return; throw error; }
  for (const name of names) if (/^slot-\d+-(\d+)-/.exec(name)?.[1] === String(pid)) releaseSubagentSlot(resolve(directory, name));
}
export function resolveTask(policy: Policy, task: { agent: string; model?: string | null }, parent?: string, parentRole = "commander") {
  if (policy.mode === "chain-of-command" && !commandTargets(parentRole, policy.roles).includes(task.agent)) throw new Error(`${parentRole} cannot delegate to ${task.agent} in Chain of Command mode`);
  const role = policy.roles.find(r => r.name === task.agent);
  if (!role) throw new Error(`Unknown role ${task.agent}. Available: ${policy.roles.map(r => r.name).join(", ")}`);
  const configured = role.model || policy.defaultModel || parent;
  if (policy.mode === "chain-of-command" && task.model && task.model !== configured) throw new Error(`Chain of Command uses the configured ${task.agent} model (${configured ?? "not set"}); change it in agent policy`);
  const model = task.model || configured;
  if (!model || !/^[^/]+\/.+$/.test(model)) throw new Error("Select a provider/model for the subagent");
  if (policy.allowedModels.length && !policy.allowedModels.includes(model)) throw new Error(`Model ${model} is not allowed. Allowed models: ${policy.allowedModels.join(", ")}`);
  if (policy.models && !policy.models[model]) throw new Error(`Model ${model} is not enabled in the provider registry`);
  const actual = policy.models?.[model] ?? model;
  const slash = actual.indexOf("/");
  return { role, model, provider: actual.slice(0, slash), modelId: actual.slice(slash + 1) };
}
export function checkLimits(policy: Policy, depth: number, count: number) {
  if (policy.enabled === false) throw new Error("Subagents are disabled");
  for (const field of ["maxConcurrent", "maxPerTurn", "maxDepth"] as const) if (!Number.isInteger(policy[field]) || policy[field] < 1) throw new Error(`Invalid ${field}`);
  if (!Number.isInteger(depth) || depth < 0 || depth >= policy.maxDepth) throw new Error(`Subagent max depth (${policy.maxDepth}) reached`);
  if (count >= policy.maxPerTurn) throw new Error(`Subagent max per turn (${policy.maxPerTurn}) reached`);
}
