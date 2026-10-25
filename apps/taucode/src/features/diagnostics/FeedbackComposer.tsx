import "./diagnostics.css";
import { useEffect, useState } from "react";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { diagnosticsEnabled, record, openFeedback } from "./client";
export function FeedbackComposer() {
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [note, setNote] = useState("");
  const [type, setType] = useState("bug");
  const [attach, setAttach] = useState(false);
  useEffect(() => {
    const open = (e: Event) => { setDetail((e as CustomEvent).detail ?? {}); setNote(""); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") setDetail(null); if (diagnosticsEnabled() && (e.metaKey || e.ctrlKey) && e.shiftKey && !e.altKey && e.key.toLowerCase() === "l") { e.preventDefault(); setDetail({}); setNote(""); } };
    window.addEventListener("taucode:diagnostic-feedback", open); window.addEventListener("keydown", key);
    return () => { window.removeEventListener("taucode:diagnostic-feedback", open); window.removeEventListener("keydown", key); };
  }, []);
  if (!detail) return null;
  return <ModalShell cardClassName="diagnostics-feedback" onBackdropClick={() => setDetail(null)} ariaLabel="Log an issue or idea">
    <form onSubmit={e => { e.preventDefault(); record({ kind: "feedback", area: "ui", code: detail.responseId ? "feedback.response_flag" : "feedback." + type, message: detail.responseId ? "Flagged response" : "User " + type, context: detail, detail: { type, note, relatedFingerprint: detail.relatedFingerprint, attachedEvents: attach ? 20 : 0 } }); setDetail(null); }}>
      <h2>Log an issue or idea</h2>
      <label>Type <select value={type} onChange={e => setType(e.target.value)}><option value="bug">Bug</option><option value="idea">Idea</option><option value="perf">Performance</option></select></label>
      <label>Note <textarea value={note} maxLength={300} onChange={e => setNote(e.target.value)} autoFocus /></label>
      <label><input type="checkbox" checked={attach} onChange={e => setAttach(e.target.checked)} />Attach the last 20 events</label>
      <p>Saved locally. Avoid including private content.</p>
      <button type="submit">Save feedback</button><button type="button" onClick={() => setDetail(null)}>Cancel</button>
    </form>
  </ModalShell>;
}

export function DiagnosticsCommands() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  useEffect(() => {
    const show = () => { setOpen(true); setQuery(""); };
    window.addEventListener("taucode:diagnostic-commands", show);
    return () => window.removeEventListener("taucode:diagnostic-commands", show);
  }, []);
  if (!open) return null;
  return <ModalShell cardClassName="diagnostics-feedback" ariaLabel="Command palette" onBackdropClick={() => setOpen(false)}>
    <input aria-label="Search commands" autoFocus value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => { if (e.key === "Escape") setOpen(false); if (e.key === "Enter") { setOpen(false); openFeedback(); } }} />
    {"log an issue or idea".includes(query.toLowerCase()) && <button type="button" onClick={() => { setOpen(false); openFeedback(); }}>Log an issue or idea <kbd>⌘⇧L</kbd></button>}
    <button type="button" onClick={() => setOpen(false)}>Close</button>
  </ModalShell>;
}
