import { createContext, lazy, Suspense, useContext, useEffect, useRef, useState, type ReactNode, type CSSProperties } from "react";
import { ChevronRight, Network, RotateCcw, Star, X } from "lucide-react";
import type { AgentPolicy, EngineKind } from "@/types";
import { useFeatures, useFeatureSettings } from "@settings/features";
import { useEngines } from "@settings/engines";
import { policyError, setAgentMode } from "@settings/registry";
import { SettingsButton, SettingsSegmented } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import "./composerPolicy.css";

const CommandPolicyEditor = lazy(() => import("./CommandPolicyEditor"));

const ComposerPolicy = createContext<{
  policy: AgentPolicy; scope: string; busy: boolean; error: string; unavailable: boolean;
  save: (policy: AgentPolicy) => Promise<boolean>; configure: () => void; reset?: () => Promise<void>;
} | null>(null);

export function ComposerPolicyProvider({ policy, scope, scopeKey = scope, onChange, onReset, loading = false, loadError, conversation = false, children }: {
  policy: AgentPolicy; scope: string; onChange: (policy: AgentPolicy) => Promise<void>; children: ReactNode;
  scopeKey?: string; onReset?: () => Promise<void>; loading?: boolean; loadError?: string; conversation?: boolean;
}) {
  const settings = useFeatureSettings();
  const engines = useEngines();
  const [draft, setDraft] = useState<AgentPolicy | null>(null);
  const [tab, setTab] = useState("defaults");
  const [busy, setBusy] = useState(false);
  const saving = useRef(false);
  const [error, setError] = useState("");
  const content = useRef<HTMLDivElement>(null);
  const opened = draft !== null;
  const generation = useRef(0);
  useEffect(() => { generation.current++; saving.current = false; setBusy(false); setDraft(null); setError(""); }, [scopeKey]);
  useEffect(() => {
    const body = content.current?.querySelector(".command-editor-body");
    if (body) body.scrollTop = 0;
  }, [tab]);
  useEffect(() => {
    if (!opened) return;
    const previous = document.activeElement as HTMLElement | null;
    content.current?.querySelector<HTMLElement>("button")?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [opened]);
  const save = async (next: AgentPolicy) => {
    if (saving.current || loading || loadError) return false;
    if (next.subagents.enabled && next.subagents.mode === "chain-of-command" && !engines.some(e => e.id === next.defaultEngine && e.protocol === "pi-rpc")) next = { ...next, defaultEngine: "pi" };
    const validation = policyError(next, settings?.providers ?? []);
    if (validation) { setError(validation); return false; }
    const revision = generation.current;
    saving.current = true; setBusy(true); setError("");
    try { await onChange(next); return revision === generation.current; }
    catch (e) { if (revision === generation.current) setError(String(e)); return false; }
    finally { if (revision === generation.current) { saving.current = false; setBusy(false); } }
  };
  const reset = onReset ? async () => {
    if (saving.current || loading) return;
    const revision = generation.current;
    saving.current = true; setBusy(true); setError("");
    try { await onReset(); if (revision === generation.current) setDraft(null); }
    catch (e) { if (revision === generation.current) setError(String(e)); }
    finally { if (revision === generation.current) { saving.current = false; setBusy(false); } }
  } : undefined;
  const close = () => { if (!saving.current) { setDraft(null); setError(""); } };
  const validation = draft ? policyError(draft, settings?.providers ?? []) : null;
  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(policy);
  return <ComposerPolicy.Provider value={{ policy, scope, busy: busy || loading, unavailable: !!loadError, error: loadError || error, save, reset, configure: () => { setDraft(policy); setTab("defaults"); setError(""); } }}>
    {children}
    {draft && <ModalShell ariaLabel="Configure Chain of Command" cardClassName="command-editor settings-page" onBackdropClick={close}>
      <div ref={content} onKeyDown={event => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
        if (event.key === "Tab") {
          const controls = [...event.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary')];
          const first = controls[0], last = controls[controls.length - 1];
          if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
          else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
        }
      }}>
        <header><span className="command-editor-emblem"><Network size={21} aria-hidden /></span><div className="command-editor-title"><h2>Chain of Command</h2><p title={scope}>{scope}</p></div><button type="button" className="ghost" aria-label="Close Chain of Command" disabled={busy} onClick={close}><X size={18} aria-hidden /></button></header>
        <fieldset disabled={busy || loading || !!loadError}>
          <div className="command-editor-navigation"><SettingsSegmented tabs label="Chain of Command settings" value={tab} options={[{ value: "defaults", label: "Team models" }, { value: "subagents", label: "Limits" }, { value: "roles", label: "Roles" }]} onChange={setTab} /></div>
          <div className="command-editor-body" role="tabpanel" aria-label={tab === "defaults" ? "Team models" : tab === "subagents" ? "Limits" : "Roles"}><Suspense fallback={<p role="status">Loading team settings…</p>}><CommandPolicyEditor policy={draft} providers={settings?.providers ?? []} tab={tab} onChange={setDraft} conversation={conversation} /></Suspense></div>
        </fieldset>
        {(validation || loadError || error) && <p role="alert">{validation || loadError || error}</p>}
        <footer><span className="command-save-note" data-dirty={dirty}>{dirty ? "Unsaved changes" : "Applies to new turns"}</span>{reset && <SettingsButton disabled={busy || loading} onClick={() => { void reset(); }}>Use project team</SettingsButton>}<SettingsButton disabled={busy} onClick={close}>Cancel</SettingsButton><SettingsButton className="primary" disabled={busy || loading || !!loadError || !!validation || !dirty} onClick={() => { void save(draft).then(saved => { if (saved) setDraft(null); }); }}>{busy ? "Saving…" : "Save changes"}</SettingsButton></footer>
      </div>
    </ModalShell>}
  </ComposerPolicy.Provider>;
}

export function ChainOfCommandControls({ disabled, engine = "pi", onConfigure }: { disabled?: boolean; engine?: EngineKind; onConfigure: () => void }) {
  const context = useContext(ComposerPolicy);
  const { subagents: available, reasoningEffects } = useFeatures();
  const [activating, setActivating] = useState(false);
  const supported = useEngines().some(e => e.id === engine && e.protocol === "pi-rpc");
  if (!context || !available) return null;
  const { policy, scope, busy, unavailable, save, configure, error, reset } = context;
  const enabled = policy.subagents.enabled && policy.subagents.mode === "chain-of-command";
  return <div className="command-controls">
    <button type="button" className={`command-toggle${activating && enabled ? " is-activating" : ""}`} onAnimationEnd={event => { if (event.animationName === "command-aura-in") setActivating(false); }} aria-label="Chain of Command" aria-pressed={enabled} disabled={disabled || busy || unavailable || (!supported && !enabled)} title={error || (!supported ? "Chain of Command requires Pi or OMP" : `Chain of Command ${enabled ? "on" : "off"} · ${scope}`)} onClick={() => { void save(setAgentMode(policy, enabled ? "single" : "chain-of-command")).then(saved => { if (saved) setActivating(!enabled && reasoningEffects); }); }}>
      <svg className="command-insignia" width="20" height="20" viewBox="0 0 32 34" aria-hidden="true">{[[11, 0], [22, 8], [18, 22], [4, 22], [0, 8]].map(([x, y], index) => <Star className="command-star" key={x} x={x} y={y} width={10} height={10} strokeWidth={1.5} fill="currentColor" style={{ "--star-index": index } as CSSProperties} />)}</svg>
    </button>
    {enabled && !unavailable && <button type="button" className="command-configure" aria-label="Configure Chain of Command" title="Configure Chain of Command" disabled={disabled || busy} onClick={() => { configure(); onConfigure(); }}><ChevronRight size={16} aria-hidden /></button>}
    {reset && (!enabled || unavailable) && <button type="button" className="command-configure" aria-label="Use project team" title="Use project team" disabled={disabled || busy} onClick={() => { void reset(); }}><RotateCcw size={14} aria-hidden /></button>}
    {error && <span role="alert" className="command-toggle-error">{error}</span>}
  </div>;
}
