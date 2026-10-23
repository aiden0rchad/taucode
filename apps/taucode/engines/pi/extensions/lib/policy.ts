import { lstatSync, readlinkSync, realpathSync } from "node:fs";
import { dirname, basename, resolve, relative, isAbsolute, sep } from "node:path";

export type Role = { name: string; description: string; model?: string; toolAccess: "read-only" | "full"; instructions: string };
export type Policy = { enabled?: boolean; maxConcurrent: number; maxPerTurn: number; maxDepth: number; allowedModels: string[]; defaultModel?: string; roles: Role[]; models?: Record<string, string> };
export type Rule = { tool: string; command?: string; path?: string; cwd?: string };
export type Access = { accessMode: "read-only" | "ask" | "full-access"; sessionAllow: Rule[] };
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
export function decide(access: Access, call: Call, home: string, interactive: boolean, child?: { workspace: string; toolAccess: string }): "allow" | "ask" | "block" {
  const path = resolve(call.cwd, call.path ?? "");
  const readonly = call.tool === "bash" && readOnlyCommand(call.command ?? "");
  if (access.accessMode === "read-only" || child?.toolAccess === "read-only") return readonly ? "allow" : "block";
  const protectedWrite = call.tool === "bash"
    ? !readonly && /(^|[\s/'"])(\.git(?:[/\s'";]|$)|\.env)|PI_CODING_AGENT_DIR|TAUCODE_|taucode\/threads/.test(call.command ?? "") || (call.command ?? "").includes(home)
    : protectedPath(path, home);
  if (protectedWrite) return interactive && !child ? "ask" : "block";
  if (child && access.accessMode === "ask") return readonly || (call.tool !== "bash" && inside(path, child.workspace)) ? "allow" : "block";
  if (access.accessMode === "full-access") return "allow";
  if (access.accessMode !== "ask") return "block";
  const rule = JSON.stringify(normalizeRule(call));
  if (access.sessionAllow.some(r => JSON.stringify(r) === rule)) return "allow";
  return interactive ? "ask" : "block";
}
export function resolveTask(policy: Policy, task: { agent: string; model?: string }, parent?: string) {
  const role = policy.roles.find(r => r.name === task.agent);
  if (!role) throw new Error(`Unknown role ${task.agent}. Available: ${policy.roles.map(r => r.name).join(", ")}`);
  const model = task.model || role.model || policy.defaultModel || parent;
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
