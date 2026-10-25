import type { AppSettings, EngineDefinition } from "@/types";
import { useFeatureSettings } from "./features";
export const builtInEngines: EngineDefinition[] = [
  { id: "pi", name: "pi", protocol: "pi-rpc", builtIn: true, enabled: true },
  { id: "omp", name: "oh-my-pi", protocol: "pi-rpc", builtIn: true, enabled: false, description: "Bun runtime · LSP, debugger and Python/JS eval with approval · Install (~1.1 GB)" },
  { id: "codex", name: "Codex", protocol: "codex-app-server", builtIn: true, enabled: true },
  { id: "claude", name: "Claude Code (ACP)", protocol: "acp", builtIn: true, enabled: true },
];
export function engineRegistry(settings?: Pick<AppSettings, "engines"> | null): EngineDefinition[] {
  return [...builtInEngines.map(engine => ({ ...engine, enabled: settings?.engines?.find(e => e.id === engine.id)?.enabled ?? engine.enabled })), ...(settings?.engines ?? []).filter(e => !builtInEngines.some(b => b.id === e.id))];
}
export function resolveEngine(id: string, settings?: AppSettings | null) {
  const registry = engineRegistry(settings);
  const engine = registry.find(e => e.id === id);
  const fallback = registry.find(e => e.id === settings?.agentPolicy?.defaultEngine && e.enabled) ?? registry.find(e => e.enabled) ?? builtInEngines[0];
  return { engine: engine ?? fallback, notice: engine ? undefined : `Unknown engine ${id}; using ${fallback.name}.` };
}
export function useEngines() { return engineRegistry(useFeatureSettings()); }
