import type { AgentPolicy, AppSettings, ModelRef, ProviderEntry } from "@/types";
export const defaultAgentPolicy: AgentPolicy = {
  defaultEngine: "pi", defaultModel: null,
  subagents: { enabled: true, maxConcurrent: 4, maxPerTurn: 8, maxDepth: 1, allowedModels: [], defaultModel: null,
    roles: [
      { name: "scout", description: "Explore quickly", model: null, toolAccess: "read-only", instructions: "" },
      { name: "planner", description: "Plan changes", model: null, toolAccess: "read-only", instructions: "" },
      { name: "worker", description: "Implement changes", model: null, toolAccess: "full", instructions: "" },
    ],
  },
};
export const modelKey = (m: ModelRef | null | undefined) => m ? m.providerId + "/" + m.modelId : "";
export function modelRef(key: string): ModelRef | null {
  const slash = key.indexOf("/");
  return slash < 0 ? null : { providerId: key.slice(0, slash), modelId: key.slice(slash + 1) };
}
export function formatModelRef(settings: Pick<AppSettings, "providers"> | null | undefined, ref: ModelRef | string | null | undefined) {
  const parsed = typeof ref === "string" ? modelRef(ref) : ref;
  const provider = settings?.providers?.find(p => p.id === parsed?.providerId);
  const name = provider?.models.find(m => m.id === parsed?.modelId)?.displayName || parsed?.modelId || (typeof ref === "string" ? ref : "") || "Select model";
  return { name, title: provider ? `${provider.name} · ${name}` : name };
}
export function providerSlug(name: string, providers: ProviderEntry[]) {
  const base = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").slice(0, 32).replace(/^-|-$/g, "") || "provider";
  let id = base;
  for (let n = 2; providers.some(p => p.id.replace(/-/g, "_") === id.replace(/-/g, "_")); n++) id = `${base.slice(0, 31 - String(n).length).replace(/-$/, "")}-${n}`;
  return id;
}
export function codexReady(settings: Pick<AppSettings, "providers"> | null | undefined) {
  return !!settings?.providers?.some(p => p.enabled && providerEngines(p).includes("codex") && p.models.some(m => m.enabled));
}
export function providerEngines(p: ProviderEntry) {
  const engines = p.kind === "openai" || p.kind === "openrouter" || p.kind === "ollama" || (p.kind === "openai-compatible" && p.apiStyle === "responses") ? ["pi", "codex"] : ["pi"];
  if (p.kind === "anthropic" || p.anthropicCompatible) engines.push("claude");
  return engines;
}
export function policyError(p: AgentPolicy, providers: ProviderEntry[]): string | null {
  const s = p.subagents;
  if (!Number.isInteger(s.maxConcurrent) || s.maxConcurrent < 1 || s.maxConcurrent > 16 || !Number.isInteger(s.maxPerTurn) || s.maxPerTurn < 1 || s.maxPerTurn > 32 || !Number.isInteger(s.maxDepth) || s.maxDepth < 1 || s.maxDepth > 3) return "Limits: concurrent 1–16, per turn 1–32, depth 1–3.";
  const enabled = new Set(providers.filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => p.id + "/" + m.id)));
  for (const m of [p.defaultModel, s.defaultModel, ...s.allowedModels, ...s.roles.map(r => r.model)]) {
    if (m && !enabled.has(modelKey(m))) return "Enable provider and model " + formatModelRef({ providers }, m).title;
  }
  const allowed = new Set(s.allowedModels.map(modelKey));
  if (allowed.size && [s.defaultModel, ...s.roles.map(r => r.model)].some(m => m && !allowed.has(modelKey(m)))) return "Role and default subagent models must be in allowed models.";
  if (new Set(s.roles.map(r => r.name)).size !== s.roles.length || s.roles.some(r => !/^[a-z0-9_-]{1,32}$/.test(r.name))) return "Role names must be unique slugs.";
  return null;
}
