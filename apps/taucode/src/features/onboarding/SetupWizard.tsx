import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { ArrowRight, Check, FolderOpen, LoaderCircle } from "lucide-react";
import type { AppMode, AppSettings, WorkspaceInfo, WorkspaceSettings } from "@/types";
import { engineRuntime, getAppSettings, getSetupStatus, listWorkspaces, openSetupPrivacy, pickWorkspacePaths, updateAppSettings, updateWorkspaceSettings, type EngineRuntimeStatus, type SetupStatus } from "@services/tauri";
import { notifySetupChanged, subscribeAppServerEvents } from "@services/events";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { useWorkspaceCrud } from "@/features/workspaces/hooks/useWorkspaceCrud";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsButton, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { features, type Features } from "@settings/features";
import { engineRegistry } from "@settings/engines";
import { defaultAgentPolicy } from "@settings/registry";
import { builtins, selectTheme } from "@/features/themes/builtins";
import { useThemes } from "@/features/themes/store";
import { applyTheme } from "@/features/themes/applyTheme";
import { ThemePreview } from "@/features/themes/ThemePreview";
import { useSetupProvider } from "./ProviderSetup";
import { emptyProgress, previewStep, readProgress, SETUP_PROGRESS, SETUP_SEEN, steps, type SetupProgress, type SetupStep } from "./setupState";
import "@/features/themes/themes.css";
import "./onboarding.css";

const titles: Record<SetupStep, string> = { welcome: "Welcome to taucode", look: "Pick a look.", provider: "Connect a model provider.", engine: "Choose your engine.", project: "Open your first project.", features: "Turn on what you want.", permissions: "Let taucode see and operate apps.", done: "You're all set." };
const descriptions: Record<SetupStep, string> = { welcome: "A calm place to work with any model, any engine.", look: "Make yourself at home. You can change this anytime.", provider: "Your models. Your keys. Stored securely in Keychain.", engine: "The runtime that brings your conversations to life.", project: "A place for your ideas to become something real.", features: "Start with what feels useful. Everything can change later.", permissions: "Screen Recording lets taucode see your screen. Accessibility lets it operate apps, when you ask.", done: "Your workspace is ready when you are." };
const featureGroups: { title: string; items: [keyof Features, string, string][] }[] = [
  { title: "Workspace", items: [["git", "Git", "Review changes and manage branches."], ["terminal", "Terminal", "Run commands beside your work."], ["checkpoints", "Checkpoints", "Return to an earlier point in a conversation."]] },
  { title: "Tools for the agent", items: [["browser", "Browser", "Explore and interact with web pages."], ["computerUse", "Computer use", "Let agents see and operate apps."], ["chatMode", "Chat mode", "Talk through ideas without a project."], ["workMode", "Work mode", "Delegate tasks and collect results."], ["diagnostics", "Developer: Diagnostics", "Inspect local activity and troubleshoot."]] },
];
const runtimeId = (id: string) => id === "claude" ? "claude-acp" : id;

export function SetupWizard({ onSettings, onFinish }: { onSettings: (settings: AppSettings) => void; onFinish: (mode: AppMode, workspaceId?: string) => Promise<void> }) {
  const preview = import.meta.env.MODE === "uipreview" ? previewStep(window.location.search) : undefined;
  const [open, setOpen] = useState(() => import.meta.env.MODE === "uipreview" ? !!preview : localStorage.getItem(SETUP_SEEN) !== "done");
  const [modal, setModal] = useState(false);
  const [progress, setProgress] = useState<SetupProgress>(() => preview ? { ...emptyProgress(), step: preview } : readProgress());
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [status, setStatus] = useState<SetupStatus>();
  const [runtimes, setRuntimes] = useState<EngineRuntimeStatus[]>([]);
  const [theme, setTheme] = useState("system");
  const [engine, setEngine] = useState("pi");
  const [enabled, setEnabled] = useState(() => features());
  const [workspaces, setWorkspaces] = useState<WorkspaceInfo[]>([]);
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const [codegraph, setCodegraph] = useState(true);
  const [, setHasLoaded] = useState(false);
  const workspaceSettingsRef = useRef(new Map<string, WorkspaceSettings>());
  const crud = useWorkspaceCrud({ workspaces, setWorkspaces, setActiveWorkspaceId: setWorkspaceId, workspaceSettingsRef, setHasLoaded });
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);
  const cancelled = useRef(false);
  const locked = useRef(false);
  const [error, setError] = useState("");
  const [confirmLater, setConfirmLater] = useState(false);
  const [direction, setDirection] = useState("forward");
  const [exiting, setExiting] = useState(false);
  const [hidden, setHidden] = useState(document.hidden);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const body = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const { files } = useThemes();
  const themes = [...builtins, ...files.flatMap(f => f.manifest ? [f.manifest] : [])];
  const step = progress.step;
  const visibleSteps = steps.filter(s => s !== "permissions" || enabled.computerUse);
  const rail = visibleSteps.filter(s => s !== "welcome" && s !== "done");
  const workspace = workspaces.find(w => w.id === workspaceId);
  const runtime = runtimes.find(r => r.engine === runtimeId(engine));
  const save = async (change: (current: AppSettings) => AppSettings) => {
    const saved = await updateAppSettings(change(await getAppSettings()));
    setSettings(saved); onSettings(saved); notifySetupChanged();
    return saved;
  };
  const provider = useSetupProvider(settings, save, open);

  const refreshStatus = useCallback(() => {
    void getSetupStatus().then(setStatus).catch(() => setError("Could not check system status. Try reopening setup."));
  }, []);
  useEffect(() => {
    const show = () => {
      const saved = readProgress();
      const current = features(settings);
      const first = steps.find(s => s !== "welcome" && s !== "done" && (s !== "permissions" || current.computerUse) && !saved.completed.includes(s));
      setProgress({ ...saved, step: first ?? "done" }); setModal(true); setOpen(true); setError("");
    };
    window.addEventListener("taucode:run-setup", show);
    return () => window.removeEventListener("taucode:run-setup", show);
  }, [settings]);
  useEffect(() => {
    if (!open) return;
    let active = true;
    const previous = document.activeElement as HTMLElement | null;
    void Promise.all([getAppSettings(), listWorkspaces(), engineRuntime("status")]).then(([s, ws, rs]) => {
      if (!active) return;
      if (s.setupComplete && !modal && !preview) { setOpen(false); return; }
      setSettings(s); setTheme(s.theme); setEngine(s.agentPolicy?.defaultEngine ?? "pi"); setEnabled(features(s)); setRuntimes(rs); setWorkspaces(ws);
      const selected = ws[0];
      if (selected) { setWorkspaceId(selected.id); setPath(selected.path); setCodegraph(selected.settings.codegraphEnabled ?? s.codegraphEnabled); }
      workspaceSettingsRef.current = new Map(ws.map(w => [w.id, w.settings]));
      if (!features(s).computerUse) setProgress(p => p.step === "permissions" ? { ...p, step: "done" } : p);
    }).catch(() => { if (active) setError("Could not load setup. Close and reopen it to try again."); });
    refreshStatus();
    const visibility = () => setHidden(document.hidden);
    window.addEventListener("focus", refreshStatus);
    document.addEventListener("visibilitychange", visibility);
    return () => { active = false; clearTimeout(timer.current); locked.current = false; window.removeEventListener("focus", refreshStatus); document.removeEventListener("visibilitychange", visibility); previous?.focus(); };
  }, [open, modal, preview, refreshStatus]);
  useEffect(() => { if (open) localStorage.setItem(SETUP_PROGRESS, JSON.stringify(progress)); }, [progress, open]);
  useEffect(() => { if (open && !exiting) heading.current?.focus(); }, [open, step, exiting]);
  useEffect(() => {
    if (confirmLater) body.current?.querySelector<HTMLButtonElement>(".setup-confirm button")?.focus();
  }, [confirmLater]);
  useEffect(() => {
    if (!open) return;
    const siblings = [...document.body.children].filter((element): element is HTMLElement => element instanceof HTMLElement && !element.contains(body.current));
    const inert = siblings.map(element => element.inert);
    siblings.forEach(element => { element.inert = true; });
    return () => { siblings.forEach((element, index) => { element.inert = inert[index]; }); };
  }, [open]);
  useEffect(() => {
    if (!open) return;
    return subscribeWindowDragDrop(({ payload }) => {
      if (step === "project" && !busy && payload.type === "drop" && payload.paths?.[0]) { setPath(payload.paths[0]); setWorkspaceId(workspaces.find(w => w.path === payload.paths![0])?.id ?? null); }
    }, { exclusive: true });
  }, [open, step, busy, workspaces]);
  useEffect(() => {
    if (!installing) return;
    let active = true;
    let pending = false;
    const poll = async () => {
      if (pending) return;
      pending = true;
      try { const rows = await engineRuntime("status"); if (active) setRuntimes(rows); }
      catch { if (active) setError("Could not read install progress."); }
      finally { pending = false; }
    };
    const interval = setInterval(() => void poll(), 500);
    const unsubscribe = subscribeAppServerEvents(event => {
      if (event.message.method === "engine/runtime") void poll();
    });
    return () => { active = false; clearInterval(interval); unsubscribe(); };
  }, [installing]);

  const move = (next: SetupStep, mark?: "completed" | "skipped") => {
    if (locked.current) return;
    locked.current = true;
    setError(""); provider.clearKey();
    setDirection(steps.indexOf(next) < steps.indexOf(step) ? "back" : "forward"); setExiting(true);
    const nextProgress = { ...progress, step: next };
    if (mark) {
      nextProgress.completed = progress.completed.filter(s => s !== step);
      nextProgress.skipped = progress.skipped.filter(s => s !== step);
      nextProgress[mark] = [...nextProgress[mark], step];
    }
    // Persist the destination immediately, even if the window closes during motion.
    localStorage.setItem(SETUP_PROGRESS, JSON.stringify(nextProgress));
    timer.current = setTimeout(() => { setProgress(nextProgress); setExiting(false); locked.current = false; }, window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ? 150 : 260);
  };
  const next = (mark: "completed" | "skipped" = "completed") => {
    const currentFeatures = step === "features" && mark === "skipped" ? features(settings) : enabled;
    if (step === "features" && mark === "skipped") setEnabled(currentFeatures);
    const order = steps.filter(s => s !== "permissions" || currentFeatures.computerUse);
    move(order[order.indexOf(step) + 1] ?? "done", mark);
  };
  const dismiss = () => { provider.clearKey(); localStorage.setItem(SETUP_SEEN, "done"); setOpen(false); setSettings(null); setConfirmLater(false); };
  const later = () => { if (busy || exiting) return; if (modal) dismiss(); else setConfirmLater(true); };
  const chooseTheme = (id: string) => {
    setTheme(id);
    const selected = selectTheme(id, themes, window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false, settings?.themeLight, settings?.themeDark);
    applyTheme(selected, settings?.themeAnimations?.[selected.id] !== false, settings?.themeAnimations?.[`${selected.id}:neonGlow`] !== false, id === "system");
  };
  const valid = !!settings && (step === "provider" ? provider.valid : step === "project" ? !!path : step === "permissions" ? !!status?.permissions.screen && !!status.permissions.accessibility : step === "engine" ? !!runtime && (runtime.state === "ready" || engine !== "codex" && runtime.state !== "unavailable") : true);
  const submit = async () => {
    if (busy || exiting || !valid) return;
    setBusy(true); setError(""); cancelled.current = false;
    try {
      if (step === "look") await save(s => ({ ...s, theme }));
      if (step === "provider" && !await provider.submit()) return;
      if (step === "engine") {
        cancelled.current = false;
        if (runtime?.state !== "ready") {
          setInstalling(true);
          const rows = await engineRuntime("prepare", runtimeId(engine)); setRuntimes(rows);
          if (cancelled.current) return;
          const result = rows.find(r => r.engine === runtimeId(engine));
          if (result?.state !== "ready") throw new Error(result?.message ?? "Engine installation did not finish.");
        }
        await save(s => ({ ...s, engines: engineRegistry(s).map(e => e.id === engine ? { ...e, enabled: true } : e), agentPolicy: { ...(s.agentPolicy ?? defaultAgentPolicy), defaultEngine: engine } }));
      }
      if (step === "project") {
        let selected = workspaces.find(w => w.path === path);
        if (!selected) {
          const result = await crud.addWorkspacesFromPaths([path]);
          selected = result.firstAdded ?? undefined;
          if (!selected) throw new Error(result.failures[0]?.message ?? "Choose a valid project folder.");
        }
        setWorkspaceId(selected.id);
        workspaceSettingsRef.current.set(selected.id, selected.settings);
        if (status?.codegraph) {
          const updated = await updateWorkspaceSettings(selected.id, { ...selected.settings, codegraphEnabled: codegraph });
          setWorkspaces(rows => rows.map(w => w.id === updated.id ? updated : w));
          workspaceSettingsRef.current.set(updated.id, updated.settings);
        }
        notifySetupChanged(selected.id);
      }
      if (step === "features") await save(s => ({ ...s, features: enabled }));
      if (step === "done") {
        // Chat must be available when there is no project to land in.
        await save(s => ({ ...s, setupComplete: true, features: !workspace ? { ...s.features, chatMode: true } : s.features }));
        await onFinish(workspace ? "code" : "chat", workspace?.id);
        window.dispatchEvent(new Event("taucode:setup-finished"));
        dismiss(); return;
      }
      next();
    } catch (e) { if (!cancelled.current) setError(String(e)); }
    finally { setBusy(false); setInstalling(false); }
  };
  if (!open) return null;
  const content = <div ref={body} className="settings-page setup-wizard" data-hidden={hidden} onKeyDown={event => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); later(); }
    if (event.key === "Tab") {
      const items = [...(body.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), [tabindex="0"]') ?? [])];
      const first = items[0], last = items[items.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === heading.current)) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
    if (event.key.startsWith("Arrow") && event.target instanceof HTMLElement) {
      const grid = event.target.closest("[data-card-grid]");
      if (grid) {
        const cards = [...grid.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
        const index = cards.indexOf(event.target as HTMLButtonElement);
        const delta = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : event.key === "ArrowUp" ? -3 : 3;
        event.preventDefault(); cards[((index + delta) % cards.length + cards.length) % cards.length]?.focus();
      }
    }
    if (event.key === "Enter" && event.target === heading.current) { event.preventDefault(); void submit(); }
  }}>
    <div className="setup-ambient" aria-hidden="true"><i /><i /></div>
    <header className="setup-topbar"><span className="setup-wordmark">taucode<span> / setup</span></span><SettingsButton className="ghost" disabled={busy || exiting} onClick={later}>Set up later</SettingsButton></header>
    <div className="setup-column">
      <ol className="setup-rail" aria-label="Setup progress">{rail.map(s => <li key={s} data-complete={progress.completed.includes(s)} aria-current={step === s ? "step" : undefined}><span className="setup-dot">{progress.completed.includes(s) && <Check size={10} />}</span><span>{s[0].toUpperCase() + s.slice(1)}</span></li>)}</ol>
      <form className="setup-form" onSubmit={e => { e.preventDefault(); void submit(); }}>
        <section key={step} className={`setup-step setup-${direction} ${exiting ? "setup-exit" : "setup-enter"}`} aria-labelledby="setup-title">
          {step === "welcome" && <div className="setup-welcome-mark" aria-hidden="true">τ</div>}
          {step === "done" && <div className="setup-success" aria-hidden="true"><Check size={32} /></div>}
          <h1 id="setup-title" aria-label={titles[step]} ref={heading} tabIndex={-1}>{titles[step].split(" ").map((word, i) => <span key={i} style={{ "--word": i } as CSSProperties}>{word} </span>)}</h1>
          <p className="setup-description">{descriptions[step].split(" ").map((word, i) => <span key={i} style={{ "--word": i } as CSSProperties}>{word} </span>)}</p>
          <fieldset disabled={busy || exiting || confirmLater} className="setup-controls">
            {step === "look" && <div className="setup-theme-grid" data-card-grid>{["system", ...themes.map(t => t.id)].map(id => <button type="button" className="setup-card setup-theme" key={id} aria-pressed={theme === id} onClick={() => chooseTheme(id)}><ThemePreview compact selected={theme === id} theme={selectTheme(id, themes, window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false, settings?.themeLight, settings?.themeDark)} /><strong>{id === "system" ? "System" : themes.find(t => t.id === id)?.name}</strong></button>)}</div>}
            {step === "provider" && provider.fields}
            {step === "engine" && <div className="setup-cards" data-card-grid>{engineRegistry(settings).filter(e => e.builtIn).map(e => {
              const state = runtimes.find(r => r.engine === runtimeId(e.id));
              const unavailable = !state || state.state === "unavailable" || e.id === "codex" && state.state !== "ready";
              return <button type="button" key={e.id} disabled={unavailable} className="setup-card" aria-label={e.name} aria-pressed={engine === e.id} onClick={() => setEngine(e.id)}><span className="setup-engine-icon" aria-hidden>{e.id === "pi" || e.id === "omp" ? "π" : e.id === "claude" ? "✳" : ">_"}</span><strong>{e.name}</strong><small>{e.id === "pi" ? "Recommended · lightweight" : e.id === "omp" ? "More built-in tools · ~1.1 GB download" : e.id === "claude" ? "Claude's coding runtime" : "OpenAI's coding runtime"}</small><span className="setup-chip">{state?.state === "ready" ? "Ready" : unavailable ? "Not available" : "Will install"}</span></button>;
            })}</div>}
            {step === "project" && <><SettingsButton className="setup-dropzone" aria-label="Choose a folder…" onClick={() => void pickWorkspacePaths().then(paths => { if (paths[0]) { setPath(paths[0]); setWorkspaceId(workspaces.find(w => w.path === paths[0])?.id ?? null); } }).catch(() => setError("Could not open the folder dialog."))}><FolderOpen size={30} /><strong>{path ? path.split(/[\\/]/).pop() : "Choose a folder…"}</strong><span>{path || "or drop a folder here"}</span></SettingsButton>{!!path && status?.codegraph && <div className="setup-toggle-row"><span><strong>Code intelligence (codegraph)</strong><small>Understand the connections in your code.</small></span><SettingsToggleSwitch aria-label="Code intelligence (codegraph)" pressed={codegraph} onClick={() => setCodegraph(!codegraph)} /></div>}</>}
            {step === "features" && featureGroups.map(group => <div className="setup-feature-group" key={group.title}><h2>{group.title}</h2>{group.items.map(([key, name, description]) => <div className="setup-toggle-row" key={key}><span><strong>{name}</strong><small>{description}</small></span><SettingsToggleSwitch aria-label={name} pressed={enabled[key]} onClick={() => setEnabled({ ...enabled, [key]: !enabled[key] })} /></div>)}</div>)}
            {step === "permissions" && <div className="setup-permissions">{(["screen", "accessibility"] as const).map(pane => <div className="setup-permission" role="group" aria-label={pane === "screen" ? "Screen Recording" : "Accessibility"} key={pane}><strong>{pane === "screen" ? "Screen Recording" : "Accessibility"}</strong><span role="status" className="setup-chip">{status?.permissions[pane] ? "Granted" : "Not granted"}</span><SettingsButton disabled={!status || status.permissions.unsupported} onClick={() => void openSetupPrivacy(pane).catch(() => setError("Could not open System Settings."))}>Open System Settings</SettingsButton></div>)}<p className="setup-hint">You choose what to grant in System Settings. You can skip this for now.</p></div>}
            {step === "done" && <ol className="setup-recap">{rail.map((s, i) => <li key={s} data-complete={progress.completed.includes(s)} style={{ "--word": i } as CSSProperties}><span className="setup-dot">{progress.completed.includes(s) ? <Check size={14} /> : "–"}</span><strong>{s[0].toUpperCase() + s.slice(1)}</strong><small>{progress.completed.includes(s) ? "Ready" : "Set up later"}</small></li>)}</ol>}
          </fieldset>
        </section>
        {installing && <div className="setup-install" role="status"><span>{runtime?.message || "Installing…"}</span><progress aria-label="Engine install progress" max={100} value={runtime?.progress} /><SettingsButton onClick={() => { cancelled.current = true; void engineRuntime("cancel", runtimeId(engine)).catch(() => { cancelled.current = false; setError("Could not cancel installation."); }); }}>Cancel</SettingsButton></div>}
        {error && <p role="alert" className="setup-error">✕ {error}</p>}
        {confirmLater ? <div className="setup-confirm" role="alertdialog" aria-label="Set up later?"><p>Your applied settings and progress will be kept. Resume from Settings → About.</p><SettingsButton onClick={() => setConfirmLater(false)}>Keep setting up</SettingsButton><SettingsButton onClick={dismiss}>Confirm set up later</SettingsButton></div> : <footer className="setup-footer">
          {step !== "welcome" && <SettingsButton className="ghost" disabled={busy || exiting} onClick={() => move(visibleSteps[visibleSteps.indexOf(step) - 1] ?? "welcome")}>Back</SettingsButton>}
          {step !== "welcome" && step !== "done" && <SettingsButton className="ghost setup-skip" disabled={busy || exiting} onClick={() => next("skipped")}>{step === "project" ? "Skip — I'll just chat for now." : "Skip for now"}</SettingsButton>}
          <SettingsButton type="submit" className="primary setup-primary" disabled={busy || exiting || !valid} aria-busy={busy} data-state={busy ? "loading" : error ? "error" : step === "provider" && provider.connected ? "success" : "idle"}>{busy ? <><LoaderCircle className="setup-spinner" size={16} />{step === "provider" && !provider.connected ? "Testing…" : installing ? "Installing…" : "Saving…"}</> : step === "welcome" ? <>Get started <ArrowRight size={16} /></> : step === "done" ? "Start building" : step === "provider" && !provider.connected ? "Test & continue" : <>{step === "provider" && provider.connected ? <Check size={16} /> : null}Continue <ArrowRight size={16} /></>}</SettingsButton>
        </footer>}
      </form>
    </div>
  </div>;
  return <ModalShell squircle={modal} className={modal ? "setup-overlay" : "setup-takeover"} cardClassName={modal ? "setup-modal-card squircle-clip" : "setup-takeover-card"} ariaLabelledBy="setup-title">{content}</ModalShell>;
}
