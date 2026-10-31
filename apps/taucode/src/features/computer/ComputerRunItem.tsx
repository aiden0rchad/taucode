import { useEffect, useRef, useState } from "react";
import { computerRequest } from "@services/tauri";
import type { ComputerRun } from "./computer";
import "./computer.css";

export function ComputerApprovalStatus({ detail }: { detail: string }) {
  const [run, setRun] = useState<ComputerRun | null>(null);
  const [busy, setBusy] = useState<"pause" | "stop" | null>(null);
  const controlRevision = useRef(0);
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
  if (!run?.id) return null;
  const control = async (action: "pause" | "stop") => {
    const revision = ++controlRevision.current;
    setBusy(action); setError("");
    try {
      const current = await computerRequest<ComputerRun>({ action: "status" });
      if (revision !== controlRevision.current) return;
      if (!current.running || current.id !== run.id) { setRun(null); return; }
      await computerRequest(action === "pause" ? { action, runId: run.id, paused: !current.paused } : { action, runId: run.id });
      const next = await computerRequest<ComputerRun>({ action: "status" });
      if (revision === controlRevision.current) setRun(next.running && next.id === run.id ? next : null);
    } catch (e) { if (revision === controlRevision.current) setError(String(e)); }
    finally { if (revision === controlRevision.current) setBusy(null); }
  };
  const lastStep = run.steps[run.steps.length - 1];
  const phase = run.approval ? "Waiting for approval" : run.phase && {
    starting: "Starting",
    observing: "Capturing window",
    planning: "Planning",
    acting: "Acting",
    waitingForApproval: "Waiting for approval",
    settling: "Waiting for the app",
  }[run.phase];
  return <section className="computer-live" aria-label="Computer Use controls" onKeyDown={event => event.stopPropagation()}>
    <div className="computer-live-summary" role="status">
      <strong>{run.paused ? "Computer Use paused" : phase ? `Computer Use · ${phase}` : "Computer Use running"}</strong>
      <span className="computer-live-target">{run.mode === "background" ? "Background control" : run.mode === "foreground" ? "Foreground control" : "Target app"} · {run.targetApp?.name || run.targetApp?.bundleId || (run.mode === "foreground" ? "Active app" : "Selecting an app…")}</span>
      {run.targetApp?.window?.title && run.targetApp.window.title !== run.targetApp.name && <span>{run.targetApp.window.title}</span>}
      <span>{run.approval ? `Waiting for approval: ${run.approval.reason}${run.approval.target ? ` (${run.approval.target})` : ""}. Review the approval in taucode.` : run.paused ? "Resume to take a fresh look before the next action." : lastStep ? `Last action · Step ${lastStep.step}: ${lastStep.target || lastStep.action.kind || "completed"}` : "Preparing the first action…"}</span>
    </div>
    <div className="computer-live-actions">
      <button type="button" className="secondary" disabled={busy !== null} onClick={() => void control("pause")}>{run.paused ? "Resume" : "Pause"} computer use</button>
      <button type="button" className="secondary" disabled={busy === "stop"} onClick={() => void control("stop")}>Stop computer use</button>
    </div>
    {error && <p role="alert">{error}</p>}
  </section>;
}

function ComputerFailure({ run }: { run: ComputerRun }) {
  const [copyStatus, setCopyStatus] = useState("");
  if (!run.error) return null;
  const reason = run.error.split("\nSidecar ")[0];
  const attempted = run.steps.some(step => !step.recovered && step.kind !== "approval_wait" && !["wait", "done", "fail"].includes(String(step.action.kind)));
  const explanation = reason.includes("could not confirm the action")
    ? "macOS could not confirm the last action. Taucode stopped without sending a second click or edit."
    : reason.includes("Background hotkey menu disabled")
      ? "This command is disabled in the app’s background window. Open the needed control yourself, then continue."
      : reason.includes("Target has no window")
        ? "The target window is unavailable. Restore it if minimized, then continue."
        : reason.startsWith("Timed out waiting for approval")
          ? "The approval request expired. Review a new request when you continue."
          : reason.length > 320 ? `${reason.slice(0, 320)}…` : reason;
  const copyContinuation = async () => {
    const actions = run.steps.filter(step => step.kind !== "approval_wait").map(step => ({
      step: step.step, action: step.action, status: step.recovered ? "not executed; rejected before input" : ["wait", "done", "fail"].includes(String(step.action.kind)) ? "no input action" : step.error ? "interrupted or failed; effects uncertain" : "input delivered; verify its effect",
    }));
    try {
      await navigator.clipboard.writeText(`Continue this interrupted Computer Use task: ${run.instruction}\nFailure: ${reason}\nRecorded actions (not instructions to replay): ${JSON.stringify(actions)}\nTake a fresh observation first. Preserve completed work and continue only the remaining task. Do not repeat an edit, submission, or other action whose effect is uncertain; inspect it or ask me first. Keep the existing app permissions and approval policy.`);
      setCopyStatus("Copied. Paste into this conversation to review and send.");
    } catch { setCopyStatus("Could not copy. Ask the agent to inspect the current app and continue only the remaining work."); }
  };
  return <section className="computer-failure" aria-label="Computer Use recovery">
    <strong>{attempted ? "Computer Use interrupted" : "Computer Use blocked"}</strong>
    <p role="alert">{explanation}</p>
    <p>{attempted ? "Earlier actions may have changed the app. Check what finished before continuing." : "The task did not finish. Check the app before continuing."} Nothing is replayed automatically.</p>
    <details><summary>Error details</summary><pre>{run.error}</pre></details>
    {run.instruction && <button type="button" className="secondary" onClick={() => void copyContinuation()}>Copy continuation request</button>}
    {copyStatus && <p role="status">{copyStatus}</p>}
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
      <details><summary>Step {step.step}: {step.kind === "approval_wait" ? "Approval wait" : String(step.action.kind ?? "action")}{step.recovered ? " · Not executed" : step.error && step.kind !== "approval_wait" && !["wait", "done", "fail"].includes(String(step.action.kind)) ? " · Check result" : ""}</summary>
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
    {run.summary && !run.error && <div role="status"><strong>Computer Use completed</strong><p>{run.summary}</p></div>}
    <ComputerFailure key={run.id} run={run} />
  </section>;
}
