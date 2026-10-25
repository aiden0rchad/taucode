import KeyRound from "lucide-react/dist/esm/icons/key-round";
import Cpu from "lucide-react/dist/esm/icons/cpu";
import Monitor from "lucide-react/dist/esm/icons/monitor";
import Accessibility from "lucide-react/dist/esm/icons/accessibility";
import Network from "lucide-react/dist/esm/icons/network";
import Activity from "lucide-react/dist/esm/icons/activity";
import ArrowUpRight from "lucide-react/dist/esm/icons/arrow-up-right";
import Download from "lucide-react/dist/esm/icons/download";
import LoaderCircle from "lucide-react/dist/esm/icons/loader-circle";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsButton, SettingsGroup, SettingsRow } from "@/features/design-system/components/settings/SettingsPrimitives";
import { engineRuntime, getAppSettings, getSetupStatus, openSetupPrivacy, providerHasApiKey, type EngineRuntimeStatus, type SetupStatus } from "@services/tauri";
import "./onboarding.css";

export const SETUP_SEEN = "taucode.setup-checklist.v1";
export type SetupSection = "providers" | "engines" | "features";

type ChipLabel = "Ready" | "Granted" | "On" | "Needs setup" | "Not granted" | "Optional" | "Off" | "Checking";
const chipTone: Record<ChipLabel, string> = {
  Ready: "success", Granted: "success", On: "success",
  "Needs setup": "warning", "Not granted": "warning",
  Optional: "muted", Off: "muted", Checking: "muted",
};
function SetupRow({ icon, title, description, label, error, children }: {
  icon: ReactNode; title: string; description: string; label: ChipLabel; error?: string; children?: ReactNode;
}) {
  return <SettingsRow className="setup-row" role="group" aria-label={title}>
    <div className="setup-row-info">
      <span className="setup-icon" aria-hidden="true">{icon}</span>
      <div className="setup-row-copy"><strong>{title}</strong><p title={description}>{description}</p></div>
    </div>
    <div className="settings-row-controls">
      <span role="status" className="setup-status" data-tone={error ? "error" : chipTone[label]} title={error} aria-label={error ? `Error: ${error}` : undefined}>
        {!error && label === "Checking" && <LoaderCircle size={12} className="setup-spinner" aria-hidden="true" />}
        {error ? "Error" : label}
      </span>
      {children}
    </div>
  </SettingsRow>;
}

export function SetupChecklist({ onNavigate }: { onNavigate: (section: SetupSection) => void }) {
  const [open, setOpen] = useState(() => import.meta.env.MODE === "uipreview" ? new URLSearchParams(window.location.search).get("setup") === "1" : localStorage.getItem(SETUP_SEEN) !== "done");
  const [status, setStatus] = useState<SetupStatus>();
  const [runtimes, setRuntimes] = useState<EngineRuntimeStatus[]>();
  const [provider, setProvider] = useState<boolean>();
  const [diagnostics, setDiagnostics] = useState<boolean>();
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<Partial<Record<"setup" | "engines" | "settings" | "screen" | "accessibility", string>>>({});
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
    setErrors(Object.fromEntries(results.map((result, index) => [["setup", "engines", "settings"][index], result.status === "rejected" ? String(result.reason) : undefined])));
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
  const pi = runtimes?.find(r => r.engine === "pi");
  const install = async () => {
    setBusy(true); setErrors(current => ({ ...current, engines: undefined }));
    try { setRuntimes(await engineRuntime("prepare", "pi")); }
    catch (e) { setErrors(current => ({ ...current, engines: String(e) })); }
    finally { setBusy(false); }
  };
  const engineError = errors.engines || (pi?.state === "error" ? pi.message : undefined);
  const essentialsReady = Number(provider === true && !errors.settings) + Number(pi?.state === "ready" && !busy && !engineError);
  return <ModalShell squircle ariaLabelledBy="setup-title" ariaDescribedBy="setup-description" cardClassName="setup-checklist-card squircle-clip" onBackdropClick={dismiss}>
    <div ref={body} className="settings-page setup-checklist">
      <h2 id="setup-title" className="ds-modal-title">Welcome to taucode</h2>
      <p id="setup-description" className="ds-modal-subtitle">Set up what you need now — everything else can wait. You can reopen this from Settings → About.</p>
      <div className="setup-progress">
        <span id="setup-progress-label">{essentialsReady} of 2 essentials ready</span>
        <progress aria-labelledby="setup-progress-label" max={2} value={essentialsReady} />
      </div>
      <SettingsGroup title="Essentials">
        <SetupRow icon={<KeyRound size={16} />} title="Providers" description="Connect a model provider." label={provider === undefined ? "Checking" : provider ? "Ready" : "Needs setup"} error={errors.settings}>
          <SettingsButton onClick={() => navigate("providers")}><ArrowUpRight size={14} aria-hidden="true" />Open Providers</SettingsButton>
        </SetupRow>
        <SetupRow icon={<Cpu size={16} />} title="Engines" description="Install pi to run conversations." label={busy || !runtimes ? "Checking" : pi?.state === "ready" ? "Ready" : "Needs setup"} error={engineError}>
          {pi?.state !== "ready" ? <SettingsButton disabled={busy} onClick={() => void install()}><Download size={14} aria-hidden="true" />Install pi</SettingsButton> : <SettingsButton onClick={() => navigate("engines")}><ArrowUpRight size={14} aria-hidden="true" />Manage engines</SettingsButton>}
        </SetupRow>
      </SettingsGroup>
      <SettingsGroup title="Computer use (optional)">
        {(["screen", "accessibility"] as const).map(pane => <SetupRow key={pane}
          icon={pane === "screen" ? <Monitor size={16} /> : <Accessibility size={16} />}
          title={pane === "screen" ? "Screen Recording" : "Accessibility"}
          description={status?.permissions.unsupported ? "Available on macOS." : pane === "screen" ? "Let taucode see your screen." : "Let taucode operate apps."}
          label={status?.permissions.unsupported ? "Optional" : !status ? "Checking" : status.permissions[pane] ? "Granted" : "Not granted"}
          error={errors[pane] || errors.setup}>
          <SettingsButton disabled={status?.permissions.unsupported} aria-label={`Open System Settings — ${pane === "screen" ? "Screen Recording" : "Accessibility"}`} onClick={() => void openSetupPrivacy(pane).catch(e => setErrors(current => ({ ...current, [pane]: String(e) })))}><ArrowUpRight size={14} aria-hidden="true" />System Settings</SettingsButton>
        </SetupRow>)}
      </SettingsGroup>
      <SettingsGroup title="Optional tools">
        <SetupRow icon={<Network size={16} />} title="Codegraph" description="Explore code relationships." label={!status ? "Checking" : status.codegraph ? "Ready" : "Optional"} error={errors.setup} />
        <SetupRow icon={<Activity size={16} />} title="Diagnostics" description="Inspect local app diagnostics." label={diagnostics === undefined ? "Checking" : diagnostics ? "On" : "Off"} error={errors.settings}>
          <SettingsButton onClick={() => navigate("features")}><ArrowUpRight size={14} aria-hidden="true" />Open Features</SettingsButton>
        </SetupRow>
      </SettingsGroup>
      <div className="ds-modal-actions"><SettingsButton className="ghost" onClick={dismiss}>Skip for now</SettingsButton><SettingsButton className="primary" onClick={dismiss}>Done</SettingsButton></div>
    </div>
  </ModalShell>;
}
