import { SettingsPage, SettingsGroup, SettingsRow, SettingsButton, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEffect, useState } from "react";
import type { ImportDetection, ImportEntry, ImportLog, ImportProgress, ImportedTranscript } from "@/types";
import { importRequest, pickImportExport } from "@services/tauri";
import { notifyImportsChanged } from "@services/events";
import "./registry.css";
import "./import.css";

const labels: Record<string, string> = { codex: "Codex", "codex-project": "Codex projects", claude: "Claude Code", "claude-cli": "Claude Code CLI", cowork: "Cowork", "codex-mcp": "Codex MCP servers", "claude-mcp": "Claude MCP servers", "claude.ai": "Claude chats", chatgpt: "ChatGPT chats" };
type HistorySelection = { logId: string; key: string; transcript: ImportedTranscript };

export function SettingsImportSection({ historyOnly = false }: { historyOnly?: boolean }) {
  const [detection, setDetection] = useState<ImportDetection | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [archived, setArchived] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ImportProgress>({ running: false, completed: 0, total: 0 });
  const [logs, setLogs] = useState<ImportLog[]>([]);
  const [summary, setSummary] = useState<ImportLog | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmation, setConfirmation] = useState<{ id: string; titles: string[] } | null>(null);
  const [history, setHistory] = useState<HistorySelection | null>(null);
  const refresh = async () => setLogs(await importRequest<ImportLog[]>("logs"));
  useEffect(() => {
    let active = true;
    void importRequest<ImportLog[]>("logs").then(v => { if (active) setLogs(v); }).catch(e => { if (active) setError(String(e)); });
    void importRequest<ImportProgress>("status").then(v => { if (active) setProgress(v); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (!busy && !progress.running) return;
    let active = true;
    const timer = window.setInterval(() => {
      void importRequest<ImportProgress>("status").then(v => {
        if (!active) return;
        setProgress(v);
        if (!v.running) { void refresh().catch(e => setError(String(e))); notifyImportsChanged(); }
      }).catch(e => { if (active) setError(String(e)); });
    }, 400);
    return () => { active = false; window.clearInterval(timer); };
  }, [busy, progress.running]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError(""); setNotice("");
    try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); setProgress(p => ({ ...p, running: false })); }
  };
  const detect = async (path?: string) => {
    const result = await importRequest<ImportDetection>(path ? "export" : "detect", path ? { path } : {});
    setDetection(result); setArchived(false); setSummary(null);
    setSelected(new Set(result.entries.filter(e => !e.archived && e.kind !== "skipped").map(e => e.key)));
  };
  const toggle = (entries: ImportEntry[], checked: boolean) => setSelected(previous => {
    const next = new Set(previous);
    for (const e of entries) { if (checked) next.add(e.key); else next.delete(e.key); }
    return next;
  });
  const undo = async (id: string, confirmChanged = false) => {
    const result = await importRequest<ImportLog | { needsConfirmation: string[] }>("undo", { logId: id, confirmChanged });
    if ("needsConfirmation" in result) { setConfirmation({ id, titles: result.needsConfirmation }); return; }
    setConfirmation(null); setHistory(null); setSummary(result); await refresh(); notifyImportsChanged();
  };
  const sources = [...new Set(detection?.entries.map(e => e.source))];
  const disabled = busy || progress.running;
  return <SettingsPage className="registry-settings import-settings" title={historyOnly ? "Imported Work history" : "Import"} subtitle="Bring conversations and projects into taucode."><SettingsGroup title="Preferences">

    {error && <p role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {!historyOnly && <>
      <SettingsRow className="import-actions"><SettingsButton disabled={disabled} onClick={() => void run(() => detect())}>Detect local sources</SettingsButton><SettingsButton disabled={disabled} onClick={() => void run(async () => { const path = await pickImportExport(); if (path) await detect(path); })}>Import chats from export file…</SettingsButton></SettingsRow>
      {detection && <>
        <SettingsToggleRow  title={<>Include archived</>}><SettingsToggleSwitch pressed={archived} disabled={disabled} onClick={() => { setArchived(!(archived)); toggle(detection.entries.filter(v => v.archived && v.kind !== "skipped"), !(archived)); }} /></SettingsToggleRow>
        {detection.warnings.map((warning, i) => <p key={i} role="status">{warning}</p>)}
        {sources.length === 0 && <p>No supported local history found.</p>}
        {sources.map(source => {
          const all = detection.entries.filter(e => e.source === source);
          const visible = all.filter(e => archived || !e.archived);
          const projects = [...new Set(visible.map(e => e.project || "Unfiled history"))];
          return <details className="import-source" key={source} open>
            <summary>{labels[source] ?? source}: {new Set(all.filter(e => e.cwd).map(e => e.cwd)).size} projects, {all.length} {source.includes("mcp") ? "servers" : source === "cowork" ? "tasks" : "items"}</summary>
            {projects.map(project => {
              const entries = visible.filter(e => (e.project || "Unfiled history") === project);
              const selectable = entries.filter(e => e.kind !== "skipped");
              return <details className="import-project" key={project} open>
                <summary>{project}</summary>
                <SettingsToggleRow  title={<>Select project</>}><SettingsToggleSwitch aria-label={`Select ${source} ${project}`} disabled={disabled || !selectable.length} pressed={selectable.length > 0 && selectable.every(e => selected.has(e.key))} onClick={() => toggle(selectable, !(selectable.length > 0 && selectable.every(e => selected.has(e.key))))} /></SettingsToggleRow>
                {entries.map(entry => <SettingsToggleRow className="import-entry" key={entry.key} title={<><span>{entry.title}{entry.archived && <small>Archived</small>}{entry.missingCwd && <small>Missing folder: {entry.cwd} — read-only history</small>}{entry.needsSecrets && <small>Needs secrets — configure in Connectors</small>}{entry.warning && <small>{entry.warning}</small>}</span></>}><SettingsToggleSwitch disabled={disabled || entry.kind === "skipped"} pressed={selected.has(entry.key)} onClick={() => toggle([entry], !(selected.has(entry.key)))} /></SettingsToggleRow>)}
              </details>;
            })}
          </details>;
        })}
        <SettingsButton disabled={disabled || !selected.size} onClick={() => void run(async () => { const log = await importRequest<ImportLog>("run", { keys: [...selected] }); setSummary(log); await refresh(); notifyImportsChanged(); })}>Import selected ({selected.size})</SettingsButton>
      </>}
    </>}
    {disabled && <div role="status"><p>{progress.phase === "detect" ? "Detecting sources…" : "Import operation in progress…"} {progress.completed}/{progress.total}</p><progress aria-label="Import progress" max={Math.max(1, progress.total)} value={progress.completed} /><SettingsButton onClick={() => void importRequest("cancel").catch(e => setError(String(e)))}>Cancel import</SettingsButton></div>}
    {summary && <div role="status"><h3>{summary.undone ? "Import undone" : summary.cancelled ? "Import cancelled" : "Import complete"}</h3><p>{summary.created.length} imported, {summary.skipped.length} skipped.</p>{[...summary.skipped, ...summary.warnings].map((message,i) => <p key={i}>{message}</p>)}</div>}
    {confirmation && <div role="alert"><p>These imported items were continued or changed: {confirmation.titles.join(", ")}. Undo will remove their imported files. Continuations created with Continue are kept.</p><SettingsButton disabled={disabled} onClick={() => void run(() => undo(confirmation.id, true))}>Remove changed imports</SettingsButton><SettingsButton onClick={() => setConfirmation(null)}>Keep imports</SettingsButton></div>}
    <h3>Import history</h3>
    {!logs.length && <p>No imports yet.</p>}
    {logs.filter(log => !historyOnly || log.created.some(e => e.kind === "work")).map(log => <details key={log.id} className="import-source"><summary>{log.id} · {log.created.length} items{log.undone ? " · undone" : ""}</summary>
      {!log.undone && <><SettingsButton disabled={disabled} onClick={() => void run(() => undo(log.id))}>Undo import</SettingsButton>{log.created.filter(e => !["connector", "project"].includes(e.kind) && (!historyOnly || e.kind === "work")).map(entry => <SettingsButton className="import-history-link" disabled={disabled} key={entry.key} onClick={() => void run(async () => setHistory({ logId: log.id, key: entry.key, transcript: await importRequest<ImportedTranscript>("read", { logId: log.id, key: entry.key }) }))}>{entry.title}</SettingsButton>)}</>}
      {[...log.skipped,...log.warnings].map((message,i) => <p key={i}>{message}</p>)}
    </details>)}
    {history && <article className="import-transcript" aria-label="Imported transcript"><h3>{history.transcript.title}</h3><p>Read-only imported snapshot · {history.transcript.source}</p>
      {history.transcript.kind === "work" ? <p>Available in Work mode with copied outputs and uploads. Open Work mode to continue as a new task.</p> : <SettingsButton disabled={disabled} onClick={() => void run(async () => { await importRequest("continue", { logId: history.logId, key: history.key }); notifyImportsChanged(); setNotice("Continuation created. Open Chat mode to continue in pi."); })}>Continue in pi</SettingsButton>}<SettingsButton onClick={() => setHistory(null)}>Close transcript</SettingsButton>
      {history.transcript.messages?.map((m,i) => <div key={i}><strong>{m.role}</strong><p>{m.text}</p></div>)}
      {history.transcript.raw?.map((entry,i) => <details key={i}><summary>Audit entry {i + 1}</summary><pre>{JSON.stringify(entry,null,2)}</pre></details>)}
      {!!history.transcript.attachments?.length && <><h4>Copied attachments</h4><ul>{history.transcript.attachments.map(name => <li key={name}>{name}</li>)}</ul></>}
    </article>}
  </SettingsGroup></SettingsPage>;
}
