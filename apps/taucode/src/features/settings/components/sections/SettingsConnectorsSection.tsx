import { SettingsPage, SettingsTextField, SettingsSelect, SettingsRow, SettingsButton, SettingsGroup, SettingsTextArea, SettingsFieldRow, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import "./registry.css";
import { useEffect, useState } from "react";
import type { AppSettings, McpCredential, McpServerEntry, McpConnectionResult, WorkspaceInfo, WorkspaceSettings } from "@/types";
import { connectorRequest } from "@services/tauri";

const reserved = ["codegraph", "taucode-browser", "taucode-computer", "taucode-diagnostics"];
const builtin: McpServerEntry = { id: "codegraph", name: "CodeGraph", enabled: true, approval: "auto" };
const empty = (): McpServerEntry => ({ id: "", name: "", enabled: true, approval: "ask", transport: { type: "stdio", command: "", args: [], env: {} } });
const names = (text: string) => text.split("\n").map(s => s.trim()).filter(Boolean);
type Props = { appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void>; workspace?: WorkspaceInfo | null; onUpdateWorkspaceSettings: (id: string, s: Partial<WorkspaceSettings>) => Promise<void> };

export function SettingsConnectorsSection({ appSettings, onUpdateAppSettings, workspace, onUpdateWorkspaceSettings }: Props) {
  const [scope, setScope] = useState("app");
  const local = scope === "workspace" && !!workspace;
  const globals = [...(appSettings.mcpServers ?? [])];
  for (const entry of [{ id: "taucode-computer", name: "Computer Use", enabled: appSettings.computer?.enabled ?? false, approval: "auto" } satisfies McpServerEntry, { ...builtin, enabled: appSettings.codegraphEnabled ?? true }, { id: "taucode-browser", name: "Browser", enabled: true, approval: appSettings.browser?.approval ?? "ask" } satisfies McpServerEntry]) {
    if (!globals.some(s => s.id === entry.id)) globals.push(entry);
  }
  const overrides = workspace?.settings.mcpServers ?? [];
  const entries = local ? [...globals.map(s => ({ ...s, enabled: overrides.find(o => o.id === s.id)?.enabled ?? (s.enabled && (s.id !== "codegraph" || (workspace?.settings.codegraphEnabled ?? true))) })), ...overrides.filter(s => !globals.some(g => g.id === s.id))] : globals;
  const [draft, setDraft] = useState<McpServerEntry | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState<Record<string, boolean>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<McpConnectionResult | null>(null);
  const workspaceId = local ? workspace?.id : undefined;
  useEffect(() => {
    let active = true;
    setSecrets({}); setSaved({});
    if (editing) void connectorRequest<Record<string, boolean>>({ action: "secrets", id: editing, workspaceId }).then(v => { if (active) setSaved(v); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [editing, workspaceId]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const persist = async (next: McpServerEntry[]) => {
    if (local) await onUpdateWorkspaceSettings(workspace!.id, { mcpServers: next });
    else await onUpdateAppSettings({ ...appSettings, codegraphEnabled: next.find(s => s.id === "codegraph")?.enabled ?? true, mcpServers: next });
  };
  const save = async () => {
    if (!draft) return;
    if (!editing && (reserved.includes(draft.id) || entries.some(s => s.id.replace(/-/g, "_") === draft.id.replace(/-/g, "_")))) throw new Error("Connector ID is reserved or already in use");
    const normalized = { ...draft, toolAllowlist: draft.toolAllowlist ? names(draft.toolAllowlist.join("\n")) : null, toolDenylist: draft.toolDenylist ? names(draft.toolDenylist.join("\n")) : null };
    const current = local ? overrides : globals;
    await persist(editing ? current.map(s => s.id === editing ? normalized : s) : [...current, normalized]);
    for (const [name, value] of Object.entries(secrets)) {
      if (value) await connectorRequest({ action: "setSecret", id: draft.id, name, value, workspaceId });
    }
    setSecrets({}); setEditing(draft.id);
    setSaved(await connectorRequest<Record<string, boolean>>({ action: "secrets", id: draft.id, workspaceId }));
  };
  const credentials = draft?.transport?.type === "stdio" ? draft.transport.env : draft?.transport?.headers ?? {};
  const setCredentials = (values: Record<string, McpCredential>) => {
    if (!draft?.transport) return;
    setDraft({ ...draft, transport: draft.transport.type === "stdio" ? { ...draft.transport, env: values } : { ...draft.transport, headers: values } });
  };
  const secretInput = (name: string) => <SettingsFieldRow  title={<>{name === "bearer" ? "Bearer token" : `${name} secret`}{saved[name] && <span>Saved in Keychain</span>}</>}><SettingsTextField aria-label={name === "bearer" ? "Bearer token" : `${name} secret`} type="password" autoComplete="new-password" value={secrets[name] ?? ""} placeholder={saved[name] ? "Saved in Keychain" : "Enter secret"} onChange={e => setSecrets({ ...secrets, [name]: e.target.value })} /></SettingsFieldRow>;
  return <SettingsPage className="registry-settings" title="Connectors" subtitle="MCP tools for Pi and Codex. Changes apply on the next engine start."><SettingsGroup title="Preferences">

    <SettingsFieldRow  title={<>Scope</>}><SettingsSelect disabled={busy} value={local ? "workspace" : "app"} onChange={e => { setScope(e.target.value); setDraft(null); setEditing(null); setResult(null); }}><option value="app">All workspaces</option>{workspace && <option value="workspace">{workspace.name}</option>}</SettingsSelect></SettingsFieldRow>
    {error && <p role="alert">{error}</p>}
    {entries.map(s => {
      const inherited = local && globals.some(g => g.id === s.id);
      return <SettingsRow className="settings-row" key={s.id}>
        <SettingsToggleRow  title={<>{s.name || s.id}</>}><SettingsToggleSwitch pressed={s.enabled} disabled={busy} onClick={() => { const enabled = !(s.enabled); void run(async () => {
          if (inherited) await persist([...overrides.filter(o => o.id !== s.id), { id: s.id, enabled }]);
          else await persist((local ? overrides : globals).map(o => o.id === s.id ? { ...o, enabled } : o));
        }); }} /></SettingsToggleRow>
        {reserved.includes(s.id) && <span>Built in</span>}
        {inherited ? <SettingsButton disabled={busy || !overrides.some(o => o.id === s.id)} onClick={() => void run(() => persist(overrides.filter(o => o.id !== s.id)))}>Use global setting for {s.name}</SettingsButton> : <>
          {["taucode-browser", "taucode-computer"].includes(s.id) ? <span>Configure in Settings → {s.id === "taucode-browser" ? "Browser" : "Computer Use"}</span> : <SettingsButton disabled={busy} onClick={() => { setDraft(structuredClone(s)); setEditing(s.id); setResult(null); }}>Edit {s.name}</SettingsButton>}
          {!reserved.includes(s.id) && <SettingsButton disabled={busy} onClick={() => void run(async () => { await persist((local ? overrides : globals).filter(o => o.id !== s.id)); if (editing === s.id) { setDraft(null); setEditing(null); } })}>Delete {s.name}</SettingsButton>}
        </>}
        <SettingsButton disabled={busy} onClick={() => void run(async () => setResult(await connectorRequest<McpConnectionResult>({ action: "test", id: s.id, workspaceId })))}>Test {s.name}</SettingsButton>
      </SettingsRow>;
    })}
    <SettingsButton disabled={busy} onClick={() => { setDraft(empty()); setEditing(null); setSecrets({}); setSaved({}); setResult(null); }}>Add connector</SettingsButton>
    {draft && <form onSubmit={e => { e.preventDefault(); void run(save); }}>
      <SettingsGroup disabled={busy} title={<>{editing ? "Edit connector" : "New connector"}</>}>

        <SettingsFieldRow  title={<>Connector ID</>}><SettingsTextField required pattern="[a-z0-9_-]{1,32}" maxLength={32} disabled={!!editing} value={draft.id} onChange={e => setDraft({ ...draft, id: e.target.value })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Name</>}><SettingsTextField required value={draft.name ?? ""} onChange={e => setDraft({ ...draft, name: e.target.value })} /></SettingsFieldRow>
        {draft.transport && <>
          <SettingsFieldRow  title={<>Transport</>}><SettingsSelect value={draft.transport.type} onChange={e => { setSecrets({}); setDraft({ ...draft, transport: e.target.value === "stdio" ? { type: "stdio", command: "", args: [], env: {} } : { type: "http", url: "", headers: {} } }); }}><option value="stdio">Local command (stdio)</option><option value="http">HTTP</option></SettingsSelect></SettingsFieldRow>
          {draft.transport.type === "stdio" ? <>
            <SettingsFieldRow  title={<>Command</>}><SettingsTextField required value={draft.transport.command} onChange={e => setDraft({ ...draft, transport: { ...draft.transport as Extract<NonNullable<McpServerEntry["transport"]>, { type: "stdio" }>, command: e.target.value } })} /></SettingsFieldRow>
            <SettingsFieldRow title={<>Arguments (one per line)</>} className="settings-field-row-long"><SettingsTextArea value={draft.transport.args.join("\n")} onChange={e => { if (draft.transport?.type === "stdio") setDraft({ ...draft, transport: { ...draft.transport, args: e.target.value ? e.target.value.split("\n") : [] } }); }} /></SettingsFieldRow>
            <SettingsFieldRow  title={<>Working directory</>}><SettingsTextField value={draft.transport.cwd ?? ""} onChange={e => { if (draft.transport?.type === "stdio") setDraft({ ...draft, transport: { ...draft.transport, cwd: e.target.value || null } }); }} /></SettingsFieldRow>
          </> : <>
            <SettingsFieldRow  title={<>URL</>}><SettingsTextField required type="url" value={draft.transport.url} onChange={e => { if (draft.transport?.type === "http") setDraft({ ...draft, transport: { ...draft.transport, url: e.target.value } }); }} /></SettingsFieldRow>
            <SettingsToggleRow  title={<>Use bearer token</>}><SettingsToggleSwitch pressed={!!draft.transport.bearer} onClick={() => { setSecrets({ ...secrets, bearer: "" }); if (draft.transport?.type === "http") setDraft({ ...draft, transport: { ...draft.transport, bearer: !draft.transport.bearer ? { secret: true } : null } }); }} /></SettingsToggleRow>
            {draft.transport.bearer && secretInput("bearer")}
          </>}
          <h3>{draft.transport.type === "stdio" ? "Environment variables" : "Headers"}</h3>
          {Object.entries(credentials).map(([name, value]) => <div className="connector-credential" key={name}>
            <SettingsFieldRow  title={<>Field name</>}><SettingsTextField required defaultValue={name} onBlur={e => { const next = e.target.value; if (next !== name && !Object.prototype.hasOwnProperty.call(credentials, next)) { const values = { ...credentials }; delete values[name]; values[next] = value; setCredentials(values); setSecrets({ ...secrets, [name]: "" }); } }} /></SettingsFieldRow>
            <SettingsToggleRow  title={<>Store {name} in Keychain</>}><SettingsToggleSwitch pressed={"secret" in value} onClick={() => { setCredentials({ ...credentials, [name]: !("secret" in value) ? { secret: true } : { value: "" } }); setSecrets({ ...secrets, [name]: "" }); }} /></SettingsToggleRow>
            {"secret" in value ? secretInput(name) : <SettingsFieldRow  title={<>{name} value</>}><SettingsTextField value={value.value} onChange={e => setCredentials({ ...credentials, [name]: { value: e.target.value } })} /></SettingsFieldRow>}
            <SettingsButton type="button" onClick={() => { const values = { ...credentials }; delete values[name]; setCredentials(values); setSecrets({ ...secrets, [name]: "" }); }}>Remove {name}</SettingsButton>
          </div>)}
          <SettingsButton type="button" onClick={() => { let name = "NEW_FIELD"; while (Object.prototype.hasOwnProperty.call(credentials, name)) name += "_"; setCredentials({ ...credentials, [name]: { secret: true } }); }}>Add {draft.transport.type === "stdio" ? "environment variable" : "header"}</SettingsButton>
        </>}
        <SettingsFieldRow title={<>Allowed tools (one per line; blank allows all)</>} className="settings-field-row-long"><SettingsTextArea value={draft.toolAllowlist?.join("\n") ?? ""} onChange={e => setDraft({ ...draft, toolAllowlist: e.target.value ? e.target.value.split("\n") : null })} /></SettingsFieldRow>
        <SettingsFieldRow title={<>Denied tools (one per line)</>} className="settings-field-row-long"><SettingsTextArea value={draft.toolDenylist?.join("\n") ?? ""} onChange={e => setDraft({ ...draft, toolDenylist: e.target.value ? e.target.value.split("\n") : null })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Approval</>}><SettingsSelect value={draft.approval ?? "auto"} onChange={e => setDraft({ ...draft, approval: e.target.value as "auto" | "ask" })}><option value="ask">Ask for every tool call</option><option value="auto">Automatic</option></SettingsSelect></SettingsFieldRow>
        <SettingsButton type="submit">Save connector</SettingsButton>
        <SettingsButton type="button" onClick={() => void run(async () => { await save(); setResult(await connectorRequest<McpConnectionResult>({ action: "test", id: draft.id, workspaceId })); })}>Test connection</SettingsButton>
        <SettingsButton type="button" onClick={() => { setDraft(null); setEditing(null); setSecrets({}); }}>Cancel</SettingsButton>
      </SettingsGroup>
    </form>}
    {result && <div role={result.ok ? "status" : "alert"}>{result.ok ? <><p>Connected · {result.latencyMs} ms · {result.tools?.length ?? 0} tools</p><ul>{result.tools?.map(t => <li key={t.name}><strong>{t.name}</strong>{t.description && <p>{t.description}</p>}</li>)}</ul></> : result.error}</div>}
  </SettingsGroup></SettingsPage>;
}
