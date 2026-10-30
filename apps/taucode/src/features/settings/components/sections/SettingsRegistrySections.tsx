import { SettingsIconButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEngines } from "@settings/engines";
import { SettingsPage, SettingsButton, SettingsTextField, SettingsSelect, SettingsNumberStepper, SettingsTextArea, SettingsGroup, SettingsFieldRow, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { Pencil, RefreshCw, Trash2 } from "lucide-react";
import { formatRelativeTimeShort } from "@utils/time";
import { ModelReasoningSettings } from "./ModelReasoningSettings";
import { ExecutionLimits } from "./ExecutionLimits";
import "./registry.css";
import { useEffect, useState } from "react";
import type { AppSettings, ProviderEntry, ProviderKind, ProviderAuth, AgentPolicy, WorkspaceInfo, WorkspaceSettings } from "@/types";
import { providerSetApiKey, providerHasApiKey, providerDeleteApiKey, providerTestAndFetch } from "@services/tauri";
import { commandRoles, commandRoleTitle, defaultAgentPolicy, isCommandRole, mergePolicy, modelKey, modelRef, policyError, providerEngines, setAgentMode } from "../../registry";

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
  return <SettingsPage className="registry-settings" title="Providers" subtitle="Use your own API keys or local models."><SettingsGroup title="Providers">
    <div className="settings-section-heading"><SettingsButton className="primary" type="button" disabled={busy} onClick={() => { setDraft(newProvider()); setEditing(null); setStatus(""); }}>Add provider</SettingsButton></div>
    {error && <p role="alert">{error}</p>}
    <div className="settings-card-list">{providers.map(p => <article className="settings-card provider-card" aria-label={`${p.name} provider`} key={p.id}>
      <SettingsToggleRow title={<span className="provider-title"><span className="provider-badge" aria-hidden>{p.name.slice(0, 1).toUpperCase()}</span>{p.name}</span>} subtitle={<><span className="provider-endpoint">{p.kind} · {p.baseUrl || "Default endpoint"}</span><span className="provider-status" role={testErrors[p.id] ? "alert" : "status"}>{testErrors[p.id] || `${p.models.length} models · ${p.modelsFetchedAt ? `tested ${Date.now() / 1000 - p.modelsFetchedAt < 60 ? "just now" : formatRelativeTimeShort(p.modelsFetchedAt * 1000) + " ago"}` : "not tested yet"}`}</span></>}>
        <div className="settings-row-controls">
          <SettingsToggleSwitch aria-label={`Enable ${p.name}`} pressed={p.enabled} disabled={busy} onClick={() => void run(() => onUpdateAppSettings({ ...appSettings, providers: providers.map(v => v.id === p.id ? { ...v, enabled: !p.enabled } : v) }))} />
          <SettingsIconButton className="ghost" type="button" aria-label={`Test & fetch ${p.name}`} title="Test & fetch models" disabled={busy} onClick={() => void run(async () => {
            setTestErrors(previous => ({ ...previous, [p.id]: "" }));
            try {
              const result = await providerTestAndFetch(p.id);
              if (!result.ok) throw new Error(result.error);
              await onUpdateAppSettings({ ...appSettings, providers: providers.map(v => v.id === p.id ? { ...v, models: result.models, modelsFetchedAt: Math.floor(Date.now() / 1000) } : v) });
            } catch (error) { setTestErrors(previous => ({ ...previous, [p.id]: String(error) })); }
          })}><RefreshCw aria-hidden /></SettingsIconButton>
          <SettingsIconButton className="ghost" type="button" aria-label={`Edit ${p.name}`} title={`Edit ${p.name}`} disabled={busy} onClick={() => { setDraft(structuredClone(p)); setEditing(p.id); setStatus(""); }}><Pencil aria-hidden /></SettingsIconButton>
          <SettingsIconButton className="ghost" type="button" aria-label={`Delete ${p.name}`} title={`Delete ${p.name}`} disabled={busy} onClick={() => setDeleting(p.id)}><Trash2 aria-hidden /></SettingsIconButton>
        </div>
      </SettingsToggleRow>
      {deleting === p.id && <div className="provider-delete-confirm" role="alertdialog" aria-label={`Delete ${p.name}?`}><p>Remove {p.name} and its model configuration?</p><SettingsButton type="button" disabled={busy} onClick={() => void run(async () => { await onUpdateAppSettings({ ...appSettings, providers: providers.filter(v => v.id !== p.id) }); if (editing === p.id) setDraft(null); setDeleting(null); })}>Delete provider</SettingsButton><SettingsButton type="button" onClick={() => setDeleting(null)}>Cancel</SettingsButton></div>}
    </article>)}</div>
    {draft && <form onSubmit={e => { e.preventDefault(); void run(() => save(draft)); }}>
      <SettingsFieldRow  title={<>Provider ID</>}><SettingsTextField required pattern="[a-z0-9_-]{1,32}" maxLength={32} disabled={!!editing} value={draft.id} onChange={e => setDraft({ ...draft, id: e.target.value })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Name</>}><SettingsTextField required value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} /></SettingsFieldRow>
      <SettingsFieldRow  title={<>Kind</>}><SettingsSelect value={draft.kind} onChange={e => { const kind = e.target.value as ProviderKind; setDraft({ ...draft, kind, baseUrl: null, apiStyle: kind === "openai-compatible" ? "chat-completions" : null, auth: { type: kind === "ollama" || kind === "lmstudio" ? "none" : "apiKey" } }); }}>{kinds.map(k => <option key={k}>{k}</option>)}</SettingsSelect></SettingsFieldRow>
      <SettingsFieldRow  title={<>Base URL</>}><SettingsTextField type="url" required={draft.kind === "openai-compatible"} placeholder={draft.kind === "openai-compatible" ? "https://your-server/v1" : "Use provider default"} value={draft.baseUrl ?? ""} onChange={e => setDraft({ ...draft, baseUrl: e.target.value || null })} /></SettingsFieldRow>
      {draft.kind === "openai-compatible" && <SettingsFieldRow  title={<>API style</>}><SettingsSelect value={draft.apiStyle ?? "chat-completions"} onChange={e => setDraft({ ...draft, apiStyle: e.target.value as "responses" | "chat-completions" })}><option value="chat-completions">Chat completions</option><option value="responses">Responses</option></SettingsSelect></SettingsFieldRow>}
      <SettingsToggleRow  title={<>Anthropic-compatible (/v1/messages)</>}><SettingsToggleSwitch pressed={draft.anthropicCompatible ?? false} onClick={() => setDraft({ ...draft, anthropicCompatible: !(draft.anthropicCompatible ?? false) })} /></SettingsToggleRow>
      <p>Engines: {providerEngines(draft).join(" / ")}</p>
      <SettingsFieldRow  title={<>Authentication</>}><SettingsSelect value={draft.auth.type} onChange={e => setDraft({ ...draft, auth: auth(e.target.value as ProviderAuth["type"]) })}><option value="none">None</option><option value="apiKey">API key</option><option value="env">Environment variable</option><option value="command">Command</option></SettingsSelect></SettingsFieldRow>
      {draft.auth.type === "apiKey" && <>
        <SettingsFieldRow  title={<>{hasKey ? "Replace API key" : "API key"}</>}><SettingsTextField type="password" autoComplete="new-password" value={key} onChange={e => setKey(e.target.value)} /></SettingsFieldRow>
        {hasKey && <><span>Saved in Keychain</span><SettingsButton type="button" disabled={busy} onClick={() => void run(async () => { await providerDeleteApiKey(draft.id); setHasKey(false); })}>Remove key</SettingsButton></>}
      </>}
      {draft.auth.type === "env" && <SettingsFieldRow  title={<>Environment variable</>}><SettingsTextField required value={draft.auth.var} onChange={e => setDraft({ ...draft, auth: { type: "env", var: e.target.value } })} /></SettingsFieldRow>}
      {draft.auth.type === "command" && <>
        <SettingsFieldRow  title={<>Auth timeout (ms)</>}><SettingsNumberStepper type="number" min="1" step="1" value={draft.auth.timeoutMs ?? 15000} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, timeoutMs: Number(e.target.value) } })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Token command</>}><SettingsTextField required value={draft.auth.command} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, command: e.target.value } })} /></SettingsFieldRow>
        <SettingsFieldRow title={<>Arguments (one per line)</>} className="settings-field-row-long"><SettingsTextArea value={draft.auth.args.join("\n")} onChange={e => setDraft({ ...draft, auth: { ...draft.auth as Extract<ProviderAuth, { type: "command" }>, args: e.target.value.split("\n").filter(Boolean) } })} /></SettingsFieldRow>
      </>}
      <SettingsButton type="submit" disabled={busy}>Save provider</SettingsButton>
      <SettingsButton type="button" disabled={busy || !editing} aria-busy={busy} onClick={() => void run(async () => {
        await save(draft);
        const result = await providerTestAndFetch(draft.id);
        if (!result.ok) throw new Error(result.error);
        setDraft({ ...draft, models: result.models });
        await onUpdateAppSettings({ ...appSettings, providers: providers.map(p => p.id === draft.id ? { ...draft, models: result.models, modelsFetchedAt: Math.floor(Date.now() / 1000) } : p) });
        setStatus(result.latencyMs + " ms");
      })}>{busy && <span className="registry-spinner" aria-hidden="true" />}{busy ? "Testing…" : "Test & fetch models"}</SettingsButton><span role="status">{status}</span>
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
      <SettingsFieldRow  title={<>Model ID</>}><SettingsTextField value={manual} onChange={e => setManual(e.target.value)} /></SettingsFieldRow>
      <SettingsButton type="button" disabled={!manual.trim()} onClick={() => { if (!draft.models.some(m => m.id === manual.trim())) setDraft({ ...draft, models: [...draft.models, { id: manual.trim(), enabled: true }] }); setManual(""); }}>Add model ID manually</SettingsButton>
    </form>}
  </SettingsGroup></SettingsPage>;
}

export function AgentPolicyEditor({ policy, providers, onChange, tab }: { tab?: "defaults" | "subagents" | "roles"; policy: AgentPolicy; providers: ProviderEntry[]; onChange: (p: AgentPolicy) => void }) {
  const engines = useEngines();
  const models = providers.filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ value: p.id + "/" + m.id, label: p.name + " / " + (m.displayName || m.id) })));
  const sub = policy.subagents;
  const chain = sub.enabled && sub.mode === "chain-of-command";
  const modelSelect = (label: string, value: string, change: (value: string) => void) => <SettingsFieldRow  title={<>{label}</>}><SettingsSelect value={value} onChange={e => change(e.target.value)}><option value="">Inherit</option>{models.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}</SettingsSelect></SettingsFieldRow>;
  return <div className="registry-policy">
    <SettingsGroup title="Working mode" subtitle="Choose one model or a team for Code. Chain of Command is available only in Code; Chat and Work use a single model when it is selected.">
      <SettingsFieldRow title="Agent mode"><SettingsSelect value={!sub.enabled ? "single" : sub.mode ?? "custom"} onChange={e => {
        const next = setAgentMode(policy, e.target.value as "single" | "custom" | "chain-of-command");
        if (next.subagents.enabled && next.subagents.mode === "chain-of-command" && !engines.some(engine => engine.id === next.defaultEngine && engine.protocol === "pi-rpc")) next.defaultEngine = "pi";
        onChange(next);
      }}><option value="single">Single model</option><option value="chain-of-command">Chain of Command</option><option value="custom">Custom delegation</option></SettingsSelect></SettingsFieldRow>
      {chain && <p>The conversation model is the commander. Colonels delegate, majors verify, and enlisted agents implement. Runs on Pi or OMP.</p>}
      {!sub.enabled && <p>Subagent delegation is disabled. Your role and model choices are kept.</p>}
    </SettingsGroup>
    {(!tab || tab === "defaults") && <SettingsGroup title="Defaults" subtitle="Engine enforcement: pi enforces every field. Codex enforces max concurrent, max depth, default subagent model, and roles."><SettingsFieldRow  title={<>Default engine</>}><SettingsSelect value={policy.defaultEngine} onChange={e => onChange({ ...policy, defaultEngine: e.target.value as AgentPolicy["defaultEngine"] })}>{engines.filter(e => e.enabled).map(e => <option key={e.id} value={e.id} disabled={chain && e.protocol !== "pi-rpc" || e.id === "claude" && !providers.some(p => p.enabled && providerEngines(p).includes("claude") && p.models.some(m => m.enabled))}>{e.name}</option>)}</SettingsSelect></SettingsFieldRow>
    {!providers.some(p => p.enabled && providerEngines(p).includes("claude") && p.models.some(m => m.enabled)) && <small>Claude Code needs an enabled Anthropic or Anthropic-compatible provider and model.</small>}

    {modelSelect("Default model", modelKey(policy.defaultModel), v => onChange({ ...policy, defaultModel: modelRef(v) }))}
    </SettingsGroup>}
    {(!tab || tab === "subagents") && <SettingsGroup title="Subagents" subtitle="Pi enforces all limits; Codex supports concurrency, depth and default model.">
    <p>{chain ? "Turn and concurrency limits are shared by the whole command tree. Waiting coordinators share their slots with workers." : "Limits apply to each delegator. Nested custom roles have their own per-turn budgets."}</p>
    {([["maxConcurrent", "Max concurrent", 16], ["maxPerTurn", "Max per turn", 32], ["maxDepth", "Max depth", 3]] as const).map(([field, label, max]) => <div key={field}><small>{label}: {field === "maxPerTurn" ? "pi" : "pi, codex"}</small><SettingsFieldRow  title={<>{label}</>}><SettingsNumberStepper type="number" min={1} max={max} value={sub[field]} onChange={e => onChange({ ...policy, subagents: { ...sub, [field]: Number(e.target.value) } })} /></SettingsFieldRow></div>)}
    <small>Allowed models: pi</small>
    <SettingsFieldRow  title={<>Allowed models (empty means any enabled model)</>}><SettingsSelect multiple value={sub.allowedModels.map(modelKey)} onChange={e => onChange({ ...policy, subagents: { ...sub, allowedModels: Array.from(e.target.selectedOptions).flatMap(o => { const m = modelRef(o.value); return m ? [m] : []; }) } })}>{models.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}</SettingsSelect></SettingsFieldRow>
    <small>Default subagent model: pi, codex</small>
    {modelSelect("Default subagent model", modelKey(sub.defaultModel), v => onChange({ ...policy, subagents: { ...sub, defaultModel: modelRef(v) } }))}
    </SettingsGroup>}
    {chain && (!tab || tab === "subagents") && <ExecutionLimits policy={policy} onChange={onChange} />}
    {chain && tab === "defaults" && <SettingsGroup title="Team models" subtitle="Choose smaller cloud or local models where suitable. Inherit uses the default subagent model, then the parent's model. Edit tools and instructions in Roles.">
      {sub.roles.filter(isCommandRole).map(role => <div key={role.name}>{modelSelect(`${commandRoleTitle(role.name)} model`, modelKey(role.model), value => onChange({ ...policy, subagents: { ...sub, roles: sub.roles.map(r => r.name === role.name ? { ...r, model: modelRef(value) } : r) } }))}</div>)}
    </SettingsGroup>}
    {(!tab || tab === "roles") && <SettingsGroup title="Roles" subtitle="Named specialists with their own model, tools and instructions.">
    {sub.roles.map((role, index) => {
      if (chain && !isCommandRole(role)) return null;
      const change = (patch: Partial<typeof role>) => onChange({ ...policy, subagents: { ...sub, roles: sub.roles.map((r, i) => i === index ? { ...r, ...patch } : r) } });
      return <SettingsGroup key={index} title={chain ? commandRoleTitle(role.name) : <>Role {index + 1}</>}>
        <SettingsFieldRow  title={<>Name</>}><SettingsTextField readOnly={chain} value={role.name} onChange={e => change({ name: e.target.value })} /></SettingsFieldRow>
        <SettingsFieldRow  title={<>Description</>}><SettingsTextField value={role.description} onChange={e => change({ description: e.target.value })} /></SettingsFieldRow>
        {modelSelect(chain ? `${commandRoleTitle(role.name)} model` : "Role model", modelKey(role.model), v => change({ model: modelRef(v) }))}
        <SettingsFieldRow  title={<>Tool access</>}><SettingsSelect value={role.toolAccess} onChange={e => change({ toolAccess: e.target.value as "read-only" | "full" })}><option value="read-only">Read only</option><option value="full">Full</option></SettingsSelect></SettingsFieldRow>
        <SettingsFieldRow title={<>Instructions</>} className="settings-field-row-long"><SettingsTextArea value={role.instructions} onChange={e => change({ instructions: e.target.value })} /></SettingsFieldRow>
        {(!chain || !commandRoles.some(core => core.name === role.name)) && <SettingsButton type="button" onClick={() => onChange({ ...policy, subagents: { ...sub, roles: sub.roles.filter((_, i) => i !== index) } })}>Remove role</SettingsButton>}
      </SettingsGroup>;
    })}
    {!chain && <SettingsButton type="button" onClick={() => onChange({ ...policy, subagents: { ...sub, roles: [...sub.roles, { name: "role-" + (sub.roles.length + 1), description: "", model: null, toolAccess: "read-only", instructions: "" }] } })}>Add role</SettingsButton>}
    </SettingsGroup>}
  </div>;
}
export function SettingsAgentPolicySection({ appSettings, onUpdateAppSettings, workspace, tab }: Props & { workspace?: WorkspaceInfo | null; tab?: "defaults" | "subagents" | "roles" }) {
  const [policy, setPolicy] = useState(appSettings.agentPolicy ?? defaultAgentPolicy);
  const [savedPolicy, setSavedPolicy] = useState(policy);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [error, setError] = useState("");
  const validation = policyError(policy, appSettings.providers ?? []);
  const dirty = JSON.stringify(policy) !== JSON.stringify(savedPolicy);
  const save = async () => {
    if (!dirty || validation || saveState === "saving") return;
    setSaveState("saving"); setError("");
    try {
      await onUpdateAppSettings({ ...appSettings, agentPolicy: policy });
      setSavedPolicy(policy); setSaveState("saved");
    } catch (e) { setError(String(e)); setSaveState("idle"); }
  };
  return <div className="registry-settings">
    <AgentPolicyEditor tab={tab} policy={policy} providers={appSettings.providers ?? []} onChange={next => { setPolicy(next); setError(""); }} />
    {(validation || error) && <p role="alert">{validation || error}</p>}
    {workspace && <SettingsGroup collapsible title={<>Effective policy for {workspace.name} (app + workspace)</>} disabled><AgentPolicyEditor policy={mergePolicy(savedPolicy, workspace.settings.agentPolicy)} providers={appSettings.providers ?? []} onChange={() => {}} /></SettingsGroup>}
    <div className="settings-section-actions"><SettingsButton className="primary" disabled={!dirty || !!validation || saveState === "saving"} onClick={() => void save()}>{saveState === "saving" ? "Saving…" : !dirty && saveState === "saved" ? "Saved" : "Save policy"}</SettingsButton><span className="settings-visually-hidden" role="status">{!dirty && saveState === "saved" ? "Policy saved" : ""}</span></div>
  </div>;
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
    <SettingsButton type="button" disabled={!!validation} onClick={() => void onUpdate(workspace.id, { agentPolicy: policy }).catch(e => setError(String(e)))}>Save project override</SettingsButton>
    <SettingsButton type="button" onClick={() => void onUpdate(workspace.id, { agentPolicy: null }).then(() => setPolicy(base)).catch(e => setError(String(e)))}>Use app policy</SettingsButton>
  </details>;
}
