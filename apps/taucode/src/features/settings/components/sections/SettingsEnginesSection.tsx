import { SettingsIconButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Check, FileText, Pencil, Play, Plus, Puzzle, SlidersHorizontal, Sparkles, Terminal, Trash2, X } from "lucide-react";
import type { AppSettings, EngineDefinition } from "@/types";
import { engineRegistry } from "@settings/engines";
import { defaultAgentPolicy } from "@settings/registry";
import { engineRuntime, pickEngineExecutable, testEngine, type EngineRuntimeStatus } from "@services/tauri";
import { RowMenu } from "@/features/chat/RowMenu";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsPage, SettingsGroup, SettingsRow, SettingsFieldRow, SettingsSelect, SettingsButton, SettingsToggleSwitch, SettingsTextField, SettingsTextArea, SettingsSegmented } from "@/features/design-system/components/settings/SettingsPrimitives";
import "./settings-engines.css";

const runtimeId = (engine: EngineDefinition) => engine.id === "claude" ? "claude-acp" : engine.id;
const protocolLabel = { "pi-rpc": "pi-RPC", acp: "ACP", "codex-app-server": "Codex" };

function EngineDialog({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    content.current?.querySelector<HTMLElement>("input, button")?.focus();
    return () => previous?.focus();
  }, []);
  return <ModalShell ariaLabel={title} className="engine-dialog" cardClassName="engine-dialog-card" onBackdropClick={onClose}><div ref={content} className="settings-page" onKeyDown={event => {
    if (event.key === "Escape") { event.stopPropagation(); onClose(); }
    if (event.key === "Tab") {
      const items = Array.from(content.current!.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex="0"]'));
      const first = items[0], last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
  }}><h2 className="ds-modal-title">{title}</h2>{children}</div></ModalShell>;
}

function EngineEditor({ engine, onClose, onSave, onTested }: { engine: EngineDefinition; onClose: () => void; onSave: (engine: EngineDefinition) => Promise<void>; onTested: (status: EngineRuntimeStatus) => void }) {
  const [draft, setDraft] = useState(engine);
  const [args, setArgs] = useState((engine.args ?? []).join("\n"));
  const [env, setEnv] = useState(Object.entries(engine.env ?? {}));
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const secret = env.find(([key]) => /(KEY|TOKEN|SECRET|PASSWORD)/i.test(key));
  const invalid = !draft.name.trim() ? "Enter an engine name." : !draft.command?.trim() ? "Enter an executable command." : secret ? `${secret[0]}: use a provider instead` : env.some(([key]) => !/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) ? "Environment keys must be valid variable names." : new Set(env.map(([key]) => key)).size !== env.length ? "Environment keys must be unique." : draft.configDirEnv && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(draft.configDirEnv) ? "Enter a valid config directory variable." : "";
  const value = (): EngineDefinition => ({ ...draft, name: draft.name.trim(), command: draft.command?.trim(), args: args.split("\n").filter(Boolean), env: Object.fromEntries(env) });
  const run = async (test: boolean) => {
    if (invalid) return;
    setBusy(true); setResult(null);
    try {
      if (test) { const status = await testEngine(value()); setResult({ ok: true, message: status.message }); onTested(status); }
      else { await onSave(value()); onClose(); }
    } catch (error) { setResult({ ok: false, message: String(error) }); }
    finally { setBusy(false); }
  };
  const close = () => { if (!busy) onClose(); };
  return <EngineDialog title={engine.name ? "Edit engine" : "Add engine"} onClose={close}>
    <SettingsGroup title="Custom engine" disabled={busy}>
      <SettingsFieldRow title="Name"><SettingsTextField value={draft.name} placeholder="Gemini CLI or a pi fork" onChange={e => setDraft({ ...draft, name: e.target.value })} /></SettingsFieldRow>
      <SettingsFieldRow title="Protocol"><SettingsSegmented label="Protocol" value={draft.protocol} options={[{ value: "acp", label: "ACP" }, { value: "pi-rpc", label: "pi-RPC" }]} onChange={protocol => setDraft({ ...draft, protocol: protocol as "acp" | "pi-rpc" })} /></SettingsFieldRow>
      <SettingsFieldRow title="Command"><div className="engine-command"><SettingsTextField value={draft.command ?? ""} placeholder="gemini or /path/to/pi-fork" onChange={e => setDraft({ ...draft, command: e.target.value })} /><SettingsButton onClick={() => { void pickEngineExecutable().then(command => { if (command) setDraft({ ...draft, command }); }).catch(error => setResult({ ok: false, message: String(error) })); }}>Browse…</SettingsButton></div></SettingsFieldRow>
      <SettingsFieldRow title="Arguments" subtitle="One argument per line"><SettingsTextArea value={args} placeholder="--experimental-acp" onChange={e => setArgs(e.target.value)} /></SettingsFieldRow>
      <SettingsRow className="engine-env-row"><div className="settings-toggle-title">Environment <span className="settings-toggle-subtitle">Non-secret values only</span></div><div className="engine-env">{env.map(([key, value], index) => <div className="engine-env-pair" key={index}>
        <SettingsTextField aria-label={`Environment key ${index + 1}`} aria-invalid={/(KEY|TOKEN|SECRET|PASSWORD)/i.test(key) || undefined} placeholder="VARIABLE" value={key} onChange={e => setEnv(env.map((pair, i) => i === index ? [e.target.value, pair[1]] : pair))} />
        <SettingsTextField aria-label={`Environment value ${index + 1}`} placeholder="Value" value={value} onChange={e => setEnv(env.map((pair, i) => i === index ? [pair[0], e.target.value] : pair))} />
        <SettingsIconButton aria-label={`Remove environment variable ${index + 1}`} onClick={() => setEnv(env.filter((_, i) => i !== index))}><X size={14} aria-hidden /></SettingsIconButton>
      </div>)}<SettingsButton onClick={() => setEnv([...env, ["", ""]])}><Plus size={14} aria-hidden /> Add variable</SettingsButton>{secret && <p role="alert" className="engine-error">{secret[0]}: use a provider instead</p>}</div></SettingsRow>
      <SettingsFieldRow title="Config dir variable" subtitle="Optional; points to an isolated directory"><SettingsTextField value={draft.configDirEnv ?? ""} placeholder="PI_CODING_AGENT_DIR" onChange={e => setDraft({ ...draft, configDirEnv: e.target.value || undefined })} /></SettingsFieldRow>
    </SettingsGroup>
    {result && <p role={result.ok ? "status" : "alert"} className={result.ok ? "engine-test-result" : "engine-error"}>{result.ok && <Check size={14} aria-hidden />} {result.message}</p>}
    <footer className="ds-modal-actions"><SettingsButton disabled={busy || !!invalid} title={invalid || undefined} onClick={() => void run(true)}><Play size={14} aria-hidden />{busy ? "Working…" : "Test"}</SettingsButton><span className="engine-footer-space" /><SettingsButton disabled={busy} onClick={close}>Cancel</SettingsButton><SettingsButton className="primary" disabled={busy || !!invalid} title={invalid || undefined} onClick={() => void run(false)}>Save engine</SettingsButton></footer>
  </EngineDialog>;
}

export function SettingsEnginesSection({ appSettings, onUpdateAppSettings, onOpenCodexFlags, onOpenCodexSetup }: { appSettings: AppSettings; onUpdateAppSettings: (settings: AppSettings) => Promise<unknown>; onOpenCodexFlags: () => void; onOpenCodexSetup: () => void }) {
  const registry = engineRegistry(appSettings);
  const policy = appSettings.agentPolicy ?? defaultAgentPolicy;
  const [runtimes, setRuntimes] = useState<EngineRuntimeStatus[]>([]);
  const [tested, setTested] = useState<Record<string, EngineRuntimeStatus>>({});
  const [draft, setDraft] = useState<EngineDefinition | null>(null);
  const [details, setDetails] = useState<{ title: string; message: string } | null>(null);
  const [deleting, setDeleting] = useState<EngineDefinition | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    const refresh = () => void engineRuntime("status").then(value => { if (active) setRuntimes(value); }).catch(e => { if (active) setMessage(String(e)); });
    refresh(); const timer = setInterval(refresh, 1500);
    return () => { active = false; clearInterval(timer); };
  }, []);
  const statusFor = (engine: EngineDefinition) => engine.builtIn ? runtimes.find(r => r.engine === runtimeId(engine)) : tested[engine.id];
  const ready = registry.filter(e => e.enabled && statusFor(e)?.state === "ready");
  const save = async (engines: EngineDefinition[], defaultEngine = policy.defaultEngine) => { await onUpdateAppSettings({ ...appSettings, engines, agentPolicy: { ...policy, defaultEngine } }); };
  const run = async (action: () => Promise<unknown>) => { setMessage(""); try { await action(); } catch (e) { setMessage(String(e)); } };
  const remember = (status: EngineRuntimeStatus) => setTested(previous => ({ ...previous, [status.engine]: status }));
  const install = async (engine: EngineDefinition) => {
    if (engine.id === "codex") { onOpenCodexSetup(); return; }
    setBusy(engine.id);
    try { setRuntimes(await engineRuntime("prepare", runtimeId(engine))); }
    catch (error) { setRuntimes(rows => [...rows.filter(r => r.engine !== runtimeId(engine)), { engine: runtimeId(engine), state: "error", message: String(error) }]); throw error; }
    finally { setBusy(null); }
  };
  const check = async (engine: EngineDefinition) => {
    try {
      const status = engine.builtIn ? (await engineRuntime("status")).find(r => r.engine === runtimeId(engine)) : await testEngine(engine);
      if (!status) throw new Error("Runtime status unavailable");
      if (!engine.builtIn) remember(status);
      setDetails({ title: `${engine.name} · ${engine.builtIn ? "Runtime check" : "Test connection"}`, message: status.message });
    } catch (error) { const message = String(error); if (!engine.builtIn) remember({ engine: engine.id, state: "error", message }); setDetails({ title: `${engine.name} · Test failed`, message }); }
  };
  return <SettingsPage title="Engines" subtitle="Choose an agent runtime. Each engine has isolated configuration; provider credentials stay in the registry.">
    <SettingsGroup title="Default"><SettingsFieldRow title="Default engine"><SettingsSelect aria-label="Default engine" disabled={!ready.length} value={ready.some(e => e.id === policy.defaultEngine) ? policy.defaultEngine : ""} onChange={e => void run(() => save(registry, e.target.value))}>{!ready.some(e => e.id === policy.defaultEngine) && <option value="" disabled>No ready default selected</option>}{ready.map(e => <option key={e.id} value={e.id}>{e.name}</option>)}</SettingsSelect></SettingsFieldRow></SettingsGroup>
    <SettingsGroup title="Installed and available engines" actions={<SettingsButton onClick={() => setDraft({ id: `custom-${crypto.randomUUID()}`, name: "", protocol: "acp", builtIn: false, enabled: true })}><Plus size={14} aria-hidden /> Add engine</SettingsButton>}>{registry.map(engine => {
      const status = statusFor(engine);
      const installing = busy === engine.id || status?.state === "installing" || status?.state === "preparing";
      const version = status?.version ?? status?.message.match(/\bv?(\d+\.\d+\.\d+[^\s]*)/)?.[1];
      const label = !engine.enabled ? "Disabled" : installing ? `Installing${status?.progress === undefined ? "" : ` ${Math.round(status.progress)}%`}…` : status?.state === "ready" ? `Ready${version ? ` · v${version}` : ""}` : status?.state === "error" ? "Error — details" : engine.builtIn ? "Not installed" : "Not tested";
      const installable = engine.builtIn;
      const Icon = !engine.builtIn ? Puzzle : engine.id === "codex" ? Terminal : engine.id === "claude" ? Sparkles : null;
      const menu = [
        { label: "Set as default", icon: <Check size={14} />, disabled: !engine.enabled || status?.state !== "ready" || engine.id === policy.defaultEngine, onClick: () => void run(() => save(registry, engine.id)) },
        { label: "Test connection", icon: <Play size={14} />, disabled: installing, onClick: () => void check(engine) },
        ...(!engine.builtIn ? [{ label: "Edit", icon: <Pencil size={14} />, onClick: () => setDraft(engine) }, { label: "Delete", icon: <Trash2 size={14} />, disabled: engine.id === policy.defaultEngine, onClick: () => setDeleting(engine) }] : []),
        ...(engine.id === "codex" ? [{ label: "Codex flags", icon: <SlidersHorizontal size={14} />, onClick: onOpenCodexFlags }] : []),
        { label: "View logs/details", icon: <FileText size={14} />, onClick: () => setDetails({ title: `${engine.name} · Details`, message: status?.message ?? (engine.builtIn ? "Runtime is not installed. Install it to get started." : "Test this engine to see its protocol handshake result.") }) },
      ];
      return <SettingsRow key={engine.id} className={`engine-row${engine.enabled ? "" : " is-disabled"}`} aria-label={`${engine.name} engine`}>
        <div className="engine-identity"><span className={`engine-icon engine-icon-${engine.id === "omp" ? "omp" : "standard"}`} aria-hidden>{Icon ? <Icon size={18} /> : "π"}</span><div className="engine-copy"><div className="engine-name"><strong>{engine.name}</strong>{engine.id === policy.defaultEngine && <span className="engine-default">Default</span>}</div><div className="engine-meta"><span className="engine-protocol">{protocolLabel[engine.protocol]}</span><span role="status">{label}</span>{engine.id === "omp" && <span>~1.1 GB</span>}</div></div></div>
        <div className="engine-actions">
          {installing ? <><progress aria-label={`Installing ${engine.name}`} max={100} value={status?.progress} /><SettingsButton onClick={() => void run(async () => { await engineRuntime("cancel", runtimeId(engine)); setBusy(null); })}>Cancel</SettingsButton></> : status?.state === "error" ? <SettingsButton onClick={() => void run(() => installable ? install(engine) : check(engine))}>Retry</SettingsButton> : installable && status?.state !== "ready" ? <SettingsButton onClick={() => void run(() => install(engine))}>Install</SettingsButton> : null}
          <SettingsToggleSwitch aria-label={`Enable ${engine.name}`} pressed={engine.enabled} disabled={installing || engine.enabled && registry.filter(e => e.enabled).length === 1} onClick={() => void run(async () => {
            const engines = registry.map(e => e.id === engine.id ? { ...e, enabled: !e.enabled } : e);
            await save(engines, engine.id === policy.defaultEngine && engine.enabled ? ready.find(e => e.id !== engine.id)?.id ?? engines.find(e => e.enabled)!.id : policy.defaultEngine);
            if (engine.id === "omp" && !engine.enabled && status?.state !== "ready") await install(engine);
          })} />
          <RowMenu portal size="md" label={engine.name} actions={menu}>{null}</RowMenu>
        </div>
      </SettingsRow>;
    })}</SettingsGroup>
    {message && <p role="alert" className="engine-error">{message}</p>}
    {draft && <EngineEditor engine={draft} onClose={() => setDraft(null)} onTested={remember} onSave={engine => save([...registry.filter(e => e.id !== engine.id), engine])} />}
    {details && <EngineDialog title={details.title} onClose={() => setDetails(null)}><pre className="engine-details">{details.message}</pre><footer className="ds-modal-actions"><SettingsButton onClick={() => setDetails(null)}>Close</SettingsButton></footer></EngineDialog>}
    {deleting && <EngineDialog title={`Delete ${deleting.name}?`} onClose={() => setDeleting(null)}><p>Remove this engine from settings? Existing conversation files are kept.</p><footer className="ds-modal-actions"><SettingsButton onClick={() => setDeleting(null)}>Cancel</SettingsButton><SettingsButton onClick={() => void run(async () => { await save(registry.filter(e => e.id !== deleting.id)); setDeleting(null); })}>Delete engine</SettingsButton></footer></EngineDialog>}
  </SettingsPage>;
}
