import { useEffect, useState } from "react";
import { computerRequest } from "@services/tauri";
import { ToastActions, ToastBody, ToastCard, ToastError, ToastHeader, ToastTitle, ToastViewport } from "@/features/design-system/components/toast/ToastPrimitives";
import type { ComputerRun } from "./computer";

export function ComputerApprovals() {
  const [approval, setApproval] = useState<ComputerRun["approval"]>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const next = await computerRequest<ComputerRun>({ action: "status" });
        if (active) setApproval(next.approval);
      } catch (e) { if (active) setError(String(e)); }
      if (active) timer = setTimeout(() => void poll(), 500);
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, []);
  if (!approval) return null;
  const decide = async (accept: boolean) => {
    setBusy(true); setError("");
    try {
      await computerRequest({ action: "approve", id: approval.id, accept });
      setApproval(null);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  return <ToastViewport className="approval-toasts" role="region" ariaLive="assertive" onKeyDown={event => event.stopPropagation()}>
    <ToastCard className="approval-toast" role="alert">
      <ToastHeader><ToastTitle>Computer Use approval needed</ToastTitle></ToastHeader>
      <ToastBody>{approval.reason}{approval.target ? ` (${approval.target})` : ""}</ToastBody>
      <ToastBody>{approval.app}</ToastBody>
      <ToastError>{JSON.stringify(approval.action, null, 2)}</ToastError>
      {error && <ToastError>{error}</ToastError>}
      <ToastActions>
        <button className="secondary" disabled={busy} onClick={() => void decide(false)}>Decline</button>
        <button className="primary" disabled={busy} onClick={() => void decide(true)}>Approve</button>
      </ToastActions>
    </ToastCard>
  </ToastViewport>;
}
