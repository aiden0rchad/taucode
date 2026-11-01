import "./diagnostics.css";
import { useEffect, useState } from "react";
import { SettingsPage, SettingsGroup, SettingsFieldRow, SettingsSelect, SettingsTextArea, SettingsToggleRow, SettingsToggleSwitch, SettingsButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { diagnosticsEnabled, record } from "./client";
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
    <form onSubmit={e => { e.preventDefault(); if (!note.trim()) return; record({ kind: "feedback", area: "ui", code: detail.responseId ? "feedback.response_flag" : "feedback." + type, message: detail.responseId ? "Flagged response" : "User " + type, context: detail, detail: { type, note, relatedFingerprint: detail.relatedFingerprint, attachedEvents: attach ? 20 : 0 } }); setDetail(null); }}>
      <SettingsPage title="Log an issue or idea" subtitle="Saved locally. Avoid including private content."><SettingsGroup title="Feedback">
      <SettingsFieldRow title="Type"><SettingsSelect value={type} onChange={e => setType(e.target.value)}><option value="bug">Bug</option><option value="idea">Idea</option><option value="perf">Performance</option></SettingsSelect></SettingsFieldRow>
      <SettingsFieldRow title="Note"><SettingsTextArea value={note} maxLength={300} onChange={e => setNote(e.target.value)} autoFocus /></SettingsFieldRow>
      <SettingsToggleRow title="Attach the last 20 events"><SettingsToggleSwitch pressed={attach} onClick={() => setAttach(!attach)} /></SettingsToggleRow>
      </SettingsGroup><div className="diagnostics-feedback-actions">
      <SettingsButton type="button" onClick={() => setDetail(null)}>Cancel</SettingsButton><SettingsButton type="submit" className="primary" disabled={!note.trim()}>Save feedback</SettingsButton></div></SettingsPage>
    </form>
  </ModalShell>;
}
