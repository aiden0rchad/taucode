import { useEffect, useState } from "react";
import { computerRequest } from "@services/tauri";
import type { ComputerRun } from "./computer";

export function ComputerApprovalStatus({ detail }: { detail: string }) {
  const [approval, setApproval] = useState<ComputerRun["approval"]>(null);
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
        if (active) setApproval(matches ? run.approval : null);
      } catch { if (active) setApproval(null); }
      if (active) timer = setTimeout(() => void poll(), 500);
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [detail]);
  return approval ? <p role="status">Waiting for approval: {approval.reason} ({approval.target}). Review in the Computer Use HUD.</p> : null;
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
        <pre>{JSON.stringify(step.action, null, 2)}</pre>
        {step.reflection && <p>{step.reflection}</p>}
        {step.predictMs !== undefined && <p>Predict: {step.predictMs} ms · Execute: {step.executeMs ?? 0} ms · Wait: {step.waitMs ?? 0} ms</p>}
        {step.error && <p role="alert">{step.error}</p>}
      </details>
    </li>)}</ol>
    {run.summary && <p>{run.summary}</p>}{run.error && <p role="alert">{run.error}</p>}
  </section>;
}
