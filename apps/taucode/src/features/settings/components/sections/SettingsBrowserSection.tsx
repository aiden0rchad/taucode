import { SettingsPage, SettingsGroup, SettingsTextField, SettingsTextArea, SettingsSelect, SettingsButton, SettingsFieldRow } from "@/features/design-system/components/settings/SettingsPrimitives";
import { SettingsRuntimeSetup } from "./SettingsRuntimeSetup";
import { useState } from "react";
import type { AppSettings } from "@/types";
import { browserRequest } from "@services/tauri";
import { defaultBrowserSettings } from "@/features/browser/browser";
import "./registry.css";

export function SettingsBrowserSection({ appSettings, onUpdateAppSettings, onManageEngines }: { onManageEngines?: () => void; appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void> }) {
  const [draft, setDraft] = useState({ ...defaultBrowserSettings, ...appSettings.browser });
  const [allow, setAllow] = useState(draft.allowedOrigins.join("\n"));
  const [block, setBlock] = useState(draft.blockedOrigins.join("\n"));
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const run = async (action: () => Promise<void>) => { setBusy(true); setError(""); setMessage(""); try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); } };
  return <SettingsPage className="registry-settings" title="Browser" subtitle="A shared browser for you and your agents.">{draft.engine === "cdp" && <SettingsRuntimeSetup engine="browser" onManage={onManageEngines} />}

    <form onSubmit={e => { e.preventDefault(); void run(async () => {
      const url = new URL(draft.searchUrl.split("{query}").join("test"));
      if (!["http:","https:"].includes(url.protocol) || !draft.searchUrl.includes("{query}")) throw new Error("Search URL must use HTTP(S) and contain {query}");
      const lines = (v: string) => v.split("\n").map(s => s.trim()).filter(Boolean);
      await onUpdateAppSettings({ ...appSettings, browser: { ...draft, allowedOrigins: lines(allow), blockedOrigins: lines(block) } });
      setMessage("Saved. Engine and private browsing changes apply immediately. Browser binary and origin changes apply to new browser sessions.");
    }); }}>
      <SettingsGroup disabled={busy} title={<>Browser preferences</>}>
        <SettingsFieldRow title={<>Engine</>} help={<>Native shares the embedded page with agents. Headless Chromium supports sites that require trusted input. Linux uses Chromium; Windows native agent tools use Chromium.</>}><SettingsSelect value={draft.engine ?? "native"} onChange={e => setDraft({ ...draft, engine: e.target.value as "native" | "cdp" })}><option value="native">Native</option><option value="cdp">Headless Chromium (advanced)</option></SettingsSelect></SettingsFieldRow>
        <SettingsFieldRow title={<>Private browsing</>} help={<>Use an ephemeral native browser store. Changing this restarts open native tabs.</>}><SettingsSelect value={String(draft.privateBrowsing ?? false)} onChange={e => setDraft({ ...draft, privateBrowsing: e.target.value === "true" })}><option value="false">Off</option><option value="true">On</option></SettingsSelect></SettingsFieldRow>
        <SettingsFieldRow  title={<>Browser binary</>}><SettingsTextField value={draft.binary ?? ""} placeholder="Auto-detect Chrome, Edge, Chromium" onChange={e => setDraft({ ...draft, binary: e.target.value || null })} /></SettingsFieldRow>
        <SettingsFieldRow title={<>Allowed origins (one per line)</>} className="settings-field-row-long"><SettingsTextArea value={allow} onChange={e => setAllow(e.target.value)} /></SettingsFieldRow>
        <SettingsFieldRow title={<>Blocked origins (one per line)</>} className="settings-field-row-long"><SettingsTextArea value={block} onChange={e => setBlock(e.target.value)} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Browser tool approval</>} help={<>Snapshots, screenshots, console, network, tabs and navigation never ask. Clicks, typing, scripts and other changes follow this setting.</>}><SettingsSelect value={draft.approval} onChange={e => setDraft({ ...draft, approval: e.target.value as "ask" | "auto" })}><option value="ask">Ask before changes</option><option value="auto">Automatic</option></SettingsSelect></SettingsFieldRow>
        <SettingsFieldRow  title={<>Search URL</>} help={<>Use {"{query}"} for the search term. Downloads are saved to ~/Downloads.</>}><SettingsTextField value={draft.searchUrl} onChange={e => setDraft({ ...draft, searchUrl: e.target.value })} /></SettingsFieldRow>
        <SettingsButton type="submit">Save browser settings</SettingsButton>
      </SettingsGroup>
    </form>
    <SettingsButton disabled={busy} onClick={() => setConfirm(true)}>Clear browsing data</SettingsButton>
    {confirm && <div role="alert"><p>Stop the browser and delete all browsing data, including website sign-ins?</p><SettingsButton disabled={busy} onClick={() => void run(async () => { await browserRequest({ action: "clear", confirmed: true }); setConfirm(false); setMessage("Browsing data cleared."); })}>Delete browsing data</SettingsButton><SettingsButton disabled={busy} onClick={() => setConfirm(false)}>Cancel</SettingsButton></div>}
    {error && <p role="alert">{error}</p>}{message && <p role="status">{message}</p>}
  </SettingsPage>;
}
