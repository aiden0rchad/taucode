import { useEffect, useState } from "react";
import type { AppSettings } from "@/types";
import { computerRequest } from "@services/tauri";
import { defaultComputerSettings, type ComputerRun, type ComputerSettings } from "@/features/computer/computer";
import { modelKey, modelRef } from "@settings/registry";
import "@/features/computer/computer.css";
export function SettingsComputerSection({ appSettings, onUpdateAppSettings }: { appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void> }) {
  const [draft, setDraft] = useState(appSettings.computer ?? defaultComputerSettings);
  const [status, setStatus] = useState<ComputerRun>({ steps: [] });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const refresh = () => computerRequest<ComputerRun>({ action: "status" }).then(setStatus).catch(e => setError(String(e)));
  useEffect(() => { void refresh(); }, []);
  const update = (value: Partial<ComputerSettings>) => setDraft(s => ({ ...s, ...value }));
  const save = async () => {
    setBusy(true); setError("");
    try {
      if (draft.enabled && (!draft.planner || !draft.grounding)) throw new Error("Select planner and grounding models");
      if (draft.enabled) { setStatus(s => ({ ...s, setup: "Preparing Python 3.12 environment…" })); await computerRequest({ action: "prepare" }); }
      await onUpdateAppSettings({ ...appSettings, computer: { ...draft, allowedApps: draft.allowedApps.map(s => s.trim()).filter(Boolean), deniedApps: draft.deniedApps.map(s => s.trim()).filter(Boolean) } });
      await refresh();
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const options = (appSettings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => <option key={`${p.id}/${m.id}`} value={`${p.id}/${m.id}`}>{p.name} / {m.displayName || m.id}</option>));
  return <section className="settings-section computer-settings">
    <h2>Computer Use</h2><p className="settings-subtitle">A vision planner proposes actions; taucode checks app policy and approvals. API keys and local providers only.</p>
    {error && <p role="alert">{error}</p>}
    <fieldset disabled={busy}><legend>Planner and safety</legend>
      <label><span><input type="checkbox" checked={draft.enabled} onChange={e => update({ enabled: e.target.checked })} /> Enable Computer Use</span></label>
      {(["planner", "grounding"] as const).map(role => <label key={role}>{role === "planner" ? "Vision planner model" : "Grounding model"}<select value={modelKey(draft[role])} onChange={e => update({ [role]: modelRef(e.target.value) })}><option value="">Select a registry model</option>{options}</select></label>)}
      <label>Grounding coordinates<select value={`${draft.groundingWidth}x${draft.groundingHeight}`} onChange={e => { const [groundingWidth, groundingHeight] = e.target.value.split("x").map(Number); update({ groundingWidth, groundingHeight }); }}><option value="1920x1080">UI-TARS-1.5-7B · 1920 × 1080</option><option value="1000x1000">UI-TARS-72B · 1000 × 1000</option></select></label>
      <label>Allowed app bundle IDs (one per line)<textarea value={draft.allowedApps.join("\n")} onChange={e => update({ allowedApps: e.target.value.split("\n") })} /></label>
      <label>Additional denied app bundle IDs (one per line)<textarea value={draft.deniedApps.join("\n")} onChange={e => update({ deniedApps: e.target.value.split("\n") })} /></label>
      <p>Password managers, Keychain, System Settings, terminal apps and known banking apps are always blocked. The run allowlist can only narrow this list.</p>
      <label>Maximum steps<input type="number" min={1} max={500} value={draft.maxSteps} onChange={e => update({ maxSteps: Number(e.target.value) })} /></label>
      <label>Maximum seconds<input type="number" min={1} max={3600} value={draft.maxSeconds} onChange={e => update({ maxSeconds: Number(e.target.value) })} /></label>
      <label>Stop shortcut<input value={draft.shortcut} onChange={e => update({ shortcut: e.target.value })} /></label>
      <button className="primary" onClick={() => void save()}>{busy ? "Preparing…" : "Save Computer Use"}</button>
    </fieldset>
    <p role="status">{status.setup}</p>
    <h3>macOS permissions</h3>
    {(["screen", "accessibility"] as const).map(pane => <div className="settings-row" key={pane}><span>{pane === "screen" ? "Screen Recording" : "Accessibility"}: {status.permissions?.[pane] ? "Granted" : "Not granted"}</span><button onClick={() => void computerRequest({ action: "permissions", pane }).catch(e => setError(String(e)))}>Open System Settings — {pane === "screen" ? "Screen Recording" : "Accessibility"}</button></div>)}
    <button onClick={() => void refresh()}>Refresh permissions</button>
  </section>;
}
