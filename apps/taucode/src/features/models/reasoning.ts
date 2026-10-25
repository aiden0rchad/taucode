import { useState } from "react";
import type { ReasoningCapability } from "@/types";

export const reasoningLevels = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
export const reasoningLabel = (level: string) => level === "xhigh" ? "XHigh" : level[0]?.toUpperCase() + level.slice(1);
export function clampReasoning(capability: ReasoningCapability | null | undefined, requested?: string | null) {
  if (!capability?.supported || !capability.levels.length) return null;
  const found = reasoningLevels.indexOf(requested || capability.default || "medium");
  const index = found < 0 ? 3 : found;
  return capability.levels.reduce((best, level) => Math.abs(reasoningLevels.indexOf(level) - index) < Math.abs(reasoningLevels.indexOf(best) - index) ? level : best);
}
export const modelEffortKey = (model: string) => `taucode.reasoning.model:${model}`;
export const threadEffortKey = (thread: string) => `taucode.reasoning.thread:${thread}`;
export function useReasoningSelection(model: string, thread: string | null, capability?: ReasoningCapability | null) {
  const [selection, setSelection] = useState<{ thread: string | null; value: string | null }>(() => ({ thread, value: (thread ? localStorage.getItem(threadEffortKey(thread)) : null) ?? localStorage.getItem(modelEffortKey(model)) ?? capability?.default ?? null }));
  const requested = selection?.thread === thread ? selection.value : (thread ? localStorage.getItem(threadEffortKey(thread)) : null) ?? localStorage.getItem(modelEffortKey(model));
  const level = clampReasoning(capability, requested);
  return { level, requested, onChange: (value: string) => {
    setSelection({ thread, value });
    if (thread) localStorage.setItem(threadEffortKey(thread), value);
    localStorage.setItem(modelEffortKey(model), value);
  } };
}
