import { useEffect, useState } from "react";
import { computerRequest } from "@services/tauri";
import type { ComputerRun } from "./computer";
import "./computer.css";

export function ComputerApprovalStatus({ detail }: { detail: string }) {
  const [run, setRun] = useState<ComputerRun | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let instruction: unknown;
    try { instruction = JSON.parse(detail).instruction; } catch { return; }
    if (typeof instruction !== "string") return;
    let active = true;
    let runId: string | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const run = await computerRequest<ComputerRun>({ action: "status" });
        const matches = run.running && run.instruction === instruction && (!runId || run.id === runId);
        if (matches) runId = run.id;
        if (active) setRun(matches ? run : null);
      } catch { if (active) setRun(null); }
      if (active) timer = setTimeout(() => void poll(), 500);
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [detail]);
  if (!run) return null;
  const control = async (action: "pause" | "stop") => {
    setBusy(true); setError("");
    try {
      const current = await computerRequest<ComputerRun>({ action: "status" });
      if (!current.running || current.id !== run.id) { setRun(null); return; }
      await computerRequest(action === "pause" ? { action, paused: !current.paused } : { action });
      const next = await computerRequest<ComputerRun>({ action: "status" });
      setRun(next.running && next.id === run.id ? next : null);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const lastStep = run.steps[run.steps.length - 1];
  return <section className="computer-live" aria-label="Computer Use controls" onKeyDown={event => event.stopPropagation()}>
    <div className="computer-live-summary" role="status">
      <strong>{run.paused ? "Computer Use paused" : "Computer Use running"}</strong>
      <span>{run.approval ? `Waiting for approval: ${run.approval.reason}${run.approval.target ? ` (${run.approval.target})` : ""}. Review the approval in taucode.` : run.paused ? "Resume to take a fresh look before the next action." : lastStep ? `Last action · Step ${lastStep.step}: ${lastStep.target || lastStep.action.kind || "completed"}` : "Preparing the first action…"}</span>
    </div>
    <div className="computer-live-actions">
      <button type="button" className="secondary" disabled={busy} onClick={() => void control("pause")}>{run.paused ? "Resume" : "Pause"} computer use</button>
      <button type="button" className="secondary" disabled={busy} onClick={() => void control("stop")}>Stop computer use</button>
    </div>
    {error && <p role="alert">{error}</p>}
  </section>;
}

export function ComputerRunItem({ run: reference }: { run: ComputerRun }) {
  const [loaded, setLoaded] = useState<ComputerRun | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    if (reference.stored && reference.id) void computerRequest<ComputerRun>({ action: "runLog", id: reference.id }).then(run => { if (active) setLoaded(run); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [reference.id, reference.stored]);
  const run = loaded?.id === reference.id && loaded ? loaded : reference;
  return <section aria-label="Computer run" className="computer-run">
    {error && <p role="alert">{error}</p>}
    {run.instruction && <p>{run.instruction}</p>}
    <ol>{run.steps.map(step => <li key={`${step.step}-${step.kind ?? "action"}`}>
      {step.kind === "approval_wait" && <p>Approval required: {step.reason} ({step.target})</p>}
      <details><summary>Step {step.step}: {step.kind === "approval_wait" ? "Approval wait" : String(step.action.kind ?? "action")}</summary>
        {step.thumbnail && <img width={240} src={`data:image/png;base64,${step.thumbnail}`} alt={`Screen before step ${step.step}`} />}
        {step.path && <p>Input: {step.path}{step.foregroundApproval ? " · takeover approved" : ""}</p>}
        {step.executionFallbackReason && <p>Pixel fallback: {step.executionFallbackReason}</p>}
        {step.window && <p>Window {step.window.id}: {step.window.title}</p>}
        {!!step.focusRestorations?.length && <p>Focus restored: {step.focusRestorations.length}</p>}
        <pre>{JSON.stringify(step.action, null, 2)}</pre>
        {!!step.grounding?.length && <details><summary>Grounding decisions</summary><pre>{JSON.stringify(step.grounding, null, 2)}</pre></details>}
        {step.reflection && <p>{step.reflection}</p>}
        {step.predictMs !== undefined && <p>Predict: {step.predictMs} ms · Execute: {step.executeMs ?? 0} ms · Wait: {step.waitMs ?? 0} ms</p>}
        {step.error && <p role="alert">{step.error}</p>}
      </details>
    </li>)}</ol>
    {run.summary && <p>{run.summary}</p>}{run.error && <p role="alert">{run.error}</p>}
  </section>;
}
