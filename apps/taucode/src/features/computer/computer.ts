import type { ModelRef } from "@/types";
export type ComputerSettings = { mode: "background" | "foreground"; allowForegroundFallback: boolean; virtualCursor: boolean; enabled: boolean; planner: ModelRef | null; grounding: ModelRef | null; groundingMode: "elements" | "pixels" | "ui-tars"; groundingWidth: number; groundingHeight: number; allowedApps: string[]; deniedApps: string[]; maxSteps: number; maxSeconds: number; shortcut: string };
export const defaultComputerSettings: ComputerSettings = { mode: "background", allowForegroundFallback: false, virtualCursor: true, enabled: false, planner: null, grounding: null, groundingMode: "elements", groundingWidth: 1920, groundingHeight: 1080, allowedApps: [], deniedApps: [], maxSteps: 30, maxSeconds: 300, shortcut: "Control+Alt+Super+Period" };
export type ComputerStep = { path?: "ax-element" | "pixel-skylight" | "ax" | "skylight" | "postToPid" | "foreground" | null; window?: { id: number; title: string }; focusRestorations?: { pid: number; windowId?: number; reason: string }[]; foregroundApproval?: boolean; step: number; kind?: "approval_wait"; reason?: string; target?: string; predictMs?: number; executeMs?: number; waitMs?: number; thumbnail: string; action: Record<string, unknown>; reflection: string; planText?: string; error?: string | null };
export type ComputerRun = { id?: string; stored?: boolean; running?: boolean; paused?: boolean; steps: ComputerStep[]; summary?: string; error?: string; instruction?: string; shortcut?: string; setup?: string; permissions?: { screen: boolean; accessibility: boolean; unsupported?: boolean }; approval?: { id: string; reason: string; target?: string; action: Record<string, unknown>; app: string } | null };
export function parseComputerRun(output?: string): ComputerRun | null {
  if (!output?.includes('computerRun')) return null;
  try {
    const value = JSON.parse(output);
    const run = value.computerRun;
    if (run && Array.isArray(run.steps) && run.steps.every((s: ComputerStep) => typeof s.step === "number" && typeof s.thumbnail === "string" && s.action && typeof s.action === "object" && typeof s.reflection === "string")) return run;
    if (Array.isArray(value.content)) {
      for (const c of value.content) { if (c.type === "text") { const run = parseComputerRun(c.text); if (run) return run; } }
    }
  } catch { /* Other tool output is rendered normally. */ }
  return null;
}
