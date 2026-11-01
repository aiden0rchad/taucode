import type { ModelRef } from "@/types";
export type ComputerSettings = { mode: "background" | "foreground"; allowForegroundFallback: boolean; virtualCursor: boolean; enabled: boolean; planner: ModelRef | null; grounding: ModelRef | null; groundingMode: "elements" | "pixels" | "ui-tars"; groundingWidth: number; groundingHeight: number; allowedApps: { bundleId: string; access: "full" | "read-only" }[]; confirmTyping: "always" | "sensitive" | "risky-only"; deniedApps: string[]; maxSteps: number; maxSeconds: number; plannerSeconds: number; appAccess: "ask" | "listed"; askApproval: boolean; shortcut: string };
export const defaultComputerSettings: ComputerSettings = { mode: "background", allowForegroundFallback: false, virtualCursor: true, enabled: false, planner: null, grounding: null, groundingMode: "elements", groundingWidth: 1920, groundingHeight: 1080, allowedApps: [], confirmTyping: "risky-only", deniedApps: [], maxSteps: 30, maxSeconds: 300, plannerSeconds: 120, appAccess: "ask", askApproval: false, shortcut: "Control+Alt+Super+Period" };
export type ComputerStep = { recovered?: boolean; grounding?: Record<string, unknown>[]; executionFallbackReason?: string | null; path?: "ax-hit-test" | "ax-element" | "pixel-skylight" | "ax" | "skylight" | "postToPid" | "foreground" | null; window?: { id: number; title: string }; focusRestorations?: { pid: number; windowId?: number; reason: string }[]; foregroundApproval?: boolean; step: number; kind?: "approval_wait"; reason?: string; target?: string; predictMs?: number; executeMs?: number; waitMs?: number; thumbnail?: string; action: Record<string, unknown>; reflection: string; planText?: string; error?: string | null };
export type ComputerRun = { mode?: "background" | "foreground"; targetApp?: { bundleId: string; name?: string; pid?: number; window?: { id: number; title: string } | null }; id?: string; stored?: boolean; running?: boolean; paused?: boolean; phase?: "starting" | "observing" | "planning" | "acting" | "waitingForApproval" | "settling"; steps: ComputerStep[]; summary?: string; error?: string; instruction?: string; shortcut?: string; setup?: string; permissions?: { screen: boolean; accessibility: boolean; unsupported?: boolean }; approval?: { id: string; kind?: "app"; name?: string; warning?: string | null; reason: string; target?: string; action?: Record<string, unknown>; app: string } | null };
export function parseComputerRun(output?: string): ComputerRun | null {
  if (!output?.includes('computerRun')) return null;
  try {
    const value = JSON.parse(output.trim().replace(/^Error:\s*/, ""));
    // Failed OMP MCP calls preserve the tool's JSON inside error.message.
    if (typeof value?.message === "string") return parseComputerRun(value.message);
    const run = value.computerRun;
    const stored = run?.stored === true && typeof run.id === "string" && run.id.length > 0;
    if (run && Array.isArray(run.steps) && run.steps.every((s: ComputerStep) => s && typeof s.step === "number" && (typeof s.thumbnail === "string" || (stored && s.thumbnail === undefined)) && s.action && typeof s.action === "object" && !Array.isArray(s.action) && typeof s.reflection === "string")) return run;
    if (Array.isArray(value.content)) {
      for (const c of value.content) { if (c.type === "text") { const run = parseComputerRun(c.text); if (run) return run; } }
    }
  } catch { /* Other tool output is rendered normally. */ }
  return null;
}
