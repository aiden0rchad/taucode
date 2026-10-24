import { useEffect, useState } from "react";
import { engineRuntime, type EngineRuntimeStatus } from "@services/tauri";

export function SettingsRuntimeSetup() {
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
  return <section className="settings-section">
    <h3>Engine runtimes</h3>
    <p className="settings-subtitle">Dependencies install into app data on first use. Node ≥22.19 and npm are required for pi, Claude Code (ACP), and browser tools; uv is required for Computer Use.</p>
    {runtimes.map(runtime => <div className="settings-row" key={runtime.engine} aria-busy={busy === runtime.engine || ["preparing", "installing"].includes(runtime.state)}>
      <span>{runtime.engine}</span><span role="status">{runtime.message}</span>
      <button type="button" disabled={busy !== null || ["preparing", "installing"].includes(runtime.state)} onClick={() => void prepare(runtime.engine)}>{busy === runtime.engine ? "Preparing…" : `Set up ${runtime.engine}`}</button>
    </div>)}
    {error && <p role="alert">{error}</p>}
  </section>;
}
