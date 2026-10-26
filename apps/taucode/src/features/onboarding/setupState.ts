export const SETUP_SEEN = "taucode.setup-checklist.v1";
export const SETUP_PROGRESS = "taucode.setup.v1";
export const steps = ["welcome", "look", "provider", "engine", "project", "features", "permissions", "done"] as const;
export type SetupStep = typeof steps[number];
export type SetupSection = "providers" | "engines" | "features";
export type SetupProgress = { step: SetupStep; completed: SetupStep[]; skipped: SetupStep[] };
export const emptyProgress = (): SetupProgress => ({ step: "welcome", completed: [], skipped: [] });
export function readProgress(): SetupProgress {
  try {
    const value = JSON.parse(localStorage.getItem(SETUP_PROGRESS) ?? "null");
    if (!value || !steps.includes(value.step)) return emptyProgress();
    return { step: value.step, completed: steps.filter(s => value.completed?.includes(s)), skipped: steps.filter(s => value.skipped?.includes(s)) };
  } catch { return emptyProgress(); }
}
export function previewStep(search: string): SetupStep | undefined {
  const value = new URLSearchParams(search).get("setup");
  return value === "1" ? "welcome" : steps.find(s => s === value);
}
