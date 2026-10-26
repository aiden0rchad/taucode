import type { ReactNode } from "react";
import { LoaderCircle } from "lucide-react";

const tones = {
  granted: "success", ready: "success", connected: "success",
  "not-granted": "warning", "needs-setup": "warning",
  checking: "muted", error: "error", optional: "muted", off: "muted",
} as const;

export function SetupStatusChip({ state, children }: { state: keyof typeof tones; children: ReactNode }) {
  return <span role="status" className={`setup-chip setup-chip-${tones[state]}`} data-state={state}>
    {state === "checking" && <LoaderCircle className="setup-spinner" size={12} aria-hidden="true" />}
    {children}
  </span>;
}
