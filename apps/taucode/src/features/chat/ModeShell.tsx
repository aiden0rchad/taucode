import { SetupWizard } from "@/features/onboarding/SetupWizard";
import type { SetupSection } from "@/features/onboarding/setupState";
import { EvalsView } from "@/features/evals/EvalsView";
import { FeedbackComposer, DiagnosticsCommands } from "@/features/diagnostics/FeedbackComposer";
import { diagnosticContext, record } from "@/features/diagnostics/client";
import { FeatureSettings, features, useFeatures } from "@settings/features";
import { useSidebarDensity } from "@settings/hooks/useSidebarDensity";
import { SidebarEngine } from "@app/components/SidebarTree";
import { createContext, useContext, useEffect, useState, type ReactNode, type Dispatch, type SetStateAction } from "react";
import type { AppMode, AppSettings } from "@/types";
import { getAppSettings, updateAppSettings, setWindowMode } from "@services/tauri";
import { useTauriEvent } from "@app/hooks/useTauriEvent";
import { useSidebarState } from "@/features/layout/hooks/useSidebarState";
import { subscribeMenuToggleProjectsSidebar, notifySetupChanged } from "@services/events";
import { matchesShortcut } from "@utils/shortcuts";
import { useWindowLabel } from "@/features/layout/hooks/useWindowLabel";
import { useThemePreference } from "@/features/layout/hooks/useThemePreference";
import { useUiScaleShortcuts } from "@/features/layout/hooks/useUiScaleShortcuts";
import { DEFAULT_THEME_ID } from "@/features/themes/defaults";
import { WorkView } from "@/features/work/WorkView";
import { ChatView } from "./ChatView";
import "./chat.css";

const modes: AppMode[] = ["code", "chat", "work"];
const ModeContext = createContext<{ mode: AppMode; select: (mode: AppMode) => void; updateSettings: (settings: AppSettings) => void } | null>(null);

function ModeScale({ settings, setSettings }: { settings: AppSettings; setSettings: Dispatch<SetStateAction<AppSettings | null>> }) {
  useUiScaleShortcuts({ settings, setSettings: update => setSettings(current => current ? typeof update === "function" ? update(current) : update : current), saveSettings: updateAppSettings });
  return null;
}

export function ModeSwitcher() {
  const enabled = useFeatures();
  const context = useContext(ModeContext);
  if (!context) return null;
  return <nav className="mode-switcher" aria-label="App mode">
    {modes.filter(mode => mode === "code" || (mode === "chat" ? enabled.chatMode : enabled.workMode)).map(mode => <button type="button" key={mode} aria-pressed={context.mode === mode} onClick={() => context.select(mode)}>{mode[0].toUpperCase() + mode.slice(1)}</button>)}
  </nav>;
}

export function useModeSettings(settings: AppSettings | null) {
  const update = useContext(ModeContext)?.updateSettings;
  useEffect(() => { if (settings) update?.(settings); }, [settings, update]);
  return update;
}

export function modeShortcut(event: KeyboardEvent, settings: AppSettings): AppMode | undefined {
  if (!event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || !/^[123]$/.test(event.key)) return;
  if (Object.entries(settings).some(([key, value]) => key.endsWith("Shortcut") && typeof value === "string" && matchesShortcut(event, value))) return;
  const next = modes[Number(event.key) - 1];
  const enabled = features(settings);
  return next === "code" || (next === "chat" ? enabled.chatMode : enabled.workMode) ? next : undefined;
}

export function ModeShell({ children, renderSettings }: { children: ReactNode; renderSettings?: (onClose: () => void, section?: SetupSection) => ReactNode }) {
  useSidebarDensity();
  const label = useWindowLabel();
  const [mode, setMode] = useState<AppMode>("code");
  const [settings, setSettings] = useState<AppSettings | null>(null);
  useEffect(() => { diagnosticContext({ mode }); if (settings?.features?.diagnostics) record({ kind: "ux", area: "ui", code: "ui.mode_switch", message: "Mode selected", context: { mode } }); }, [mode, settings]);
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  useTauriEvent(subscribeMenuToggleProjectsSidebar, () => setSidebarCollapsed(!sidebarCollapsed), { enabled: mode !== "code" && !settingsOpen });
  const [setupSection, setSetupSection] = useState<SetupSection>();
  const [setupWorkspace, setSetupWorkspace] = useState<string>();
  useEffect(() => {
    if (mode === "code" && setupWorkspace) { notifySetupChanged(setupWorkspace); setSetupWorkspace(undefined); }
  }, [mode, setupWorkspace]);
  const [evalsFull, setEvalsFull] = useState<{runId?:string}|null>(null);
  useEffect(() => { const open = (event:Event) => setEvalsFull({runId:(event as CustomEvent<{runId?:string}>).detail?.runId}); window.addEventListener("taucode:evals-full", open); return () => window.removeEventListener("taucode:evals-full", open); }, []);
  useThemePreference(settings ?? { theme: DEFAULT_THEME_ID }, () => {
    if (!settings) return;
    const next = { ...settings, theme: "system" };
    setSettings(next);
    void updateAppSettings(next).catch(e => setError(String(e)));
  }, loaded);
  useEffect(() => {
    const open = () => setSettingsOpen(true);
    window.addEventListener("taucode:provider-settings", open);
    return () => window.removeEventListener("taucode:provider-settings", open);
  }, []);
  useEffect(() => {
    let cancelled = false;
    void getAppSettings().then(value => {
      if (cancelled) return;
      setSettings(value);
      const saved = value.windowModes?.[label];
      setMode(saved && modes.includes(saved) ? saved : "code");
      setLoaded(true);
    }).catch(e => { if (!cancelled) { setError(String(e)); setLoaded(true); } });
    return () => { cancelled = true; };
  }, [label]);
  const select = async (next: AppMode) => {
    try { await setWindowMode(next); setMode(next); setError(""); }
    catch (e) { setError(String(e)); }
  };
  useEffect(() => {
    if (!settings) return;
    const keydown = (event: KeyboardEvent) => {
      const next = modeShortcut(event, settings);
      if (next && !event.defaultPrevented) { event.preventDefault(); void setWindowMode(next).then(() => setMode(next)).catch(e => setError(String(e))); }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [settings]);
  useEffect(() => { const enabled = features(settings); if ((mode === "chat" && !enabled.chatMode) || (mode === "work" && !enabled.workMode)) { setMode("code"); void setWindowMode("code").catch(e => setError(String(e))); } }, [mode, settings]);
  if (!loaded) return null;
  return <FeatureSettings.Provider value={settings}>{settings?.features?.diagnostics && <><FeedbackComposer /><DiagnosticsCommands /></>}<ModeContext.Provider value={{ mode, select: next => { void select(next); }, updateSettings: setSettings }}>
    {mode !== "code" && !settingsOpen && settings && <ModeScale settings={settings} setSettings={setSettings} />}
    {error && <p role="alert" className="chat-mode-error">{error}</p>}
    {evalsFull && settings && <main className="evals-full"><div className="evals-windowbar" data-tauri-drag-region aria-hidden="true" /><EvalsView full initialRunId={evalsFull.runId} appSettings={settings} onUpdateAppSettings={async next => { const saved = await updateAppSettings(next); setSettings(saved); return saved; }} onClose={() => setEvalsFull(null)} /></main>}
    <div style={{ display: evalsFull ? "none" : "contents" }}>
    {mode === "code" && <SidebarEngine.Provider value={settings?.agentPolicy?.defaultEngine ?? "pi"}>{children}</SidebarEngine.Provider>}
    {mode === "chat" && settings && <ChatView settings={settings} onOpenSettings={() => setSettingsOpen(true)} />}
    {mode === "work" && settings && <WorkView settings={settings} onOpenSettings={() => setSettingsOpen(true)} />}
    </div>
    {settingsOpen && renderSettings?.(() => { setSettingsOpen(false); setSetupSection(undefined); void getAppSettings().then(setSettings).catch(e => setError(String(e))); }, setupSection)}
    <SetupWizard onSettings={setSettings} onFinish={async (next, workspaceId) => { await setWindowMode(next); setMode(next); setSetupWorkspace(workspaceId); setSettingsOpen(false); }} />
  </ModeContext.Provider></FeatureSettings.Provider>;
}
