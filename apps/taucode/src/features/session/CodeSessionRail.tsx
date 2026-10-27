import { useEffect, useRef, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ChevronDown, GripVertical, Cpu, FileOutput, GitBranch, Layers, ListTodo, Monitor, Plug, Server, Square, X } from "lucide-react";
import type { BranchInfo, CodeEnvironment, McpServerEntry, SkillOption, ThreadTokenUsage, WorkspaceInfo } from "@/types";
import type { ManagedTasksController } from "@/features/tasks/useManagedTasks";
import { BackgroundTasks } from "@/features/tasks/BackgroundTasks";
import { useSidePanel } from "@/features/panels/SidePanel";
import { IconButton } from "@/features/design-system/components/IconButton";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { contextUsage } from "./useCodeSessionData";
import { isMissingRepo } from "@/features/git/utils/repoErrors";
import "./code-session-rail.css";

const sections = ["Environment", "Context", "Connectors", "Outputs", "Changes", "Servers", "Background tasks"] as const;
type Section = typeof sections[number];
const icons = { Environment: Monitor, Context: Layers, Connectors: Plug, Outputs: FileOutput, Changes: GitBranch, Servers: Server, "Background tasks": ListTodo };
const storageKey = "taucode.code.sessionRail";
function readPreferences(): { order: Section[]; closed: Section[] } {
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) ?? "null");
    return { order: [...new Set<Section>((Array.isArray(saved?.order) ? saved.order : []).filter((id: Section) => sections.includes(id))) , ...sections].filter((id, index, list) => list.indexOf(id) === index),
      closed: (Array.isArray(saved?.closed) ? saved.closed : []).filter((id: Section) => sections.includes(id)) };
  } catch { return { order: [...sections], closed: [] }; }
}

export type CodeSessionRailProps = {
  workspace: WorkspaceInfo;
  workspaces: WorkspaceInfo[];
  branch: string;
  branches: BranchInfo[];
  canCheckoutBranch: boolean;
  onCheckoutBranch: (branch: string) => void;
  onSelectWorkspace: (id: string) => void;
  profiles: CodeEnvironment[];
  engine: string;
  modelId: string | null;
  onSaveEnvironment: (name: string, workspaceIds: string[]) => Promise<void>;
  onSelectEnvironment: (profile: CodeEnvironment) => void;
  onDeleteEnvironment: (id: string) => Promise<void>;
  remote: { mode: "local" | "remote"; host: string };
  usage: ThreadTokenUsage | null;
  protocol: string;
  skills: SkillOption[];
  skillsAvailable: boolean;
  memoryFiles: string[];
  connectors: McpServerEntry[];
  connectorError: string | null;
  connectorsLoading: boolean;
  outputs: string[];
  onOpenFile: (path: string) => boolean;
  git: { enabled: boolean; files: number; additions: number; deletions: number; error: string | null };
  tasks: ManagedTasksController;
  tasksEnabled: boolean;
  browserEnabled: boolean;
  onOpenSettings: (section: "environments" | "server" | "connectors" | "agents") => void;
  onEditLaunchScripts: () => void;
};

function SaveEnvironment({ workspace, workspaces, engine, modelId, onSave, onClose }: Pick<CodeSessionRailProps, "workspace" | "workspaces" | "engine" | "modelId"> & { onSave: CodeSessionRailProps["onSaveEnvironment"]; onClose: () => void }) {
  const [name, setName] = useState(workspace.name);
  const [ids, setIds] = useState([workspace.id]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    formRef.current?.querySelector<HTMLInputElement>("input")?.focus();
    return () => previous?.focus({ preventScroll: true });
  }, []);
  useEffect(() => { if (busy) formRef.current?.focus(); }, [busy]);
  return <ModalShell ariaLabel="New environment" cardClassName="code-environment-dialog" onBackdropClick={() => { if (!busy) onClose(); }}>
    <form ref={formRef} tabIndex={-1} aria-busy={busy} onKeyDown={event => {
      if (event.key === "Escape" && !busy) { event.stopPropagation(); onClose(); }
      if (event.key !== "Tab") return;
      const controls = [...event.currentTarget.querySelectorAll<HTMLElement>("input:not(:disabled),button:not(:disabled)")];
      if (!controls.length) { event.preventDefault(); return; }
      if (event.shiftKey && document.activeElement === controls[0]) { event.preventDefault(); controls[controls.length - 1]?.focus(); }
      else if (!event.shiftKey && document.activeElement === controls[controls.length - 1]) { event.preventDefault(); controls[0]?.focus(); }
    }} onSubmit={event => {
      event.preventDefault();
      setBusy(true); setError(null);
      void onSave(name.trim(), ids).then(onClose).catch(error => setError(String(error))).finally(() => setBusy(false));
    }}>
      <h2 className="ds-modal-title">New environment</h2>
      <label className="ds-modal-label">Name<input required maxLength={80} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></label>
      <fieldset disabled={busy}><legend>Folders</legend>{workspaces.map(entry => <label className="code-environment-folder" key={entry.id}>
        <input type="checkbox" checked={ids.includes(entry.id)} onChange={event => setIds(event.target.checked ? [...ids, entry.id] : ids.filter(id => id !== entry.id))} />
        <span>{entry.name}<small>{entry.path}{entry.kind === "worktree" ? " · Worktree" : ""}</small></span>
      </label>)}</fieldset>
      <p className="ds-modal-subtitle">{engine}{modelId ? ` · ${modelId}` : ""}. Uses each folder’s saved setup and launch scripts. Restoring opens a new conversation draft; scripts run only when you start them.</p>
      {error && <p role="alert">{error}</p>}
      <div className="code-environment-actions"><button type="button" className="ghost" onClick={onClose} disabled={busy}>Cancel</button><button className="primary" disabled={busy || !name.trim() || !ids.length}>{busy ? "Saving…" : "Create"}</button></div>
    </form>
  </ModalShell>;
}

const tokens = (value: number) => value.toLocaleString();

export function CodeSessionRail(props: CodeSessionRailProps) {
  const panel = useSidePanel();
  const [preferences, setPreferences] = useState(readPreferences);
  const [dragged, setDragged] = useState<Section | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const usage = contextUsage(props.usage, props.protocol);
  const running = props.tasks.tasks.filter(task => task.status === "running");
  const servers = props.tasks.servers.filter(server => server.status === "running");
  const finished = props.tasks.tasks.length - running.length;
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running.length) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running.length]);
  useEffect(() => { try { localStorage.setItem(storageKey, JSON.stringify(preferences)); } catch { /* Keep working when storage is unavailable. */ } }, [preferences]);
  const openTool = (id: string) => panel?.openTool(id);
  const openFile = (path: string) => { if (props.onOpenFile(path)) openTool("files"); };
  const openPage = (url: string) => { void panel?.openPage(url).catch(error => setError(String(error))); };
  const stop = (id: string) => { void props.tasks.stop(id).catch(error => setError(String(error))); };
  const elapsed = (started: number) => { const seconds = Math.max(0, Math.floor((now - started) / 1000)); return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`; };
  const move = (section: Section, direction: number) => setPreferences(current => {
    const order = [...current.order]; const from = order.indexOf(section); const to = from + direction;
    if (to < 0 || to >= order.length) return current;
    [order[from], order[to]] = [order[to], order[from]];
    return { ...current, order };
  });
  const contents: Record<Section, ReactNode> = {
    Environment: <>
      <label className="code-session-field">Folder<select aria-label="Session folder" value={props.workspace.id} onChange={event => props.onSelectWorkspace(event.target.value)}>{props.workspaces.map(workspace => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}</select></label>
      <p className="code-session-path" title={props.workspace.path}>{props.workspace.path}</p>
      <label className="code-session-field">Branch<select aria-label="Session branch" value={props.branch} disabled={!props.canCheckoutBranch} onChange={event => props.onCheckoutBranch(event.target.value)}>
        {![...props.branches].some(branch => branch.name === props.branch) && <option value={props.branch}>{props.branch || "Unavailable"}</option>}{props.branches.map(branch => <option key={branch.name} value={branch.name}>{branch.name}</option>)}
      </select></label>
      <div className="code-session-row"><span><Cpu aria-hidden />{props.remote.mode === "remote" ? props.remote.host : "Local computer"}</span><span className="code-session-muted">{props.workspace.connected ? "Connected" : "Disconnected"}</span></div>
      <div className="code-session-links"><button onClick={() => props.onOpenSettings("environments")}>Setup</button><button onClick={props.onEditLaunchScripts}>Launch scripts</button><button onClick={() => setSaving(true)}>Save environment…</button></div>
      {!!props.profiles.length && <details className="code-session-nested"><summary>Saved environments <span>{props.profiles.length}</span></summary>{props.profiles.map(profile => <div className="code-session-row" key={profile.id}>
        <button className="code-session-link" disabled={!profile.workspaceIds.some(id => props.workspaces.some(workspace => workspace.id === id))} onClick={() => props.onSelectEnvironment(profile)}>{profile.name}</button>
        <IconButton size="sm" aria-label={`Delete environment ${profile.name}`} onClick={() => { void props.onDeleteEnvironment(profile.id).catch(error => setError(String(error))); }}><X size={12} /></IconButton>
      </div>)}</details>}
      <div className="code-session-row code-session-remote"><span>Remote access <small>{props.remote.mode === "remote" ? `Daemon · ${props.remote.host}` : "Local backend configured"}</small></span><button className="code-session-link" onClick={() => props.onOpenSettings("server")}>Settings</button></div>
    </>,
    Context: <>
      {usage ? <><div className="code-session-row"><span>{usage.detailed ? "Latest reported usage" : "Context used"}</span><strong>{usage.percent === null ? "Capacity unknown" : `${usage.percent}%`}</strong></div>
        <div className="code-context-bar" role="meter" aria-label={usage.detailed ? "Latest exchange relative to model capacity" : "Context usage"} aria-valuemin={0} aria-valuemax={usage.capacity ?? Math.max(usage.used, 1)} aria-valuenow={Math.min(usage.used, usage.capacity ?? usage.used)} aria-valuetext={`${tokens(usage.used)} tokens${usage.capacity ? ` of ${tokens(usage.capacity)}` : "; model capacity unavailable"}`}>
          {(usage.detailed ? [{ id: "input", value: usage.input }, { id: "cache", value: usage.cached }, { id: "output", value: usage.output }, { id: "other", value: usage.other }] : [{ id: "input", value: usage.used }]).map(segment => <span key={segment.id} data-kind={segment.id} style={{ width: `${Math.min(100, segment.value / Math.max(usage.capacity ?? usage.used, usage.used, 1) * 100)}%` }} />)}
        </div><p className="code-session-muted">{tokens(usage.used)}{usage.capacity ? ` / ${tokens(usage.capacity)}` : ""} tokens</p>
        {usage.detailed ? <dl className="code-context-breakdown"><dt><i data-kind="input" />Input</dt><dd>{tokens(usage.input)}</dd><dt><i data-kind="cache" />Cached input</dt><dd>{tokens(usage.cached)}</dd><dt><i data-kind="output" />Output</dt><dd>{tokens(usage.output)}</dd>{usage.other > 0 && <><dt>Other reported tokens</dt><dd>{tokens(usage.other)}</dd></>}<dt>Reasoning output</dt><dd>{usage.reasoning ? tokens(usage.reasoning) : "Not reported"}</dd></dl> : <p className="code-session-muted">Breakdown not reported</p>}
      </> : <p className="code-session-muted">Usage not reported yet</p>}
      
      <details className="code-session-nested"><summary>MCP tools</summary><p className="code-session-muted">{props.connectors.length} enabled connectors</p><button className="code-session-link" onClick={() => props.onOpenSettings("connectors")}>Manage connectors</button></details>
      <details className="code-session-nested"><summary>Skills <span>{props.skillsAvailable ? props.skills.length : "—"}</span></summary>{props.skillsAvailable ? props.skills.length ? props.skills.map(skill => <button key={skill.path} className="code-session-file" title={skill.path} onClick={() => openFile(skill.path)}>{skill.name}</button>) : <p className="code-session-muted">No skills reported.</p> : <p className="code-session-muted">Skill inventory not reported</p>}<button className="code-session-link" onClick={() => props.onOpenSettings("agents")}>Agent settings</button></details>
      <details className="code-session-nested"><summary>Memory files <span>{props.memoryFiles.length}</span></summary>{props.memoryFiles.length ? props.memoryFiles.map(path => <button className="code-session-file" key={path} onClick={() => openFile(path)}>{path}</button>) : <p className="code-session-muted">None detected</p>}</details>
    </>,
    Connectors: <>{props.connectorsLoading ? <p className="code-session-muted">Loading…</p> : props.connectorError ? <p role="alert">{props.connectorError}</p> : props.connectors.length ? props.connectors.map(connector => <details className="code-session-connector" key={connector.id}>
      <summary><Plug size={12} aria-hidden /><span>{connector.name || connector.id}</span><small>Enabled</small></summary>
      <div className="code-session-connector-detail"><span>{connector.transport?.type === "stdio" ? "Local command" : connector.transport?.type === "http" ? "HTTP" : "Built in"}</span><small>{connector.approval === "auto" ? "Automatic approval" : "Approval policy applies"}</small><button className="code-session-link" onClick={() => props.onOpenSettings("connectors")}>Configure {connector.name || connector.id}</button></div>
    </details>) : <p className="code-session-muted">None enabled</p>}<button className="code-session-link" onClick={() => props.onOpenSettings("connectors")}>Manage connectors</button></>,
    Outputs: <>{props.outputs.length ? <><p className="code-session-muted">Reported files</p>{props.outputs.map(path => <button className="code-session-file" title={path} key={path} onClick={() => openFile(path)}><FileOutput size={13} aria-hidden /><span>{path.split(/[\\/]/).pop()}</span></button>)}</> : <p className="code-session-muted">No file outputs reported yet.</p>}</>,
    Changes: <>{!props.git.enabled ? <p className="code-session-muted">Git is disabled.</p> : props.git.error ? <p className="code-session-muted">{isMissingRepo(props.git.error) ? "Not a Git repository" : props.git.error}</p> : <><button className="code-session-changes" onClick={() => openTool("git")}><span>{props.git.files} changed {props.git.files === 1 ? "file" : "files"}</span><span><b className="code-session-added">+{props.git.additions}</b> <b className="code-session-deleted">−{props.git.deletions}</b></span></button><button className="ghost code-session-commit" onClick={() => openTool("git")}>Commit…</button></>}</>,
    Servers: <>{servers.length ? servers.map(server => <div className="code-session-row" key={server.id}><button className="code-session-file" disabled={!props.browserEnabled} aria-label={`Open server ${server.title}`} title={server.url} onClick={() => openPage(server.url)}><span className="code-session-status" />{server.title}<small>{server.url}</small></button><IconButton size="sm" aria-label={`Stop server ${server.title}`} disabled={props.tasks.busy} onClick={() => { stop(server.taskId); }}><Square size={11} /></IconButton></div>) : <p className="code-session-muted">No URLs reported by running tasks.</p>}<button className="code-session-link" onClick={() => openTool("background")}>{props.tasksEnabled ? "Start a background task" : "Manage tasks"}</button><p className="code-session-muted" title="URLs reported by owned tasks; HTTP readiness is not checked.">Addresses from task output</p></>,
    "Background tasks": <>{running.length ? running.map(task => <div className="code-session-row" key={task.id}><button className="code-session-file" title={task.command} onClick={() => openTool("background")}><span className="code-session-status" />{task.label || task.command}<small>{elapsed(task.startedAt)}</small></button><IconButton size="sm" aria-label={`Stop task ${task.label || task.command}`} disabled={props.tasks.busy} onClick={() => { stop(task.id); }}><Square size={11} /></IconButton></div>) : <p className="code-session-muted">No background tasks running.</p>}<button className="code-session-link" onClick={() => openTool("background")}>{finished ? `${finished} finished · View tasks` : "Manage tasks"}</button>{props.tasks.error && <p role="alert">{props.tasks.error}</p>}{!props.tasksEnabled && <p className="code-session-muted">Enable Terminal to start tasks.</p>}</>,
  };
  const counts: Partial<Record<Section, number>> = { Connectors: props.connectors.length, Outputs: props.outputs.length, Changes: props.git.files, Servers: servers.length, "Background tasks": running.length };
  return <div className="code-session-rail" aria-label="Session details">
    {error && <p role="alert">{error}</p>}
    {preferences.order.map((section, index) => {
      const Icon = icons[section]; const open = !preferences.closed.includes(section); const id = `code-session-${section.toLowerCase().replace(/ /g, "-")}`;
      return <section className={`code-session-card${dragged === section ? " is-dragging" : ""}`} key={section} onDragOver={event => { if (dragged) event.preventDefault(); }} onDrop={event => {
        event.preventDefault();
        if (dragged && dragged !== section) setPreferences(current => { const order = current.order.filter(id => id !== dragged); order.splice(order.indexOf(section), 0, dragged); return { ...current, order }; });
        setDragged(null);
      }} aria-labelledby={`${id}-heading`}><div className="code-session-card-header">
        <button className="code-session-card-toggle" aria-label={`${section}${counts[section] === undefined ? "" : ` (${counts[section]})`}`} id={`${id}-heading`} aria-expanded={open} aria-controls={id} onClick={() => setPreferences(current => ({ ...current, closed: open ? [...current.closed, section] : current.closed.filter(id => id !== section) }))}><Icon size={14} aria-hidden /><span>{section}</span>{counts[section] !== undefined && <small>{counts[section]}</small>}<ChevronDown size={12} className={open ? "is-open" : ""} aria-hidden /></button>
        <div className="code-session-reorder"><span draggable title={`Drag to reorder ${section}`} className="code-session-grip" aria-hidden onDragStart={event => { event.dataTransfer.setData("text/plain", section); event.dataTransfer.effectAllowed = "move"; setDragged(section); }} onDragEnd={() => setDragged(null)}><GripVertical size={12} /></span><IconButton size="sm" aria-label={`Move ${section} up`} disabled={index === 0} onClick={() => move(section, -1)}><ArrowUp size={11} /></IconButton><IconButton size="sm" aria-label={`Move ${section} down`} disabled={index === preferences.order.length - 1} onClick={() => move(section, 1)}><ArrowDown size={11} /></IconButton></div>
      </div><div className="code-session-card-body" id={id} hidden={!open}>{contents[section]}</div></section>;
    })}
    {saving && <SaveEnvironment key={props.workspace.id} workspace={props.workspace} workspaces={props.workspaces} engine={props.engine} modelId={props.modelId} onSave={props.onSaveEnvironment} onClose={() => setSaving(false)} />}
  </div>;
}

export function CodeBackgroundTasks({ tasks, commands, commandsEnabled }: { tasks: ManagedTasksController; commands: import("@/types").LaunchScriptEntry[]; commandsEnabled: boolean }) {
  const panel = useSidePanel();
  const [error, setError] = useState<string | null>(null);
  return <>{error && <p role="alert">{error}</p>}<BackgroundTasks controller={tasks} commandsEnabled={commandsEnabled} commands={commands} onOpenUrl={url => { void panel?.openPage(url).catch(error => setError(String(error))); }} /></>;
}
