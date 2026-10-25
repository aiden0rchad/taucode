import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import type { AppMode, AppSettings } from "@/types";
import { getAppSettings, setWindowMode } from "@services/tauri";
import { matchesShortcut } from "@utils/shortcuts";
import { useWindowLabel } from "@/features/layout/hooks/useWindowLabel";
import { useThemePreference } from "@/features/layout/hooks/useThemePreference";
import { WorkView } from "@/features/work/WorkView";
import { ChatView } from "./ChatView";
import "./chat.css";

const modes: AppMode[] = ["code", "chat", "work"];
const ModeContext = createContext<{ mode: AppMode; select: (mode: AppMode) => void; updateSettings: (settings: AppSettings) => void } | null>(null);

export function ModeSwitcher() {
  const context = useContext(ModeContext);
  if (!context) return null;
  return <nav className="mode-switcher" aria-label="App mode">
    {modes.map(mode => <button type="button" key={mode} aria-pressed={context.mode === mode} onClick={() => context.select(mode)}>{mode[0].toUpperCase() + mode.slice(1)}</button>)}
  </nav>;
}

export function useModeSettings(settings: AppSettings) {
  const update = useContext(ModeContext)?.updateSettings;
  useEffect(() => { update?.(settings); }, [settings, update]);
  return update;
}

export function modeShortcut(event: KeyboardEvent, settings: AppSettings): AppMode | undefined {
  if (!event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || !/^[123]$/.test(event.key)) return;
  if (Object.entries(settings).some(([key, value]) => key.endsWith("Shortcut") && typeof value === "string" && matchesShortcut(event, value))) return;
  return modes[Number(event.key) - 1];
}

export function ModeShell({ children, renderSettings }: { children: ReactNode; renderSettings?: (onClose: () => void) => ReactNode }) {
  const label = useWindowLabel();
  const [mode, setMode] = useState<AppMode>("code");
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  useThemePreference(settings?.theme ?? "system");
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
  if (!loaded) return null;
  return <ModeContext.Provider value={{ mode, select: next => { void select(next); }, updateSettings: setSettings }}>
    {error && <p role="alert" className="chat-mode-error">{error}</p>}
    {mode === "code" && children}
    {mode === "chat" && settings && <ChatView settings={settings} onOpenSettings={() => setSettingsOpen(true)} />}
    {mode === "work" && settings && <WorkView settings={settings} onOpenSettings={() => setSettingsOpen(true)} />}
    {settingsOpen && renderSettings?.(() => { setSettingsOpen(false); void getAppSettings().then(setSettings).catch(e => setError(String(e))); })}
  </ModeContext.Provider>;
}
