import { useCallback, useEffect, useRef, useState } from "react";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsButton, SettingsGroup, SettingsRow } from "@/features/design-system/components/settings/SettingsPrimitives";
import { engineRuntime, getAppSettings, getSetupStatus, openSetupPrivacy, providerHasApiKey, type EngineRuntimeStatus, type SetupStatus } from "@services/tauri";
import "./onboarding.css";

export const SETUP_SEEN = "taucode.setup-checklist.v1";
export type SetupSection = "providers" | "engines" | "features";

export function SetupChecklist({ onNavigate }: { onNavigate: (section: SetupSection) => void }) {
  const [open, setOpen] = useState(() => import.meta.env.MODE !== "uipreview" && localStorage.getItem(SETUP_SEEN) !== "done");
  const [status, setStatus] = useState<SetupStatus>();
  const [runtimes, setRuntimes] = useState<EngineRuntimeStatus[]>([]);
  const [provider, setProvider] = useState<boolean>();
  const [diagnostics, setDiagnostics] = useState<boolean>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const body = useRef<HTMLDivElement>(null);
  const refresh = useCallback(async () => {
    const results = await Promise.allSettled([
      getSetupStatus().then(setStatus),
      engineRuntime("status").then(setRuntimes),
      getAppSettings().then(async settings => {
        setDiagnostics(settings.features?.diagnostics ?? false);
        const enabled = (settings.providers ?? []).filter(p => p.enabled);
        if (enabled.some(p => p.auth.type === "none")) { setProvider(true); return; }
        const ready = await Promise.allSettled(enabled.map(p => providerHasApiKey(p.id)));
        if (ready.some(r => r.status === "fulfilled" && r.value)) setProvider(true);
        else if (ready.some(r => r.status === "rejected")) throw new Error("Could not check provider credentials. Open Providers to review them.");
        else setProvider(false);
      }),
    ]);
    const failed = results.find(r => r.status === "rejected");
    setError(failed?.status === "rejected" ? String(failed.reason) : "");
  }, []);
  const dismiss = useCallback(() => {
    localStorage.setItem(SETUP_SEEN, "done");
    setOpen(false);
  }, []);
  useEffect(() => {
    const show = () => setOpen(true);
    window.addEventListener("taucode:setup-checklist", show);
    return () => window.removeEventListener("taucode:setup-checklist", show);
  }, []);
  useEffect(() => {
    if (!open) return;
    void refresh();
    const focus = () => { void refresh(); };
    window.addEventListener("focus", focus);
    const previous = document.activeElement as HTMLElement | null;
    body.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); dismiss(); }
      if (event.key !== "Tab") return;
      const buttons = [...(body.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
      const first = buttons[0], last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    };
    document.addEventListener("keydown", keydown, true);
    return () => { window.removeEventListener("focus", focus); document.removeEventListener("keydown", keydown, true); previous?.focus(); };
  }, [open, refresh, dismiss]);
  if (!open) return null;
  const navigate = (section: SetupSection) => { dismiss(); onNavigate(section); };
  const pi = runtimes.find(r => r.engine === "pi");
  const install = async () => {
    setBusy(true); setError("");
    try { setRuntimes(await engineRuntime("prepare", "pi")); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  const stateLabel = (value: boolean | undefined) => value === undefined ? "Checking…" : value ? "Ready" : "Not set up";
  return <ModalShell ariaLabelledBy="setup-title" ariaDescribedBy="setup-description" cardClassName="setup-checklist-card" onBackdropClick={dismiss}>
    <div ref={body}>
      <h2 id="setup-title" className="ds-modal-title">Welcome to taucode</h2>
      <p id="setup-description" className="ds-modal-subtitle">Set up what you need. Everything here can wait, and you can return from Settings → About.</p>
      <SettingsGroup title="Start a conversation">
        <SettingsRow><div><strong>Providers</strong><p role="status">{stateLabel(provider)}</p></div><SettingsButton onClick={() => navigate("providers")}>Open Providers</SettingsButton></SettingsRow>
        <SettingsRow><div><strong>Engines</strong><p role="status">pi: {busy ? "Installing…" : pi?.message ?? "Checking…"}</p><p className="ds-modal-subtitle">Other engines are optional.</p></div>{pi?.state !== "ready" ? <SettingsButton disabled={busy} onClick={() => void install()}>Install pi</SettingsButton> : <SettingsButton onClick={() => navigate("engines")}>Manage engines</SettingsButton>}</SettingsRow>
      </SettingsGroup>
      <SettingsGroup title="Computer use · optional" subtitle="Only needed when you want taucode to see and operate apps.">
        {(["screen", "accessibility"] as const).map(pane => <SettingsRow key={pane}><div><strong>{pane === "screen" ? "Screen Recording" : "Accessibility"}</strong><p role="status">{status?.permissions.unsupported ? "macOS only" : status?.permissions[pane] === undefined ? "Checking…" : status.permissions[pane] ? "Granted" : "Not granted"}</p></div><SettingsButton disabled={status?.permissions.unsupported} aria-label={`Open System Settings — ${pane === "screen" ? "Screen Recording" : "Accessibility"}`} onClick={() => void openSetupPrivacy(pane).catch(e => setError(String(e)))}>Open System Settings</SettingsButton></SettingsRow>)}
      </SettingsGroup>
      <SettingsGroup title="Optional tools">
        <SettingsRow><div><strong>Codegraph</strong><p role="status">{status ? status.codegraph ? "Installed" : "Not installed" : "Checking…"}</p></div></SettingsRow>
        <SettingsRow><div><strong>Diagnostics</strong><p role="status">{diagnostics === undefined ? "Checking…" : diagnostics ? "On" : "Off"}</p></div><SettingsButton onClick={() => navigate("features")}>Open Features</SettingsButton></SettingsRow>
      </SettingsGroup>
      {error && <p role="alert">{error}</p>}
      <div className="ds-modal-actions"><SettingsButton onClick={dismiss}>Skip for now</SettingsButton><SettingsButton className="primary" onClick={dismiss}>Done</SettingsButton></div>
    </div>
  </ModalShell>;
}
