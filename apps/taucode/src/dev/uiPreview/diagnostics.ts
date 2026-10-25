import type { JournalIssue } from "@/features/diagnostics/DiagnosticsSettings";

export function createDiagnosticsFixture(empty: boolean) {
  const now = Date.now();
  const rows = [
    ["engine.spawn_failed", "Engine failed to start", "engine", "high", "open"],
    ["ipc.slow", "Workspace request took longer than expected", "ipc", "medium", "open"],
    ["ui.render_crash", "Sidebar panel could not render", "ui", "critical", "acknowledged"],
    ["provider.rate_limited", "Provider temporarily rate limited", "provider", "medium", "fixed"],
    ["git.refresh_failed", "Repository status refresh failed", "git", "low", "ignored"],
    ["browser.navigate_failed", "Browser navigation did not complete", "browser", "high", "open"],
  ];
  let issues: JournalIssue[] = empty ? [] : rows.map(([code, title, area, severity, status], index) => {
    const ts = new Date(now - (index * 47 + 2) * 60000).toISOString();
    return { fingerprint: "preview-" + index, code, title, area, severity, status, count: [4, 12, 1, 3, 2, 5][index], firstSeen: new Date(now - 86400000 * (index + 1)).toISOString(), lastSeen: ts,
      lastEvent: { ts, code, message: title, detail: { panel: area === "ui" ? "sidebar" : "conversation", recovery: "Retry the operation after checking configuration." } },
      engines: ["pi"], models: ["fixture-model"], regressed: index === 0, notes: index === 2 ? ["Acknowledged: checking the sidebar render path against the repository."] : [] };
  });
  let lastDigest: string | null = empty ? null : new Date(now - 60000).toISOString();
  return (action: string, params: Record<string, unknown> = {}) => {
    if (action === "info") return { ...__DIAGNOSTICS_PREVIEW_PATHS__, binaryError: "Build the diagnostics binary with cargo build --offline --bins in apps/taucode/src-tauri.", openIssues: issues.filter(i => i.status === "open").length, eventsToday: issues.reduce((n, i) => n + i.count, 0), dropped: 0, lastDigest };
    if (action === "diag_issues") return issues.filter(i => (!params.status || i.status === params.status) && (!params.area || i.area === params.area) && (!params.severity || i.severity === params.severity)).slice(0, Number(params.limit ?? 100));
    if (action === "diag_issue") { const issue = issues.find(i => i.fingerprint === params.fingerprint); if (!issue) throw new Error("Unknown issue"); return { issue, events: [issue.lastEvent] }; }
    if (action === "diag_events") return issues.map(i => i.lastEvent);
    if (action === "diag_feedback") return [];
    if (action === "diag_annotate") {
      const issue = issues.find(i => i.fingerprint === params.fingerprint);
      if (!issue) throw new Error("Unknown issue");
      if (typeof params.status === "string") issue.status = params.status;
      if (params.status === "fixed") issue.regressed = false;
      if (typeof params.note === "string" && params.note) issue.notes.push(params.note);
      return { ok: true };
    }
    if (action === "clear" && params.confirmed === true) { issues = []; lastDigest = null; }
    if (action === "regenerate") lastDigest = new Date().toISOString();
    if (action === "diag_digest" || action === "regenerate") return "# Diagnostics journal";
    return null;
  };
}
