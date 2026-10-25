import { SettingsGroup, SettingsRow, SettingsButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { CheckCircle2, CircleAlert, LoaderCircle, CircleHelp } from "lucide-react";
import { useEffect, useState } from "react";
import { engineRuntime, type EngineRuntimeStatus } from "@services/tauri";

export function SettingsRuntimeSetup({ engine, onManage }: { engine?: string; onManage?: () => void }) {
  const [runtimes, setRuntimes] = useState<EngineRuntimeStatus[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    const refresh = async () => {
      try { const value = await engineRuntime("status"); if (active) setRuntimes(value); }
      catch (e) { if (active) setError(String(e)); }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), 1000);
    return () => { active = false; clearInterval(timer); };
  }, []);
  const prepare = async (engine: string) => {
    setBusy(engine); setError("");
    try { setRuntimes(await engineRuntime("prepare", engine)); }
    catch (e) { setError(String(e)); }
    finally { setBusy(null); }
  };
  return <SettingsGroup title={engine ? undefined : "Engine runtimes"} subtitle={engine ? undefined : "Dependencies install into app data on first use. Node ≥22.19 and npm are required for pi, Claude Code (ACP), and browser tools; uv is required for Computer Use and Work document tools."}>
    {(engine ? runtimes.filter(runtime => runtime.engine === engine) : runtimes).map(runtime => {
      const installing = busy === runtime.engine || ["preparing", "installing"].includes(runtime.state);
      const status = installing ? "Installing" : runtime.state === "ready" ? "Ready" : runtime.state === "error" ? "Error" : "Missing";
      const Icon = installing ? LoaderCircle : status === "Ready" ? CheckCircle2 : status === "Error" ? CircleAlert : CircleHelp;
      return <SettingsRow className="settings-runtime-row" key={runtime.engine} aria-busy={installing}>
        <div className="settings-runtime-summary"><strong>{runtime.engine === "claude" ? "Claude Code" : runtime.engine}</strong><span aria-hidden>·</span><span className="settings-status-pill" data-status={status} role="status" title={runtime.message}><Icon size={13} aria-hidden />{status}</span>{import.meta.env.MODE === "uipreview" && <small>(mock)</small>}</div>
        {engine ? <SettingsButton className="ghost" onClick={onManage}>Manage</SettingsButton> : <SettingsButton disabled={busy !== null || installing} onClick={() => void prepare(runtime.engine)}>{busy === runtime.engine ? "Preparing…" : `Set up ${runtime.engine}`}</SettingsButton>}
        {!engine && <small className="settings-runtime-help">{runtime.message.replace(/^Mock /i, "")}</small>}
        {!engine && runtime.engine === "claude" && <p className="settings-callout"><CircleAlert size={16} aria-hidden />Some Anthropic-compatible gateways reject Claude Code's <code>anthropic-beta</code> headers. Allow those headers on the gateway or use a different model route.</p>}
      </SettingsRow>;
    })}
    {engine && !runtimes.some(runtime => runtime.engine === engine) && <SettingsRow className="settings-runtime-row"><span>{engine} · Checking runtime…</span><SettingsButton className="ghost" onClick={onManage}>Manage</SettingsButton></SettingsRow>}
    {error && <p role="alert">{error}</p>}
  </SettingsGroup>;
}
