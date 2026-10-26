import { formatModelRef } from "@settings/registry";
import { useSidebarScrollFade } from '@app/hooks/useSidebarScrollFade';
import { DigitalRain } from "@/features/themes/DigitalRain";
import { useFeatures } from "@settings/features";
import { Squircle } from "@/features/design-system/components/squircle/Squircle";
import { SidebarTree } from "@app/components/SidebarTree";
import { formatRelativeTimeShort } from "@utils/time";
import { buildThreadTranscript } from "@utils/threadText";
import { exportMarkdownFile } from "@services/tauri";
import { WorkConcurrency } from "./WorkConcurrency";
import { WorkFiles } from "./WorkFiles";
import { useReasoningSelection } from "@/features/models/reasoning";
import { SidebarResizeHandle } from "@/features/layout/components/SidebarResizeHandle";
import { useSidebarState } from "@/features/layout/hooks/useSidebarState";
import { ComposerDictation } from "@/features/composer/components/ComposerDictation";
import { NewConversationRow } from "@/features/chat/NewConversationRow";
import { Folder, FolderLock, X, FolderTree, FileSearch, Table, Presentation, Trash2, CheckCircle2, Circle, ExternalLink, FolderOpen, Eye, Copy, Terminal, LoaderCircle, Search, ChevronDown, FilePenLine, Globe, Bot, ListChecks, CircleAlert } from "lucide-react";
import { ComposerShell, ComposerSendButton, ComposerAccessControl } from "@/features/composer/components/ComposerShell";
import { getFileTypeIconUrl } from "@utils/fileTypeIcons";
import { createElement, useCallback, useEffect, useRef, useState } from "react";
import type { AppSettings, WorkRequest, WorkTask } from "@/types";
import { checkpointRequest, pickWorkspacePath, pickWorkspacePaths, pickWorkAttachments, workAttachment, workOutput, workRequest } from "@services/tauri";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { Markdown } from "@/features/messages/components/Markdown";
import { Checkpoints, RewindAction } from "@/features/checkpoints/Checkpoints";
import { ModeSwitcher } from "@/features/chat/ModeShell";
import { templates, useWork, workGroups } from "./useWork";
import "./work.css";
import Square from "lucide-react/dist/esm/icons/square";
import Plus from "lucide-react/dist/esm/icons/plus";
import { WorkspaceShell } from "@/features/panels/SidePanel";
import { SidebarBottomRail } from "@app/components/SidebarBottomRail";
import { RowMenu } from "@/features/chat/RowMenu";

const templateIcons = { "Organize a folder": FolderTree, "Research brief": FileSearch, "Spreadsheet from documents": Table, "Slide deck from notes": Presentation, "Clean up downloads": Trash2 };
const workToolIcons: Record<string, typeof Terminal> = { commandExecution: Terminal, fileChange: FilePenLine, webSearch: Globe, collabToolCall: Bot, subAgentActivity: Bot, plan: ListChecks };

function InputRequest({ request, busy, respond }: { request: WorkRequest; busy: boolean; respond: (answer: Record<string, unknown>) => void }) {
  const [answer, setAnswer] = useState("");
  const approval = request.method.endsWith("/requestApproval");
  return <section className="work-input" aria-label={approval ? "Approval required" : "Question"}>
    <h3>{approval ? "Approval required" : "Your input is needed"}</h3>
    {request.params.reason && <p>{request.params.reason}</p>}
    {request.params.command && <pre>{request.params.command}</pre>}
    {request.params.diffPreview && <pre>{request.params.diffPreview}</pre>}
    {approval ? <div className="work-actions"><button disabled={busy} onClick={() => respond({ decision: "accept" })}>Allow once</button><button disabled={busy} onClick={() => respond({ decision: "decline" })}>Block</button></div> : <form onSubmit={e => { e.preventDefault(); respond({ answers: { answer: { answers: [answer] } } }); }}>
      {request.params.questions?.map((q, index) => <div key={index}><p>{q.question}</p>{q.options?.map(option => <button type="button" disabled={busy} key={option.label} onClick={() => respond({ answers: { answer: { answers: [option.label] } } })}>{option.label}</button>)}</div>)}
      <label>Your answer <input value={answer} onChange={e => setAnswer(e.target.value)} /></label><button disabled={busy || !answer.trim()}>Reply</button>
    </form>}
  </section>;
}

export function WorkView({ settings, onOpenSettings = () => {} }: { settings: AppSettings; onOpenSettings?: () => void }) {
  const features = useFeatures();
  const work = useWork();
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const searchInput = useRef<HTMLInputElement>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [search, setSearch] = useState("");
  const lastInputTask = useRef<string | null>(null);
  const [scrollTask, setScrollTask] = useState<string | null>(null);
  const [collapsedSpaces, setCollapsedSpaces] = useState<string[]>([]);
  const { sidebarBodyRef, scrollFade, updateScrollFade } = useSidebarScrollFade([work.state, search, collapsedSpaces]);
  const [dragOver, setDragOver] = useState(false);
  const [brief, setBrief] = useState("");
  const [spaceId, setSpaceId] = useState("");
  const [access, setAccess] = useState<WorkTask["accessMode"]>("ask");
  const models = (settings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ ...m, provider: p.name, key: p.id + "/" + m.id, label: p.name + " · " + (m.displayName || m.id) })));
  const [model, setModel] = useState(models[0]?.key ?? "");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [dictating, setDictating] = useState(false);

  const [folder, setFolder] = useState("");
  const [readOnlyFolders, setReadOnlyFolders] = useState<string[]>([]);
  const tasksRef = useRef(work.state.tasks);
  tasksRef.current = work.state.tasks;
  const writableSpaces = work.state.spaces.filter(s => !s.readOnly);
  const selectedSpace = writableSpaces.find(s => s.id === spaceId)?.id || "";
  const needs = work.state.tasks.filter(t => t.status === "needs-input");
  const task = work.task;
  const capability = models.find(m => m.key === model)?.reasoning;
  const reasoning = useReasoningSelection(model, work.active, capability);
  const continuationSpace = writableSpaces.find(s => s.id === spaceId)?.id || writableSpaces.find(s => s.id === task?.spaceId)?.id || writableSpaces[0]?.id || "";
  const active = task?.status === "running" || task?.status === "needs-input";
  const nativeDrop = useRef<(paths: string[]) => void>(() => {});
  useEffect(() => subscribeWindowDragDrop(event => { setDragOver(event.payload.type === "enter" || event.payload.type === "over"); if (event.payload.type === "drop") nativeDrop.current(event.payload.paths ?? []); }), []);
  const run = async (operation: () => Promise<unknown>) => {
    setBusy(true); work.setError("");
    try { await operation(); } catch (error) { work.setError(String(error)); } finally { setBusy(false); }
  };
  const attach = (paths: string[]) => run(async () => {
    if (!selectedSpace) throw new Error("Choose a space first.");
    if (attachments.length + paths.length > 31) throw new Error("Attach at most 31 files.");
    const added: string[] = [];
    for (const path of paths) added.push(await workAttachment(selectedSpace, path));
    setAttachments(current => [...current, ...added]);
  });
  nativeDrop.current = paths => { if (!busy && !task) void attach(paths); };
  const create = (runNow: boolean) => run(async () => {
    if (busy || dictating || !brief.trim() || !selectedSpace || !model) return;
    const created = await work.mutate<WorkTask>("create", { spaceId: selectedSpace, brief, model, accessMode: access, attachments, runNow, effort: reasoning.level });
    work.setActive(created.id); setBrief(""); setAttachments([]); if (window.innerWidth < 760) setSidebarCollapsed(true);
  });
  const selectTask = (id: string) => { work.setActive(id); setSpaceId(""); if (window.innerWidth < 760) setSidebarCollapsed(true); };
  const jumpToInput = () => {
    const entry = needs[(needs.findIndex(t => t.id === lastInputTask.current) + 1) % needs.length];
    if (!entry) return;
    lastInputTask.current = entry.id;
    setSearch("");
    setCollapsedSpaces(current => current.filter(id => id !== entry.spaceId));
    work.setActive(entry.id);
    setSpaceId("");
    setScrollTask(entry.id);
  };
  useEffect(() => {
    if (!scrollTask) return;
    const row = Array.from(sidebarBodyRef.current?.querySelectorAll<HTMLButtonElement>(".work-task-row") ?? []).find(row => row.dataset.taskId === scrollTask);
    row?.scrollIntoView({ block: "nearest" });
    setScrollTask(null);
  }, [scrollTask, sidebarBodyRef]);
  const taskButton = (entry: WorkTask) => {
    const status = entry.status.replace("-", " ");
    const timestamp = entry.updatedAt ?? entry.createdAt;
    const attention = entry.status === "needs-input" || entry.status === "failed";
    const title = [
      entry.title, work.state.spaces.find(space => space.id === entry.spaceId)?.name, formatModelRef(settings, entry.model).title, status,
      ...(["createdAt", "updatedAt"] as const).flatMap(key => entry[key] == null ? [] : [`${key === "createdAt" ? "Created" : "Updated"}: ${new Date(entry[key] * 1000).toLocaleString()}`]),
    ].filter(Boolean).join(" · ");
    return <RowMenu key={entry.id} label={entry.title} actions={[
    { label: "Open task", onClick: () => selectTask(entry.id) },
    ...(["running", "needs-input", "queued"].includes(entry.status) ? [{ label: "Stop task", disabled: busy, onClick: () => { void run(() => work.mutate("stop", { taskId: entry.id })); } }] : []),
  ]}><button className={`thread-row work-task-row is-nested${work.active === entry.id ? " active" : ""}`} data-task-id={entry.id} data-depth={1} title={title} aria-label={entry.title} aria-current={work.active === entry.id ? "page" : undefined} onClick={() => selectTask(entry.id)}><span className={`thread-status work-status-${entry.status}`} aria-label={status} /><span className="thread-name">{entry.title}</span>{(attention || timestamp != null) && <span className={`thread-time${attention ? ` work-attention-${entry.status}` : ""}`}>{attention ? status : formatRelativeTimeShort(timestamp! * 1000)}</span>}</button></RowMenu>;
  };
  const checkpoints = useCallback<typeof checkpointRequest>((_space, params) => {
    const task = tasksRef.current.find(t => t.threadId === params.threadId);
    return workRequest("checkpoint", { taskId: task?.id, request: params });
  }, []);
  const taskMarkdown = task ? `# ${task.title}\n\n${task.brief}\n\n${buildThreadTranscript(work.items)}` : "";
  const taskPanel = task ? (<aside className="work-inspector" aria-label="Task details">
          <section aria-label="Plan"><h2>Plan</h2>{task.plan.length ? <><progress className="plan-progress" aria-label="Plan completion" max={task.plan.length} value={task.plan.filter(step => step.status === "completed").length} /><ul className="work-plan">{task.plan.map((step, index) => <li key={index}><span role="checkbox" aria-label={step.step} aria-checked={step.status === "completed"}>{step.status === "completed" ? <CheckCircle2 aria-hidden /> : <Circle aria-hidden />}</span><span>{step.step}</span></li>)}</ul></> : <p className="chat-muted">The task’s checklist will appear here.</p>}</section>
          <section aria-label="Outputs"><h2>Outputs <small>{task.outputs.length}</small></h2>{!task.outputs.length && <p className="chat-muted">Created and modified files will appear here.</p>}
            {task.outputs.map(path => <div className="work-output" key={path}><img className="file-type-icon" src={getFileTypeIconUrl(path)} alt="" /><strong title={path}>{path.split("/").pop()}</strong><div className="work-actions">{(["open", "reveal", ...(/\.(png|jpe?g|gif|webp|pdf)$/i.test(path) ? ["preview"] : [])] as const).map(action => <button className="ghost icon-button" key={action} aria-label={action === "open" ? "Open" : action === "reveal" ? "Reveal in Finder" : "Quick Look"} title={action === "open" ? "Open" : action === "reveal" ? "Reveal in Finder" : "Quick Look"} disabled={busy} onClick={() => { void run(() => workOutput(task.id, path, action as "open" | "reveal" | "preview")); }}>{action === "open" ? <ExternalLink aria-hidden /> : action === "reveal" ? <FolderOpen aria-hidden /> : <Eye aria-hidden />}</button>)}</div></div>)}
          </section>
        </aside>) : null;
  return <WorkspaceShell titleActions={<>
        <span className="thread-header-model" title={formatModelRef(settings, task?.model ?? model).title}>pi · {formatModelRef(settings, task?.model ?? model).name}</span>
        {task && <div className="work-actions"><span className={`thread-state-chip ${task.status === "running" ? "processing" : task.status === "needs-input" ? "reviewing" : "ready"}`} role="status">{task.status.replace("-", " ")}</span>{(active || task.status === "queued") && <button className="ghost main-header-action" aria-label="Stop task" title="Stop task" disabled={busy} onClick={() => { void run(() => work.mutate("stop", { taskId: task.id })); }}>Stop</button>}
          {task.threadId && !task.importId && <RewindAction workspaceId={task.spaceId} threadId={task.threadId} running={active} />}
        </div>}
        {work.state.tasks.some(t => ["running", "needs-input"].includes(t.status)) && <button className="ghost main-header-action" aria-label="Stop all tasks" title="Stop all tasks" disabled={busy} onClick={() => { void run(() => work.mutate("stop-all")); }}><Square size={14} aria-hidden /></button>}
      </>} reservedShortcuts={Object.entries(settings).filter(([key, value]) => key.endsWith("Shortcut") && typeof value === "string").map(([, value]) => String(value))} scope={`work:${work.active ?? "new"}`} title={task?.title || "New task"} actions={task ? [{ label: "Copy", onClick: () => { void run(() => navigator.clipboard.writeText(taskMarkdown)); } }, { label: "Export", onClick: () => { void run(() => exportMarkdownFile(taskMarkdown, "task.md", "Export task")); } }] : []} onNavigate={scope => work.setActive(scope.slice(5) === "new" ? null : scope.slice(5))} tools={task ? [{ id: "task", label: "Task", content: taskPanel }, { id: "files", label: "Files", content: <WorkFiles tasks={work.state.tasks} spaceId={task.spaceId} /> }] : selectedSpace ? [{ id: "files", label: "Files", content: <WorkFiles tasks={work.state.tasks} spaceId={selectedSpace} /> }] : []} searchUrl={settings.browser?.searchUrl} onInterrupt={() => work.active ? workRequest("stop", { taskId: work.active }).then(() => {}) : undefined}><div className={"chat-layout work-layout" + (sidebarCollapsed ? " sidebar-collapsed" : "")}>
    <SidebarResizeHandle />
    <aside className="sidebar chat-sidebar" aria-label="Spaces and tasks">
      <ModeSwitcher />
      {!searchOpen && <NewConversationRow label="New task" disabled={busy} shortcut={settings.newAgentShortcut} handleShortcut onSearch={() => setSearchOpen(true)} onNew={() => { work.setActive(null); if (window.innerWidth < 760) setSidebarCollapsed(true); }} />}
      {searchOpen && <label className="sidebar-search is-open"><Search aria-hidden /><input autoFocus onKeyDown={e => { if (e.key === "Escape") { setSearch(""); setSearchOpen(false); } }} ref={searchInput} type="search" aria-label="Search tasks" placeholder="Search tasks" value={search} onChange={e => setSearch(e.target.value)} /><button type="button" className="ghost icon-button" aria-label="Close search" onClick={() => { setSearch(""); setSearchOpen(false); }}><X aria-hidden /></button></label>}
      {needs.length > 0 && <button className="ghost work-input-callout" onClick={jumpToInput}><span className="thread-status work-status-needs-input" aria-hidden />{needs.length} {needs.length === 1 ? "task needs" : "tasks need"} your input</button>}
      <div className="sidebar-section-header"><span className="sidebar-section-title">Spaces</span><div className="mode-section-actions"><button className="ghost sidebar-title-add" aria-label="Add space…" title="Add space…" disabled={busy} onClick={() => { void run(async () => { const path = await pickWorkspacePath(); if (path) { setFolder(path); setReadOnlyFolders([]); work.setActive(null); if (window.innerWidth < 760) setSidebarCollapsed(true); } }); }}><Plus size={14} aria-hidden /></button><WorkConcurrency value={work.state.concurrency} disabled={busy} onChange={count => { void run(() => work.mutate("concurrency", { count })); }} /></div></div>
      <div ref={sidebarBodyRef} onScroll={updateScrollFade} className={`chat-list sidebar-body${scrollFade.top ? " fade-top" : ""}${scrollFade.bottom ? " fade-bottom" : ""}`}>
        {work.state.spaces.map(space => <Squircle as="section" className={`work-space project-group-card${(task?.spaceId ?? selectedSpace) === space.id ? " is-selected" : ""}`} key={space.id} aria-label={space.name}>
          <h2 className="workspace-row" data-depth={0} title={space.path}><button className="ghost sidebar-section-title work-space-toggle" aria-expanded={!collapsedSpaces.includes(space.id)} onClick={() => setCollapsedSpaces(current => current.includes(space.id) ? current.filter(id => id !== space.id) : [...current, space.id])}>{collapsedSpaces.includes(space.id) ? <Folder aria-hidden /> : <FolderOpen aria-hidden />}<span className="workspace-name">{space.name}</span><ChevronDown className="work-space-chevron" aria-hidden /></button><span className="workspace-count">{work.state.tasks.filter(t => t.spaceId === space.id).length}</span><button type="button" className="ghost sidebar-title-add" aria-label={`New task in ${space.name}`} title={`New task in ${space.name}`} disabled={busy || space.readOnly} onClick={() => { setSpaceId(space.id); work.setActive(null); if (window.innerWidth < 760) setSidebarCollapsed(true); }}><Plus aria-hidden /></button></h2>
          {!collapsedSpaces.includes(space.id) && space.readOnly && <p className="chat-muted">Read-only history. Choose an available space to continue.</p>}
          {!collapsedSpaces.includes(space.id) && !!space.readOnlyFolders.length && <details className="work-readonly-folders"><summary><FolderLock aria-hidden /><span>{space.readOnlyFolders.length} read-only {space.readOnlyFolders.length === 1 ? "folder" : "folders"}</span><ChevronDown aria-hidden /></summary>{space.readOnlyFolders.map(path => <p key={path} className="chat-muted">{path}</p>)}</details>}
          {!collapsedSpaces.includes(space.id) && <SidebarTree rows={workGroups.flatMap(([, statuses]) => work.state.tasks.filter(t => t.spaceId === space.id && t.title.toLowerCase().includes(search.toLowerCase()) && (statuses as readonly string[]).includes(t.status))).map(task => ({ ...task, depth: 0 }))} rowKey={task => task.id} selected={task => work.active === task.id}>{taskButton}</SidebarTree>}
        </Squircle>)}
      </div>
      <SidebarBottomRail onOpenSettings={onOpenSettings} onOpenDebug={() => {}} showDebugButton={false} />
    </aside>
    <main className="chat-main">

      {work.error && <p role="alert" className="work-error">{work.error}</p>}
      {folder && <section className="work-add-space" aria-label="New space"><h2>New space</h2><p>{folder}</p><p className="chat-muted">This is the folder where tasks can create files. Add other folders for reading only.</p>
        {readOnlyFolders.map(path => <p key={path}>{path}<button onClick={() => setReadOnlyFolders(current => current.filter(p => p !== path))}>Remove</button></p>)}
        <div className="work-actions"><button disabled={busy} onClick={() => { void run(async () => { const paths = await pickWorkspacePaths(); setReadOnlyFolders(current => [...new Set([...current, ...paths])]); }); }}>Add read-only folders…</button>
          <button disabled={busy} onClick={() => { void run(async () => { const space = await work.mutate<{ id: string }>("add-space", { path: folder, readOnlyFolders }); setSpaceId(space.id); setAttachments([]); setFolder(""); }); }}>Create space</button>
          <button onClick={() => setFolder("")}>Cancel</button></div>
      </section>}
      {task ? <div className="work-task-view">
        <div className="chat-scroll" role="log" aria-label="Task progress" aria-live="polite"><div className="chat-column">
          <article className="message chat-message user"><h2>Task brief</h2><Markdown codeBlockStyle="message" className="markdown" value={task.brief} showFilePath={false} /><ul>{task.attachments.map(path => <li key={path}>{path}</li>)}</ul></article>
          {task.pending.map(request => <InputRequest key={request.id} request={request} busy={busy} respond={answer => { void run(() => work.mutate("respond", { taskId: task.id, requestId: request.id, answer })); }} />)}
          {task.error && <p role="alert">{task.error}</p>}
          {work.items.map(item => item.kind === "message" ? <article className={"message chat-message " + item.role} key={item.id}><h2>{item.role === "user" ? "You" : "Assistant"}</h2><Markdown codeBlockStyle="message" className="markdown" value={item.text} showFilePath={false} /><div className="message-hover-actions"><button className="ghost icon-button" title="Copy message" aria-label="Copy message" onClick={() => { void navigator.clipboard.writeText(item.text).catch(() => work.setError("Could not copy message.")); }}><Copy aria-hidden /></button></div></article> : item.kind === "tool" ? <details className={`work-tool${/fail|error/i.test(item.status ?? "") ? " is-error" : ""}`} key={item.id}><summary>{createElement(/fail|error/i.test(item.status ?? "") ? CircleAlert : item.status === "inProgress" ? LoaderCircle : workToolIcons[item.toolType] ?? Search, { "aria-hidden": true, className: item.status === "inProgress" ? "tool-spinner" : undefined })}{item.title} · {item.status}<ChevronDown aria-hidden /></summary><pre>{item.detail}</pre>{item.output && <Markdown codeBlockStyle="message" className="markdown" value={item.output} showFilePath={false} />}</details> : item.kind === "reasoning" ? <details key={item.id}><summary>Thinking</summary><Markdown codeBlockStyle="message" className="markdown" value={item.content || item.summary} showFilePath={false} /></details> : null)}
          {task.status === "queued" && <p className="chat-muted">Waiting for an available slot. One task runs in each space at a time.</p>}
          {active && <p className="chat-muted">This task keeps running when you switch modes or hide the window.</p>}
          {task.importId ? <section aria-label="Continue imported task">
            <p className="chat-muted">Read-only Cowork history. Continue starts a new task with summarized context and the last 10 messages.</p>
            <label>Space <select aria-label="Continuation space" value={continuationSpace} onChange={e => setSpaceId(e.target.value)} disabled={busy}><option value="" disabled>Choose an available space</option>{writableSpaces.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
            <label>Model <select aria-label="Continuation model" value={model} onChange={e => setModel(e.target.value)} disabled={busy}>{!models.length && <option value="">Enable a provider in Code → Settings</option>}{models.map(m => <option key={m.key} value={m.key}>{m.label}</option>)}</select></label>
            <button disabled={busy || !continuationSpace || !model} onClick={() => { void run(async () => {
              const created = await work.mutate<WorkTask>("continue", { taskId: task.id, spaceId: continuationSpace, model, ...(reasoning.level ? { effort: reasoning.level } : {}) });
              work.setActive(created.id);
            }); }}>Continue</button>
          </section> : !active && task.status !== "queued" && <button onClick={() => { setBrief("Continue this task:\n" + task.brief + "\n\nReviewed plan:\n" + task.plan.map(p => "- " + p.step).join("\n") + "\n\nAsk before changing any existing files."); setSpaceId(task.spaceId); setAccess("ask"); setModel(task.model); work.setActive(null); }}>Continue as a new task…</button>}
        </div></div>

      </div> : <div className="chat-scroll"><div className="chat-column">
        <div className="chat-empty work-empty"><DigitalRain /><span className="chat-eyebrow">A space for getting things done</span><h1>What would you like to make?</h1><p>Bring a folder and a brief. Follow the plan, review requests, and find the finished files here.</p>

        </div>
        <form className="work-composer" aria-label="New task" onSubmit={e => { e.preventDefault(); void create(true); }} onDragOver={e => { e.preventDefault(); setDragOver(true); }} onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragOver(false); }} onDrop={e => { e.preventDefault(); setDragOver(false); }}>
          <ComposerShell reasoning={{ modelName: formatModelRef(settings, model).name, models: models.map(m => ({ id: m.key, name: m.displayName || m.id, provider: m.provider })), selectedModelId: model, defaultModelId: settings.agentPolicy?.defaultModel ? settings.agentPolicy.defaultModel.providerId + "/" + settings.agentPolicy.defaultModel.modelId : undefined, onSelectModel: setModel, capability, value: reasoning.requested, onChange: reasoning.onChange, disabled: busy, onOpenSettings }} toolbar={<div className="work-fields"><ComposerAccessControl value={access} onChange={setAccess} disabled={busy} /><label className="composer-chip"><Folder aria-hidden /><select aria-label="Space" value={selectedSpace} disabled={busy} onChange={e => { setSpaceId(e.target.value); setAttachments([]); }}><option value="" disabled>Choose a space</option>{writableSpaces.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>


          </div>} menu={<><button className="ghost icon-button" type="button" aria-label="Queue task" title={!selectedSpace ? "Choose a space first" : !brief.trim() ? "Describe the task to queue" : "Queue task"} disabled={busy || dictating || !brief.trim() || !selectedSpace || !model} onClick={() => { void create(false); }}>Queue task</button><button type="button" disabled={busy || !selectedSpace} title={!selectedSpace ? "Choose a space first" : "Attach files to this space"} onClick={() => { void run(async () => { const paths = await pickWorkAttachments(); if (paths.length) await attach(paths); }); }}>Attach files to space</button></>} actions={<><ComposerDictation settings={settings} disabled={busy} onBusyChange={setDictating} onTranscript={text => setBrief(current => current + (current && !/\s$/.test(current) ? " " : "") + text)} onError={work.setError} onOpenSettings={onOpenSettings} /><ComposerSendButton label="Run now" disabled={busy || dictating || !brief.trim() || !selectedSpace || !model} disabledReason={dictating ? "Finish dictation first" : busy ? "Please wait" : !selectedSpace ? "Choose a space first" : !model ? "Choose a model in Settings" : "Describe the result you want"} onSend={() => { void create(true); }} /></>}>
          {dragOver && <div className="composer-drop-overlay">Drop attachments into this space</div>}
          <ul className="chat-attachments">{attachments.map(path => <li key={path}><img src={getFileTypeIconUrl(path)} alt="" />{path.split("/").pop()}<button type="button" aria-label={"Remove " + path} onClick={() => setAttachments(current => current.filter(p => p !== path))}><X aria-hidden /></button></li>)}</ul>
          <label className="work-brief"><textarea aria-label="Task brief" placeholder="Describe the result you want…" rows={1} value={brief} disabled={busy} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void create(true); } }} onChange={e => setBrief(e.target.value)} /></label>

          </ComposerShell>
        </form>
          <div className="chat-examples work-templates starter-pills">{templates.map(template => <button key={template.title} onClick={() => { setBrief(template.brief); setAccess(template.access); }}>{createElement(templateIcons[template.title], { "aria-hidden": true })}{template.title}</button>)}</div>
      </div></div>}
    </main>
    {features.checkpoints && <Checkpoints requestCheckpoint={checkpoints} running={Object.fromEntries(work.state.tasks.filter(t => t.threadId).map(t => [t.threadId!, { isProcessing: ["running","needs-input"].includes(t.status) }]))} onChanged={async (_space, thread) => { const task = tasksRef.current.find(t => t.threadId === thread); if (task) await work.read(task); await work.refresh(); }} />}
  </div></WorkspaceShell>;
}

