import { useEffect, useState } from "react";
import { computerRequest } from "@services/tauri";
import { ApprovalToasts } from "@app/components/ApprovalToasts";
import { ComputerRunItem } from "./ComputerRunItem";
import type { ComputerRun } from "./computer";
import "./computer.css";
export function ComputerHud() {
  const [run, setRun] = useState<ComputerRun>({ steps: [] });
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { const next = await computerRequest<ComputerRun>({ action: "status" }); if (active) setRun(next); }
      catch (e) { if (active) setError(String(e)); }
      if (active) timer = setTimeout(() => void poll(), 200);
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, []);
  const send = (params: Record<string, unknown>) => { void computerRequest(params).catch(e => setError(String(e))); };
  const approval = run.approval;
  return <main className="computer-hud">
    <header><strong>Computer Use</strong><span role="status">{run.running ? approval ? "Waiting for approval" : run.paused ? "Paused" : `Step ${(run.steps[run.steps.length - 1]?.step ?? 0) + 1}` : "Stopped"}</span></header>
    <p>Stop: {run.shortcut ?? "⌃⌥⌘."}</p>
    <div className="computer-controls"><button className="secondary" disabled={!run.running} onClick={() => send({ action: "pause", paused: !run.paused })}>{run.paused ? "Resume" : "Pause"}</button><button className="primary" disabled={!run.running} onClick={() => send({ action: "stop" })}>Stop</button></div>
    {error && <p role="alert">{error}</p>}
    <ComputerRunItem run={run} />
    <ApprovalToasts approvals={approval ? [{ workspace_id: "computer", request_id: approval.id, method: "computer/requestApproval", params: { reason: `${approval.reason} (${approval.target ?? ""})`, app: approval.app, action: approval.action } }] : []} workspaces={[]} onDecision={(request, decision) => send({ action: "approve", id: request.request_id, accept: decision === "accept" })} />
  </main>;
}
