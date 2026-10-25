import { SettingsFieldRow } from "@/features/design-system/components/settings/SettingsPrimitives";
import { Pencil, RefreshCw, Trash2 } from "lucide-react";
import { formatRelativeTimeShort } from "@utils/time";
import { SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ModelReasoningSettings } from "./ModelReasoningSettings";
import "./registry.css";
import { useEffect, useState } from "react";
import type { AppSettings, ProviderEntry, ProviderKind, ProviderAuth, AgentPolicy, WorkspaceInfo, WorkspaceSettings } from "@/types";
import { providerSetApiKey, providerHasApiKey, providerDeleteApiKey, providerTestAndFetch } from "@services/tauri";
import { defaultAgentPolicy, modelKey, modelRef, policyError, providerEngines } from "../../registry";

type Props = { appSettings: AppSettings; onUpdateAppSettings: (s: AppSettings) => Promise<void> };
const kinds: ProviderKind[] = ["openai", "anthropic", "google", "openrouter", "groq", "mistral", "xai", "deepseek", "zai", "ollama", "lmstudio", "openai-compatible"];
const newProvider = (): ProviderEntry => ({ id: "", name: "", kind: "openai", auth: { type: "apiKey" }, enabled: true, models: [] });

export function SettingsProvidersSection({ appSettings, onUpdateAppSettings }: Props) {
  const providers = appSettings.providers ?? [];
  const [deleting, setDeleting] = useState<string | null>(null);
  const [testErrors, setTestErrors] = useState<Record<string, string>>({});
  const [draft, setDraft] = useState<ProviderEntry | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [manual, setManual] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    setKey(""); setHasKey(false);
    if (editing) void providerHasApiKey(editing).then(value => { if (active) setHasKey(value); }).catch(() => {});
    return () => { active = false; };
  }, [editing]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const save = async (p: ProviderEntry) => {
    const next = editing ? providers.map(v => v.id === editing ? p : v) : [...providers, p];
    await onUpdateAppSettings({ ...appSettings, providers: next });
    if (key) { await providerSetApiKey(p.id, key); setKey(""); setHasKey(true); }
    setEditing(p.id);
  };
  const auth = (type: ProviderAuth["type"]): ProviderAuth => type === "env" ? { type, var: "" } : type === "command" ? { type, command: "", args: [] } : { type };
  return <section className="settings-section registry-settings">
    <div className="settings-section-heading"><div><h2>Providers</h2><p className="settings-subtitle">Use your own API keys or local models.</p></div><button className="primary" type="button" disabled={busy} onClick={() => { setDraft(newProvider()); setEditing(null); setStatus(""); }}>Add provider</button></div>
    {error && <p role="alert">{error}</p>}
    <div className="settings-card-list">{providers.map(p => <article className="settings-card provider-card" aria-label={`${p.name} provider`} key={p.id}>
      <SettingsToggleRow title={<span className="provider-title"><span className="provider-badge" aria-hidden>{p.name.slice(0, 1).toUpperCase()}</span>{p.name}</span>} subtitle={<><span className="provider-endpoint">{p.kind} · {p.baseUrl || "Default endpoint"}</span><span className="provider-status" role={testErrors[p.id] ? "alert" : "status"}>{testErrors[p.id] || `${p.models.length} models · ${p.modelsFetchedAt ? `tested ${Date.now() / 1000 - p.modelsFetchedAt < 60 ? "just now" : formatRelativeTimeShort(p.modelsFetchedAt * 1000) + " ago"}` : "not tested yet"}`}</span></>}>
        <div className="settings-row-controls">
          <SettingsToggleSwitch aria-label={`Enable ${p.name}`} pressed={p.enabled} disabled={busy} onClick={() => void run(() => onUpdateAppSettings({ ...appSettings, providers: providers.map(v => v.id === p.id ? { ...v, enabled: !p.enabled } : v) }))} />
          <button className="ghost icon-button" type="button" aria-label={`Test & fetch ${p.name}`} title="Test & fetch models" disabled={busy} onClick={() => void run(async () => {
            setTestErrors(previous => ({ ...previous, [p.id]: "" }));
            try {
              const result = await providerTestAndFetch(p.id);
              if (!result.ok) throw new Error(result.error);
              await onUpdateAppSettings({ ...appSettings, providers: providers.map(v => v.id === p.id ? { ...v, models: result.models, modelsFetchedAt: Math.floor(Date.now() / 1000) } : v) });
            } catch (error) { setTestErrors(previous => ({ ...previous, [p.id]: String(error) })); }
          })}><RefreshCw aria-hidden /></button>
          <button className="ghost icon-button" type="button" aria-label={`Edit ${p.name}`} title={`Edit ${p.name}`} disabled={busy} onClick={() => { setDraft(structuredClone(p)); setEditing(p.id); setStatus(""); }}><Pencil aria-hidden /></button>
          <button className="ghost icon-button" type="button" aria-label={`Delete ${p.name}`} title={`Delete ${p.name}`} disabled={busy} onClick={() => setDeleting(p.id)}><Trash2 aria-hidden /></button>
        </div>
      </SettingsToggleRow>
      {deleting === p.id && <div className="provider-delete-confirm" role="alertdialog" aria-label={`Delete ${p.name}?`}><p>Remove {p.name} and its model configuration?</p><button type="button" disabled={busy} onClick={() => void run(async () => { await onUpdateAppSettings({ ...appSettings, providers: providers.filter(v => v.id !== p.id) }); if (editing === p.id) setDraft(null); setDeleting(null); })}>Delete provider</button><button type="button" onClick={() => setDeleting(null)}>Cancel</button></div>}
    </article>)}</div>
    {draft && <form onSubmit={e => { e.preventDefault(); void run(() => save(draft)); }}>
      <SettingsFieldRow  title={<>Provider ID</>}><input required pattern="[a-z0-9_-]{1,32}" maxLength={32} disabled={!!editing} value={draft.id} onChange={e => setDraft({ ...draft, id: e.target.value })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Name</>}><input required value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Kind</>}><select value={draft.kind} onChange={e => { const kind = e.target.value as ProviderKind; setDraft({ ...draft, kind, baseUrl: null, apiStyle: kind === "openai-compatible" ? "chat-completions" : null, auth: { type: kind === "ollama" || kind === "lmstudio" ? "none" : "apiKey" } }); }}>{kinds.map(k => <option key={k}>{k}</option>)}</select></SettingsFieldRow>
      <SettingsFieldRow  title={<>Base URL</>}><input type="url" required={draft.kind === "openai-compatible"} placeholder={draft.kind === "openai-compatible" ? "https://your-server/v1" : "Use provider default"} value={draft.baseUrl ?? ""} onChange={e => setDraft({ ...draft, baseUrl: e.target.value || null })} /></SettingsFieldRow>
      {draft.kind === "openai-compatible" && <SettingsFieldRow  title={<>API style</>}><select value={draft.apiStyle ?? "chat-completions"} onChange={e => setDraft({ ...draft, apiStyle: e.target.value as "responses" | "chat-completions" })}><option value="chat-completions">Chat completions</option><option value="responses">Responses</option></select></SettingsFieldRow>}
      <SettingsToggleRow  title={<>Anthropic-compatible (/v1/messages)</>}><SettingsToggleSwitch pressed={draft.anthropicCompatible ?? false} onClick={() => setDraft({ ...draft, anthropicCompatible: !(draft.anthropicCompatible ?? false) })} /></SettingsToggleRow>
      <p>Engines: {providerEngines(draft).join(" / ")}</p>
      <SettingsFieldRow  title={<>Authentication</>}><select value={draft.auth.type} onChange={e => setDraft({ ...draft, auth: auth(e.target.value as ProviderAuth["type"]) })}><option value="none">None</option><option value="apiKey">API key</option><option value="env">Environment variable</option><option value="command">Command</option></select></SettingsFieldRow>
      {draft.auth.type === "apiKey" && <>
        <SettingsFieldRow  title={<>{hasKey ? "Replace API key" : "API key"}</>}><input type="password" autoComplete="new-password" value={key} onChange={e => setKey(e.target.value)} /></SettingsFieldRow>
        {hasKey && <><span>Saved in Keychain</span><button type="button" disabled={busy} onClick={() => void run(async () => { await providerDeleteApiKey(draft.id); setHasKey(false); })}>Remove key</button></>}
      </>}
      {draft.auth.type === "env" && <SettingsFieldRow  title={<>Environment variable</>}><input required value={draft.auth.var} onChange={e => setDraft({ ...draft, auth: { type: "env", var: e.target.value } })} /></SettingsFieldRow>}
      {draft.auth.type === "command" && <>
        <SettingsFieldRow  title={<>Auth timeout (ms)</>}><input type="number" min="1" step="1" value={draft.auth.timeoutMs ?? 15000} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, timeoutMs: Number(e.target.value) } })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Token command</>}><input required value={draft.auth.command} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, command: e.target.value } })} /></SettingsFieldRow>
        <label>Arguments (one per line)<textarea value={draft.auth.args.join("\n")} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, args: e.target.value.split("\n").filter(Boolean) } })} /></label>
      </>}
      <button type="submit" disabled={busy}>Save provider</button>
      <button type="button" disabled={busy || !editing} aria-busy={busy} onClick={() => void run(async () => {
        await save(draft);
        const result = await providerTestAndFetch(draft.id);
        if (!result.ok) throw new Error(result.error);
        setDraft({ ...draft, models: result.models });
        await onUpdateAppSettings({ ...appSettings, providers: providers.map(p => p.id === draft.id ? { ...draft, models: result.models, modelsFetchedAt: Math.floor(Date.now() / 1000) } : p) });
        setStatus(result.latencyMs + " ms");
      })}>{busy && <span className="registry-spinner" aria-hidden="true" />}{busy ? "Testing…" : "Test & fetch models"}</button><span role="status">{status}</span>
      <div>{draft.models.map(m => <div key={m.id}><SettingsToggleRow  title={<>{m.displayName || m.id}</>}><SettingsToggleSwitch pressed={m.enabled} onClick={() => setDraft({ ...draft, models: draft.models.map(v => v.id === m.id ? { ...v, enabled: !(m.enabled) } : v) })} /></SettingsToggleRow>
        <ModelReasoningSettings model={m} disabled={busy} onChange={model => setDraft({ ...draft, models: draft.models.map(v => v.id === m.id ? model : v) })} onDetect={() => void run(async () => {
          await save(draft);
          const result = await providerTestAndFetch(draft.id, m.id);
          if (!result.ok) throw new Error(result.error);
          const next = { ...draft, models: result.models };
          setDraft(next);
          await onUpdateAppSettings({ ...appSettings, providers: providers.some(p => p.id === draft.id) ? providers.map(p => p.id === draft.id ? next : p) : [...providers, next] });
        })} />
      </div>)}</div>
      <SettingsFieldRow  title={<>Model ID</>}><input value={manual} onChange={e => setManual(e.target.value)} /></SettingsFieldRow>
      <button type="button" disabled={!manual.trim()} onClick={() => { if (!draft.models.some(m => m.id === manual.trim())) setDraft({ ...draft, models: [...draft.models, { id: manual.trim(), enabled: true }] }); setManual(""); }}>Add model ID manually</button>
    </form>}
  </section>;
}

export function AgentPolicyEditor({ policy, providers, onChange }: { policy: AgentPolicy; providers: ProviderEntry[]; onChange: (p: AgentPolicy) => void }) {
  const models = providers.filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ value: p.id + "/" + m.id, label: p.name + " / " + (m.displayName || m.id) })));
  const sub = policy.subagents;
  const modelSelect = (label: string, value: string, change: (value: string) => void) => <SettingsFieldRow  title={<>{label}</>}><select value={value} onChange={e => change(e.target.value)}><option value="">Inherit</option>{models.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}</select></SettingsFieldRow>;
  return <div className="registry-policy">
    <p>Engine enforcement: pi enforces every field. Codex enforces max concurrent, max depth, default subagent model, and roles.</p>
    <SettingsFieldRow  title={<>Default engine</>}><select value={policy.defaultEngine} onChange={e => onChange({ ...policy, defaultEngine: e.target.value as AgentPolicy["defaultEngine"] })}><option value="pi">pi</option><option value="codex">codex</option><option value="claude" disabled={!providers.some(p => p.enabled && providerEngines(p).includes("claude") && p.models.some(m => m.enabled))}>Claude Code (ACP)</option></select></SettingsFieldRow>
    {!providers.some(p => p.enabled && providerEngines(p).includes("claude") && p.models.some(m => m.enabled)) && <small>Claude Code needs an enabled Anthropic or Anthropic-compatible provider and model.</small>}
    <small>Some Anthropic-compatible gateways reject Claude Code's <code>anthropic-beta</code> headers (e.g. LiteLLM with beta overrides disabled). If turns fail with a 400 about beta headers, allow those headers on the gateway or use a different model route.</small>
    {modelSelect("Default model", modelKey(policy.defaultModel), v => onChange({ ...policy, defaultModel: modelRef(v) }))}
    <small>Enable subagents: pi</small>
    <SettingsToggleRow  title={<>Enable subagents</>}><SettingsToggleSwitch pressed={sub.enabled} onClick={() => onChange({ ...policy, subagents: { ...sub, enabled: !(sub.enabled) } })} /></SettingsToggleRow>
    {([["maxConcurrent", "Max concurrent", 16], ["maxPerTurn", "Max per turn", 32], ["maxDepth", "Max depth", 3]] as const).map(([field, label, max]) => <div key={field}><small>{label}: {field === "maxPerTurn" ? "pi" : "pi, codex"}</small><SettingsFieldRow  title={<>{label}</>}><input type="number" min={1} max={max} value={sub[field]} onChange={e => onChange({ ...policy, subagents: { ...sub, [field]: Number(e.target.value) } })} /></SettingsFieldRow></div>)}
    <small>Allowed models: pi</small>
    <SettingsFieldRow  title={<>Allowed models (empty means any enabled model)</>}><select multiple value={sub.allowedModels.map(modelKey)} onChange={e => onChange({ ...policy, subagents: { ...sub, allowedModels: Array.from(e.target.selectedOptions).flatMap(o => { const m = modelRef(o.value); return m ? [m] : []; }) } })}>{models.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}</select></SettingsFieldRow>
    <small>Default subagent model: pi, codex</small>
    {modelSelect("Default subagent model", modelKey(sub.defaultModel), v => onChange({ ...policy, subagents: { ...sub, defaultModel: modelRef(v) } }))}
    <h3>Roles</h3><small>Name, description, model, tool access, instructions: pi, codex</small>
    {sub.roles.map((role, index) => {
      const change = (patch: Partial<typeof role>) => onChange({ ...policy, subagents: { ...sub, roles: sub.roles.map((r, i) => i === index ? { ...r, ...patch } : r) } });
      return <fieldset key={index}><legend>Role {index + 1}</legend>
        <SettingsFieldRow  title={<>Name</>}><input value={role.name} onChange={e => change({ name: e.target.value })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Description</>}><input value={role.description} onChange={e => change({ description: e.target.value })} /></SettingsFieldRow>
        {modelSelect("Role model", modelKey(role.model), v => change({ model: modelRef(v) }))}
        <SettingsFieldRow  title={<>Tool access</>}><select value={role.toolAccess} onChange={e => change({ toolAccess: e.target.value as "read-only" | "full" })}><option value="read-only">Read only</option><option value="full">Full</option></select></SettingsFieldRow>
        <label>Instructions<textarea value={role.instructions} onChange={e => change({ instructions: e.target.value })} /></label>
        <button type="button" onClick={() => onChange({ ...policy, subagents: { ...sub, roles: sub.roles.filter((_, i) => i !== index) } })}>Remove role</button>
      </fieldset>;
    })}
    <button type="button" onClick={() => onChange({ ...policy, subagents: { ...sub, roles: [...sub.roles, { name: "role-" + (sub.roles.length + 1), description: "", model: null, toolAccess: "read-only", instructions: "" }] } })}>Add role</button>
  </div>;
}
export function SettingsAgentPolicySection({ appSettings, onUpdateAppSettings, workspace }: Props & { workspace?: WorkspaceInfo | null }) {
  const [policy, setPolicy] = useState(appSettings.agentPolicy ?? defaultAgentPolicy);
  const [error, setError] = useState("");
  const validation = policyError(policy, appSettings.providers ?? []);
  return <section className="settings-section registry-settings"><h2>Agents</h2>
    <AgentPolicyEditor policy={policy} providers={appSettings.providers ?? []} onChange={setPolicy} />
    {(validation || error) && <p role="alert">{validation || error}</p>}
    {workspace && <details open><summary>Effective policy for {workspace.name} (app + workspace)</summary><fieldset disabled><AgentPolicyEditor policy={mergePolicy(appSettings.agentPolicy ?? defaultAgentPolicy, workspace.settings.agentPolicy)} providers={appSettings.providers ?? []} onChange={() => {}} /></fieldset></details>}
    <button type="button" disabled={!!validation} onClick={() => void onUpdateAppSettings({ ...appSettings, agentPolicy: policy }).then(() => setError("")).catch(e => setError(String(e)))}>Save policy</button>
  </section>;
}
export function ProjectAgentPolicy({ workspace, appSettings, onUpdate }: { workspace: WorkspaceInfo; appSettings: AppSettings; onUpdate: (id: string, settings: Partial<WorkspaceSettings>) => Promise<void> }) {
  const base = appSettings.agentPolicy ?? defaultAgentPolicy;
  const override = workspace.settings.agentPolicy;
  const effective = mergePolicy(base, override);
  const [policy, setPolicy] = useState(effective);
  const [error, setError] = useState("");
  const validation = policyError(policy, appSettings.providers ?? []);
  return <details><summary>{workspace.name}: agent policy</summary>
    <AgentPolicyEditor policy={policy} providers={appSettings.providers ?? []} onChange={setPolicy} />
    {(validation || error) && <p role="alert">{validation || error}</p>}
    <button type="button" disabled={!!validation} onClick={() => void onUpdate(workspace.id, { agentPolicy: policy }).catch(e => setError(String(e)))}>Save project override</button>
    <button type="button" onClick={() => void onUpdate(workspace.id, { agentPolicy: null }).then(() => setPolicy(base)).catch(e => setError(String(e)))}>Use app policy</button>
  </details>;
}

function mergePolicy(base: AgentPolicy, override: WorkspaceSettings["agentPolicy"]): AgentPolicy {
  return { ...base, ...override, defaultModel: override?.defaultModel ?? base.defaultModel, subagents: { ...base.subagents, ...override?.subagents, defaultModel: override?.subagents?.defaultModel ?? base.subagents.defaultModel } };
}
