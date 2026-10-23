import { useState } from "react";
import type { AppSettings } from "@/types";
import { browserRequest } from "@services/tauri";
import { defaultBrowserSettings } from "@/features/browser/browser";
import "./registry.css";

export function SettingsBrowserSection({ appSettings, onUpdateAppSettings }: { appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void> }) {
  const [draft, setDraft] = useState({ ...defaultBrowserSettings, ...appSettings.browser });
  const [allow, setAllow] = useState(draft.allowedOrigins.join("\n"));
  const [block, setBlock] = useState(draft.blockedOrigins.join("\n"));
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const run = async (action: () => Promise<void>) => { setBusy(true); setError(""); setMessage(""); try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); } };
  return <section className="settings-section registry-settings">
    <h2>Browser</h2><p className="settings-subtitle">A shared browser for you, Pi and Codex. Website sign-ins stay in the local browser profile. Toggle the pane with ⌘/Ctrl Shift B.</p>
    <form onSubmit={e => { e.preventDefault(); void run(async () => {
      const url = new URL(draft.searchUrl.split("{query}").join("test"));
      if (!["http:","https:"].includes(url.protocol) || !draft.searchUrl.includes("{query}")) throw new Error("Search URL must use HTTP(S) and contain {query}");
      const lines = (v: string) => v.split("\n").map(s => s.trim()).filter(Boolean);
      await onUpdateAppSettings({ ...appSettings, browser: { ...draft, allowedOrigins: lines(allow), blockedOrigins: lines(block) } });
      setMessage("Saved. Browser binary changes apply on next browser start; origin and approval changes apply on next engine start.");
    }); }}>
      <fieldset disabled={busy} className="connector-fields"><legend>Browser preferences</legend>
        <label>Browser binary<input value={draft.binary ?? ""} placeholder="Auto-detect Chrome, Edge, Chromium" onChange={e => setDraft({ ...draft, binary: e.target.value || null })} /></label>
        <label>Allowed origins (one per line)<textarea value={allow} onChange={e => setAllow(e.target.value)} /></label>
        <label>Blocked origins (one per line)<textarea value={block} onChange={e => setBlock(e.target.value)} /></label>
        <label>Browser tool approval<select value={draft.approval} onChange={e => setDraft({ ...draft, approval: e.target.value as "ask" | "auto" })}><option value="ask">Ask before changes</option><option value="auto">Automatic</option></select></label>
        <p className="settings-subtitle">Snapshots, screenshots, console, network, tabs and navigation never ask. Clicks, typing, scripts and other changes follow this setting.</p>
        <label>Search URL<input value={draft.searchUrl} onChange={e => setDraft({ ...draft, searchUrl: e.target.value })} /></label>
        <p className="settings-subtitle">Use {"{query}"} for the search term. Downloads are saved to ~/Downloads.</p>
        <button type="submit">Save browser settings</button>
      </fieldset>
    </form>
    <button disabled={busy} onClick={() => setConfirm(true)}>Clear browsing data</button>
    {confirm && <div role="alert"><p>Stop the browser and delete all browsing data, including website sign-ins?</p><button disabled={busy} onClick={() => void run(async () => { await browserRequest({ action: "clear", confirmed: true }); setConfirm(false); setMessage("Browsing data cleared."); })}>Delete browsing data</button><button disabled={busy} onClick={() => setConfirm(false)}>Cancel</button></div>}
    {error && <p role="alert">{error}</p>}{message && <p role="status">{message}</p>}
  </section>;
}
