import "./diagnostics.css";
import { useEffect, useState } from "react";
import { SettingsPage, SettingsGroup, SettingsFieldRow, SettingsSelect, SettingsTextArea, SettingsTextField, SettingsToggleRow, SettingsToggleSwitch, SettingsButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { diagnosticsEnabled, record, openFeedback } from "./client";
export function FeedbackComposer() {
  const [detail, setDetail] = useState<Record<string, unknown> | null>(null);
  const [note, setNote] = useState("");
  const [type, setType] = useState("bug");
  const [attach, setAttach] = useState(false);
  useEffect(() => {
    const open = (e: Event) => { setDetail((e as CustomEvent).detail ?? {}); setNote(""); setAttach(false); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") setDetail(null); if (diagnosticsEnabled() && (e.metaKey || e.ctrlKey) && e.shiftKey && !e.altKey && e.key.toLowerCase() === "l") { e.preventDefault(); setDetail({}); setNote(""); setAttach(false); } };
    window.addEventListener("taucode:diagnostic-feedback", open); window.addEventListener("keydown", key);
    return () => { window.removeEventListener("taucode:diagnostic-feedback", open); window.removeEventListener("keydown", key); };
  }, []);
  if (!detail) return null;
  return <ModalShell cardClassName="diagnostics-feedback" onBackdropClick={() => setDetail(null)} ariaLabel="Log an issue or idea">
    <form onSubmit={e => { e.preventDefault(); record({ kind: "feedback", area: "ui", code: detail.responseId ? "feedback.response_flag" : "feedback." + type, message: detail.responseId ? "Flagged response" : "User " + type, context: detail, detail: { type, note, relatedFingerprint: detail.relatedFingerprint, attachedEvents: attach ? 20 : 0 } }); setDetail(null); }}>
      <SettingsPage title="Log an issue or idea" subtitle="Saved locally. Avoid including private content."><SettingsGroup title="Feedback">
      <SettingsFieldRow title="Type"><SettingsSelect value={type} onChange={e => setType(e.target.value)}><option value="bug">Bug</option><option value="idea">Idea</option><option value="perf">Performance</option></SettingsSelect></SettingsFieldRow>
      <SettingsFieldRow title="Note"><SettingsTextArea value={note} maxLength={300} onChange={e => setNote(e.target.value)} autoFocus /></SettingsFieldRow>
      <SettingsToggleRow title="Attach the last 20 events"><SettingsToggleSwitch pressed={attach} onClick={() => setAttach(!attach)} /></SettingsToggleRow>
      </SettingsGroup><div className="diagnostics-feedback-actions">
      <SettingsButton type="submit">Save feedback</SettingsButton><SettingsButton type="button" onClick={() => setDetail(null)}>Cancel</SettingsButton></div></SettingsPage>
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
    <SettingsTextField aria-label="Search commands" autoFocus value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => { if (e.key === "Escape") setOpen(false); if (e.key === "Enter") { setOpen(false); openFeedback(); } }} />
    {"log an issue or idea".includes(query.toLowerCase()) && <SettingsButton type="button" onClick={() => { setOpen(false); openFeedback(); }}>Log an issue or idea <kbd>⌘⇧L</kbd></SettingsButton>}
    <SettingsButton type="button" onClick={() => setOpen(false)}>Close</SettingsButton>
  </ModalShell>;
}
