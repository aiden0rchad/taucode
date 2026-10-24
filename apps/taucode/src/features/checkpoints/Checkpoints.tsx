import { useEffect, useRef, useState } from "react";
import type { Checkpoint, CheckpointChange } from "@/types";
import { CLAUDE_REWIND_REASON, engineCapabilities } from "@threads/utils/engineCapabilities";
import { checkpointRequest } from "@services/tauri";
import { openCheckpoints, subscribeAppServerEvents, subscribeCheckpointOpen, type CheckpointOpen } from "@services/events";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { ToastCard, ToastViewport } from "@/features/design-system/components/toast/ToastPrimitives";
import "./checkpoints.css";

export function RewindAction({ workspaceId, threadId, userItemId, running }: CheckpointOpen & { running: boolean }) {
  return <button type="button" className="ghost" disabled={running} title={running ? "Finish or interrupt the turn before rewinding" : "Rewind files or conversation"} onClick={() => openCheckpoints({ workspaceId, threadId, userItemId })}>Rewind…</button>;
}

export function Checkpoints({ running, onChanged, requestCheckpoint = checkpointRequest }: {
  requestCheckpoint?: typeof checkpointRequest;
  running: Record<string, { isProcessing: boolean }>;
  onChanged: (workspace: string, thread: string, text?: string) => Promise<void>;
}) {
  const [request, setRequest] = useState<CheckpointOpen | null>(null);
  const [rows, setRows] = useState<Checkpoint[]>([]);
  const [target, setTarget] = useState<Checkpoint | null>(null);
  const [changes, setChanges] = useState<CheckpointChange[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mode, setMode] = useState("both");
  const [conversation, setConversation] = useState(false);
  const [engine, setEngine] = useState("");
  const [saving, setBusy] = useState(false);
  const [loadingList, setLoadingList] = useState(false);
  const [loadingDiff, setLoadingDiff] = useState(false);
  const busy = saving || loadingList || loadingDiff;
  const [error, setError] = useState("");
  const [undo, setUndo] = useState<CheckpointOpen | null>(null);
  const [warning, setWarning] = useState("");
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => subscribeCheckpointOpen((next) => { setRequest(next); setTarget(null); setError(""); }), []);
  useEffect(() => subscribeAppServerEvents((event) => {
    const params = event.message.params as { threadId?: string; warning?: string } | undefined;
    if ((event.message.method === "turn/started" || event.message.method === "checkpoint/turnStarting")) {
      setUndo((current) => current?.workspaceId === event.workspace_id && current.threadId === params?.threadId ? null : current);
    }
    if (event.message.method === "checkpoint/updated" && params?.warning) setWarning(params.warning);
  }), []);
  useEffect(() => {
    if (!request) return;
    let cancelled = false;
    setLoadingList(true);
    setConversation(false);
    const params = { threadId: request.threadId };
    Promise.all([requestCheckpoint(request.workspaceId, { ...params, action: "list" }), requestCheckpoint(request.workspaceId, { ...params, action: "capabilities" }).catch(() => ({ conversation: false, engine: "" }))]).then(([list, capabilities]) => {
      if (cancelled) return;
      const checkpoints = list.checkpoints ?? [];
      setRows(checkpoints);
      const supportsConversation = Boolean(capabilities.conversation) && engineCapabilities(capabilities.engine === "claude" ? "claude" : capabilities.engine === "pi" ? "pi" : "codex").conversationRewind;
      setConversation(supportsConversation);
      setEngine(capabilities.engine ?? "");
      setMode(supportsConversation ? "both" : "files");
      if (list.undoAvailable) setUndo(request);
      if (request.userItemId) {
        const row = checkpoints.find((row) => row.metadata.kind === "before" && row.metadata.userItemId === request.userItemId);
        if (row?.commit) setTarget(row);
        else setError(row?.warning ?? "Checkpoint unavailable for this message.");
      }
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); }).finally(() => { if (!cancelled) setLoadingList(false); });
    return () => { cancelled = true; };
  }, [request, requestCheckpoint]);
  useEffect(() => {
    if (!target || !request) { setLoadingDiff(false); return; }
    let cancelled = false;
    setLoadingDiff(true); setChanges([]); setError("");
    if (target.metadata.kind !== "before") setMode("files");
    requestCheckpoint(request.workspaceId, { threadId: request.threadId, action: "diff", checkpointId: target.id }).then((result) => {
      if (cancelled) return;
      setChanges(result.changes ?? []);
      setSelected(new Set(result.changes?.map((change) => change.path)));
    }).catch((e: unknown) => { if (!cancelled) setError(String(e)); }).finally(() => { if (!cancelled) setLoadingDiff(false); });
    return () => { cancelled = true; };
  }, [target, request, requestCheckpoint]);
  useEffect(() => {
    if (!request) return;
    const previous = document.activeElement as HTMLElement | null;
    content.current?.querySelector<HTMLElement>("button")?.focus();
    return () => previous?.focus();
  }, [request, requestCheckpoint]);
  const close = () => { if (!busy) setRequest(null); };
  const isRunning = Boolean(request && running[request.threadId]?.isProcessing);
  const allowConversation = conversation && target?.metadata.kind === "before";
  const conversationDisabledReason = !allowConversation ? engine === "claude" ? CLAUDE_REWIND_REASON : "Conversation rewind is unsupported by this engine or checkpoint" : undefined;
  const checked = changes.filter((change) => selected.has(change.path));
  const deletes = checked.filter((change) => change.status === "added since").length;
  const preview = target?.metadata.preview?.slice(0, 70) || "this message";
  const summary = [mode !== "conversation" ? `Restore ${checked.length - deletes} files, delete ${deletes}` : "", mode !== "files" ? `rewind conversation to before ‘${preview}’` : ""].filter(Boolean).join(", ");
  const confirm = async () => {
    if (!request || !target) return;
    setBusy(true); setError("");
    try {
      const result = await requestCheckpoint(request.workspaceId, { threadId: request.threadId, action: "rewind", checkpointId: target.id, mode, paths: [...selected] });
      if (result.undoAvailable) setUndo(request);
      if (result.conversationChanged) await onChanged(request.workspaceId, request.threadId, result.text);
      if (result.error) setError(result.error);
      else setRequest(null);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const undoRewind = async () => {
    if (!undo) return;
    setBusy(true);
    try {
      const result = await requestCheckpoint(undo.workspaceId, { threadId: undo.threadId, action: "undo" });
      if (result.conversationChanged) await onChanged(undo.workspaceId, undo.threadId);
      setUndo(null);
    } catch (e) { setWarning(String(e)); } finally { setBusy(false); }
  };
  return <>
    {(undo || warning) && <ToastViewport className="checkpoint-banner" ariaLive="polite"><ToastCard role="status">
      {undo && <>Rewound. <button type="button" className="ghost" disabled={busy || running[undo.threadId]?.isProcessing} onClick={() => void undoRewind()}>Undo</button></>}
      {warning && <span>{warning} <button type="button" className="ghost" onClick={() => setWarning("")} aria-label="Dismiss checkpoint warning">Dismiss</button></span>}
    </ToastCard></ToastViewport>}
    {request && <ModalShell ariaLabel="Checkpoints and rewind" onBackdropClick={close} cardClassName="checkpoint-card">
      <div ref={content} onKeyDown={(event) => {
        if (event.key === "Escape") close();
        if (event.key === "Tab") {
          const focusable = Array.from(content.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), summary') ?? []);
          const first = focusable[0], last = focusable[focusable.length - 1];
          if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
          else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
        }
      }}>
        <div className="checkpoint-heading"><h2>{target ? "Rewind" : "Checkpoints"}</h2><button type="button" className="ghost" disabled={busy} onClick={close}>Close</button></div>
        {error && <p role="alert">{error}</p>}
        {isRunning && <p>Finish or interrupt the turn before rewinding.</p>}
        {busy && <p role="status">Loading checkpoint…</p>}
        {!target ? <div className="checkpoint-list">{!busy && rows.length === 0 && <p>No checkpoints yet.</p>}{[...rows].reverse().map((row) => <article key={row.id}>
          <time>{new Date(row.metadata.timestamp).toLocaleString()}</time> · {row.metadata.kind} · {row.filesChanged} files changed
          <p>{row.metadata.preview}</p>
          {row.warning && <p>{row.warning}</p>}
          {row.skipped.length > 0 && <details><summary>{row.skipped.length} files skipped</summary>{row.skipped.join(", ")}</details>}
          <button type="button" className="ghost" disabled={!row.commit || busy || isRunning} onClick={() => setTarget(row)}>Rewind to here</button>
        </article>)}</div> : <>
          <label className="checkpoint-mode">Mode <select value={mode} disabled={busy || isRunning} onChange={(event) => setMode(event.target.value)}>
            <option value="both" disabled={!allowConversation} title={conversationDisabledReason}>Files and conversation</option>
            <option value="files">Files only</option>
            <option value="conversation" disabled={!allowConversation} title={conversationDisabledReason}>Conversation only</option>
          </select></label>
          {!allowConversation && <p title={conversationDisabledReason}>Conversation rewind unavailable. Use Files only.</p>}
          {engine === "codex" && mode !== "files" && <p>Undo restores files. Codex conversation rollback cannot be undone.</p>}
          {target.skipped.length > 0 && <p>Skipped files: {target.skipped.join(", ")}</p>}
          <div className="checkpoint-files">{changes.map((change) => <div key={change.path} className="checkpoint-file">
            <label><input type="checkbox" checked={selected.has(change.path)} disabled={busy || isRunning || mode === "conversation"} onChange={(event) => setSelected((previous) => { const next = new Set(previous); if (event.target.checked) next.add(change.path); else next.delete(change.path); return next; })} /> {change.path} <span>{change.status}</span></label>
            {change.status === "added since" && <p className="checkpoint-deletion">This file will be deleted.</p>}
            <details><summary>View diff for {change.path}</summary><pre>{change.diff || "No text diff available."}</pre></details>
          </div>)}</div>
          {!busy && changes.length === 0 && <p>No restorable file changes.</p>}
          <p>{summary}</p>
          <div className="checkpoint-actions"><button type="button" className="ghost" disabled={busy} onClick={() => setTarget(null)}>All checkpoints</button><button type="button" className="primary" disabled={busy || isRunning || Boolean(error) || (mode === "files" && checked.length === 0)} onClick={() => void confirm()}>Confirm rewind</button></div>
        </>}
      </div>
    </ModalShell>}
  </>;
}
