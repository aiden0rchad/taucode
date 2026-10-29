import { createContext, lazy, Suspense, useContext, useEffect, useRef, useState, type ReactNode, type CSSProperties } from "react";
import { ChevronRight, Star, X } from "lucide-react";
import type { AgentPolicy, EngineKind } from "@/types";
import { useFeatures, useFeatureSettings } from "@settings/features";
import { useEngines } from "@settings/engines";
import { policyError, setAgentMode } from "@settings/registry";
import { SettingsButton, SettingsSegmented } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import "./composerPolicy.css";

const AgentPolicyEditor = lazy(() => import("@settings/components/sections/SettingsRegistrySections").then(module => ({ default: module.AgentPolicyEditor })));

const ComposerPolicy = createContext<{
  policy: AgentPolicy; scope: string; busy: boolean; error: string;
  save: (policy: AgentPolicy) => Promise<boolean>; configure: () => void;
} | null>(null);

export function ComposerPolicyProvider({ policy, scope, onChange, children }: {
  policy: AgentPolicy; scope: string; onChange: (policy: AgentPolicy) => Promise<void>; children: ReactNode;
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
  useEffect(() => { setDraft(null); setError(""); }, [scope]);
  useEffect(() => {
    if (!opened) return;
    const previous = document.activeElement as HTMLElement | null;
    content.current?.querySelector<HTMLElement>("button")?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [opened]);
  const save = async (next: AgentPolicy) => {
    if (saving.current) return false;
    if (next.subagents.enabled && next.subagents.mode === "chain-of-command" && !engines.some(e => e.id === next.defaultEngine && e.protocol === "pi-rpc")) next = { ...next, defaultEngine: "pi" };
    const validation = policyError(next, settings?.providers ?? []);
    if (validation) { setError(validation); return false; }
    saving.current = true; setBusy(true); setError("");
    try { await onChange(next); return true; }
    catch (e) { setError(String(e)); return false; }
    finally { saving.current = false; setBusy(false); }
  };
  const close = () => { if (!saving.current) { setDraft(null); setError(""); } };
  const validation = draft ? policyError(draft, settings?.providers ?? []) : null;
  return <ComposerPolicy.Provider value={{ policy, scope, busy, error, save, configure: () => { setDraft(policy); setTab("defaults"); setError(""); } }}>
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
        <header><div><h2>Chain of Command</h2><p>{scope} · Applies to new turns</p></div><button type="button" className="ghost" aria-label="Close Chain of Command" disabled={busy} onClick={close}><X size={20} aria-hidden /></button></header>
        <fieldset disabled={busy}>
          <SettingsSegmented tabs label="Chain of Command settings" value={tab} options={[{ value: "defaults", label: "Team models" }, { value: "subagents", label: "Limits" }, { value: "roles", label: "Roles" }]} onChange={setTab} />
          <div className="command-editor-body"><Suspense fallback={<p role="status">Loading team settings…</p>}><AgentPolicyEditor compact policy={draft} providers={settings?.providers ?? []} tab={tab as "defaults" | "subagents" | "roles"} onChange={setDraft} /></Suspense></div>
        </fieldset>
        {(validation || error) && <p role="alert">{validation || error}</p>}
        <footer><SettingsButton disabled={busy} onClick={close}>Cancel</SettingsButton><SettingsButton className="primary" disabled={busy || !!validation} onClick={() => { void save(draft).then(saved => { if (saved) setDraft(null); }); }}>{busy ? "Saving…" : "Save changes"}</SettingsButton></footer>
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
  const { policy, scope, busy, save, configure, error } = context;
  const enabled = policy.subagents.enabled && policy.subagents.mode === "chain-of-command";
  return <div className="command-controls">
    <button type="button" className={`command-toggle${activating && enabled ? " is-activating" : ""}`} onAnimationEnd={event => { if (event.animationName === "command-aura-in") setActivating(false); }} aria-label="Chain of Command" aria-pressed={enabled} disabled={disabled || busy || (!supported && !enabled)} title={error || (!supported ? "Chain of Command requires Pi or OMP" : `Chain of Command ${enabled ? "on" : "off"} · ${scope}`)} onClick={() => { void save(setAgentMode(policy, enabled ? "single" : "chain-of-command")).then(saved => { if (saved) setActivating(!enabled && reasoningEffects); }); }}>
      <svg className="command-insignia" width="20" height="20" viewBox="0 0 32 34" aria-hidden="true">{[[11, 0], [22, 8], [18, 22], [4, 22], [0, 8]].map(([x, y], index) => <Star className="command-star" key={x} x={x} y={y} width={10} height={10} strokeWidth={1.5} fill="currentColor" style={{ "--star-index": index } as CSSProperties} />)}</svg>
    </button>
    {enabled && <button type="button" className="command-configure" aria-label="Configure Chain of Command" title="Configure Chain of Command" disabled={disabled || busy} onClick={() => { configure(); onConfigure(); }}><ChevronRight size={16} aria-hidden /></button>}
    {error && <span role="alert" className="command-toggle-error">{error}</span>}
  </div>;
}
