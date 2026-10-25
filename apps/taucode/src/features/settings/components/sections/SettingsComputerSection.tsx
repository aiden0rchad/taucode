import { SettingsFieldRow } from "@/features/design-system/components/settings/SettingsPrimitives";
import { SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEffect, useState } from "react";
import type { AppSettings } from "@/types";
import { computerRequest, computerRequestPermissions } from "@services/tauri";
import { defaultComputerSettings, type ComputerRun, type ComputerSettings } from "@/features/computer/computer";
import { modelKey, modelRef } from "@settings/registry";
import "@/features/computer/computer.css";
function allowedApps(entries: (string | ComputerSettings["allowedApps"][number])[]): ComputerSettings["allowedApps"] {
  return entries.map(entry => {
    const app = typeof entry === "string" ? { bundleId: entry, access: "full" as const } : entry;
    return { ...app, access: app.bundleId.trim().toLowerCase() === "com.apple.systempreferences" ? "read-only" : app.access };
  });
}
export function SettingsComputerSection({ appSettings, onUpdateAppSettings }: { appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void> }) {
  const [draft, setDraft] = useState({ ...defaultComputerSettings, ...appSettings.computer, allowedApps: allowedApps(appSettings.computer?.allowedApps ?? []) });
  const [status, setStatus] = useState<ComputerRun>({ steps: [] });
  const [busy, setBusy] = useState(false);
  const [requesting, setRequesting] = useState(false);
  const [error, setError] = useState("");
  const refresh = () => computerRequest<ComputerRun>({ action: "status" }).then(setStatus).catch(e => setError(String(e)));
  useEffect(() => { void refresh(); }, []);
  const update = (value: Partial<ComputerSettings>) => setDraft(s => ({ ...s, ...value }));
  const requestAccess = async (checkFirst = false) => {
    setRequesting(true); setError("");
    try {
      if (checkFirst) {
        const current = await computerRequest<ComputerRun>({ action: "status" });
        setStatus(current);
        if (current.permissions?.screen && current.permissions.accessibility) return;
      }
      const permissions = await computerRequestPermissions();
      setStatus(s => ({ ...s, permissions }));
      if (permissions.unsupported) throw new Error("Computer Use requires macOS");
      for (const pane of ["screen", "accessibility"] as const) {
        if (!permissions[pane]) await computerRequest({ action: "permissions", pane });
      }
    } catch (e) { setError(String(e)); } finally { setRequesting(false); }
  };
  const save = async () => {
    setBusy(true); setError("");
    try {
      if (draft.enabled && (!draft.planner || !draft.grounding)) throw new Error("Select planner and grounding models");
      if (draft.enabled) { setStatus(s => ({ ...s, setup: "Preparing Python 3.12 environment…" })); await computerRequest({ action: "prepare" }); }
      await onUpdateAppSettings({ ...appSettings, computer: { ...draft, allowedApps: allowedApps(draft.allowedApps.map(a => ({ ...a, bundleId: a.bundleId.trim() })).filter(a => a.bundleId)), deniedApps: draft.deniedApps.map(s => s.trim()).filter(Boolean) } });
      await refresh();
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const options = (appSettings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => <option key={`${p.id}/${m.id}`} value={`${p.id}/${m.id}`}>{p.name} / {m.displayName || m.id}</option>));
  return <section className="settings-section computer-settings">
    <h2>Computer Use</h2><p className="settings-subtitle">A vision planner proposes actions; taucode checks app policy and approvals. API keys and local providers only.</p>
    {error && <p role="alert">{error}</p>}
    <fieldset disabled={busy || requesting}><legend>Planner and safety</legend>
      <SettingsToggleRow  title={<><span> Enable Computer Use</span></>}><SettingsToggleSwitch pressed={draft.enabled} onClick={() => { update({ enabled: !(draft.enabled) }); if (!(draft.enabled)) void requestAccess(true); }} /></SettingsToggleRow>
      <SettingsFieldRow  title={<>Input mode</>}><select value={draft.mode} onChange={e => update({ mode: e.target.value as ComputerSettings["mode"] })}><option value="background">Background (recommended): you can keep using your Mac</option><option value="foreground">Foreground: takes over the mouse</option></select></SettingsFieldRow>
      <SettingsToggleRow  title={<><span> Allow foreground fallback (approval required for each step)</span></>}><SettingsToggleSwitch pressed={draft.allowForegroundFallback} onClick={() => update({ allowForegroundFallback: !(draft.allowForegroundFallback) })} /></SettingsToggleRow>
      <SettingsToggleRow  title={<><span> Show virtual cursor</span></>}><SettingsToggleSwitch pressed={draft.virtualCursor} onClick={() => update({ virtualCursor: !(draft.virtualCursor) })} /></SettingsToggleRow>
      {(["planner", "grounding"] as const).map(role => <SettingsFieldRow key={role} title={<>{role === "planner" ? "Vision planner model" : "Grounding model"}</>}><select value={modelKey(draft[role])} onChange={e => update({ [role]: modelRef(e.target.value) })}><option value="">Select a registry model</option>{options}</select></SettingsFieldRow>)}
      <SettingsFieldRow  title={<>Grounding mode</>}><select value={draft.groundingMode} onChange={e => update({ groundingMode: e.target.value as ComputerSettings["groundingMode"] })}><option value="elements">Accessibility elements (recommended)</option><option value="pixels">Vision model — pixel coordinates</option><option value="ui-tars">UI-TARS — preset coordinates</option></select></SettingsFieldRow>
      {draft.groundingMode === "ui-tars" && <SettingsFieldRow  title={<>Grounding coordinates</>}><select value={`${draft.groundingWidth}x${draft.groundingHeight}`} onChange={e => { const [groundingWidth, groundingHeight] = e.target.value.split("x").map(Number); update({ groundingWidth, groundingHeight }); }}><option value="1920x1080">UI-TARS-1.5-7B · 1920 × 1080</option><option value="1000x1000">UI-TARS-72B · 1000 × 1000</option></select></SettingsFieldRow>}
      <SettingsFieldRow  title={<>Typing confirmation</>}><select value={draft.confirmTyping} onChange={e => update({ confirmTyping: e.target.value as ComputerSettings["confirmTyping"] })}><option value="sensitive">Sensitive fields and risky actions</option><option value="always">Always</option><option value="risky-only">Risky only (allow Return in ordinary text fields)</option></select></SettingsFieldRow>
      {draft.confirmTyping === "risky-only" && <p role="alert">Return can submit or send in some apps. Risky-only mode allows it in ordinary text fields of full-access apps.</p>}
      <p>Sensitive mode allows typing in ordinary text fields of full-access apps. Secure or sensitive fields and Enter require approval. Enabled menu shortcuts use the menu title's risk policy; unresolved shortcuts outside the safe list require approval. Safe shortcuts: ⌘N, ⌘F, ⌘A, ⌘C, ⌘V, ⌘Z, ⌘⇧Z, arrows, Tab, Escape. ⌘S also requires approval, including saving a new document.</p>
      <label>Allowed app bundle IDs (one per line)<textarea value={draft.allowedApps.map(a => a.bundleId).join("\n")} onChange={e => update({ allowedApps: allowedApps(e.target.value.split("\n").map(bundleId => draft.allowedApps.find(a => a.bundleId === bundleId) ?? { bundleId, access: "full" })) })} /></label>
      {draft.allowedApps.map((app, index) => app.bundleId.trim() && <SettingsFieldRow key={index} title={<>Access for {app.bundleId}</>}><select disabled={app.bundleId.trim().toLowerCase() === "com.apple.systempreferences"} value={app.access} onChange={e => update({ allowedApps: draft.allowedApps.map((a, i) => i === index ? { ...a, access: e.target.value as typeof a.access } : a) })}><option value="full">Full</option><option value="read-only">Read-only</option></select></SettingsFieldRow>)}
      <p>Read-only apps allow navigation clicks, scrolling and opening the app. Typing, shortcuts, setting controls, risky clicks and dragging are blocked.</p>
      <label>Additional denied app bundle IDs (one per line)<textarea value={draft.deniedApps.join("\n")} onChange={e => update({ deniedApps: e.target.value.split("\n") })} /></label>
      <p>Password managers, Keychain, terminal apps and known banking apps are always blocked. The run allowlist can only narrow this list.</p>
      <p>System Settings is locked to read-only. Blocked panes: Privacy &amp; Security, Users &amp; Groups, Passwords, Login Password, Lock Screen, Touch ID &amp; Password, Wallet &amp; Apple Pay, Login Items &amp; Extensions, Sharing, Network, and Battery → Options.</p>
      <SettingsFieldRow  title={<>Maximum steps</>}><input type="number" min={1} max={500} value={draft.maxSteps} onChange={e => update({ maxSteps: Number(e.target.value) })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Maximum seconds</>}><input type="number" min={1} max={3600} value={draft.maxSeconds} onChange={e => update({ maxSeconds: Number(e.target.value) })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Stop shortcut</>}><input value={draft.shortcut} onChange={e => update({ shortcut: e.target.value })} /></SettingsFieldRow>
      <button className="primary" onClick={() => void save()}>{busy ? "Preparing…" : "Save Computer Use"}</button>
    </fieldset>
    <p role="status">{status.setup}</p>
    <h3>macOS permissions</h3>
    <button className="primary" disabled={requesting || busy} onClick={() => void requestAccess()}>{requesting ? "Requesting…" : "Request access"}</button>
    {(["screen", "accessibility"] as const).map(pane => <div className="settings-row" key={pane}><span>{pane === "screen" ? "Screen Recording" : "Accessibility"}: {status.permissions?.[pane] ? "Granted" : "Not granted"}</span><button onClick={() => void computerRequest({ action: "permissions", pane }).catch(e => setError(String(e)))}>Open System Settings — {pane === "screen" ? "Screen Recording" : "Accessibility"}</button></div>)}
    <button disabled={requesting} onClick={() => void refresh()}>Refresh permissions</button>
    <p>Screen Recording takes effect only after quitting and reopening taucode.</p>
    <button disabled={requesting || busy} onClick={() => void computerRequest({ action: "restart" }).catch(e => setError(String(e)))}>Quit &amp; Reopen</button>
  </section>;
}
