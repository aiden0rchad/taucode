import { useEffect, useState } from "react";
import { Camera, Download, ExternalLink } from "lucide-react";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { getAppBuildType, issueReportDraft, issueReportScreenshot, openIssueReport } from "@services/tauri";
import { getIssueReportContext, withModalsHidden } from "./issueReportContext";
import "./issue-report.css";

export const ISSUE_REPORT_INCLUDES = [
  "taucode version, build type and commit",
  "macOS version and processor architecture",
  "Engine and model of the conversation in front",
  "Open issues from the diagnostics journal: codes, counts and fingerprints",
];

type Props = { onClose: () => void; fingerprint?: string };

export function IssueReportDialog({ onClose, fingerprint }: Props) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    let live = true;
    void (async () => {
      const buildType = await getAppBuildType().catch(() => "unknown");
      const draft = await issueReportDraft({ ...getIssueReportContext(), buildType, commit: __APP_COMMIT_HASH__ || undefined, fingerprint });
      if (live) { setTitle(draft.title); setBody(draft.body); }
    })().catch(e => { if (live) setError(String(e)); }).finally(() => { if (live) setLoading(false); });
    return () => { live = false; };
  }, [fingerprint]);

  const run = (action: () => Promise<string>) => {
    setBusy(true); setError(""); setStatus("");
    void action().then(setStatus).catch(e => setError(String(e))).finally(() => setBusy(false));
  };
  const screenshot = (action: "copy" | "save") => run(async () => {
    const result = await withModalsHidden(() => issueReportScreenshot(action));
    return action === "copy" ? "Screenshot copied. Paste it into the issue on GitHub." : `Screenshot saved to ${result.path ?? "Downloads"}. Drag it into the issue on GitHub.`;
  });
  const open = () => run(async () => {
    const { truncated } = await openIssueReport(title, body);
    if (truncated) {
      await navigator.clipboard.writeText(body).catch(() => {});
      return "The report was too long for a GitHub link, so it was shortened. The full text is on your clipboard.";
    }
    return "GitHub opened in your browser. Review the issue there and click Submit to post it.";
  });

  return (
    <ModalShell className="issue-report-modal" cardClassName="issue-report-card" ariaLabelledBy="issue-report-title" onBackdropClick={busy ? undefined : onClose}
      onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); if (!busy) onClose(); } }}>
      <div className="ds-modal-title" id="issue-report-title">Report an issue</div>
      <div className="ds-modal-subtitle">
        This fills in a new issue for aiden0rchad/taucode on GitHub. Nothing is sent from taucode: the issue is only posted if you click Submit on GitHub.
      </div>
      <div className="issue-report-included">
        <strong>Included</strong>
        <ul>{ISSUE_REPORT_INCLUDES.map(item => <li key={item}>{item}</li>)}</ul>
        <p>Never included: API keys and other secrets, your home folder path, e-mail addresses, file contents, prompts or replies. Everything below is editable.</p>
      </div>
      <label className="ds-modal-label" htmlFor="issue-report-title-input">Title</label>
      <input id="issue-report-title-input" className="ds-modal-input" value={title} placeholder="Short summary of the problem" onChange={e => setTitle(e.target.value)} disabled={loading} />
      <label className="ds-modal-label" htmlFor="issue-report-body">Description</label>
      <textarea id="issue-report-body" className="ds-modal-textarea issue-report-body" value={loading ? "Preparing report…" : body} onChange={e => setBody(e.target.value)} disabled={loading} spellCheck />
      <div className="issue-report-screenshot">
        <span>GitHub links can't carry images. Capture taucode's window, then paste or drag it into the issue.</span>
        <div>
          <button type="button" className="ghost" disabled={busy} onClick={() => screenshot("copy")}><Camera size={14} />Copy screenshot</button>
          <button type="button" className="ghost" disabled={busy} onClick={() => screenshot("save")}><Download size={14} />Save screenshot</button>
        </div>
      </div>
      {status && <p className="issue-report-status" role="status">{status}</p>}
      {error && <p className="ds-modal-error" role="alert">{error}</p>}
      <div className="ds-modal-actions">
        <button type="button" className="ghost ds-modal-button" onClick={onClose} disabled={busy}>Close</button>
        <button type="button" className="primary ds-modal-button" onClick={open} disabled={busy || loading}><ExternalLink size={14} />Open on GitHub</button>
      </div>
    </ModalShell>
  );
}
