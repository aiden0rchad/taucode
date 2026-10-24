import { useCallback, useEffect, useRef, useState } from "react";
import type { AppSettings, WorkRequest, WorkTask } from "@/types";
import { checkpointRequest, pickWorkspacePath, pickWorkspacePaths, pickWorkAttachments, workAttachment, workOutput, workRequest } from "@services/tauri";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { Markdown } from "@/features/messages/components/Markdown";
import { Checkpoints, RewindAction } from "@/features/checkpoints/Checkpoints";
import { ModeSwitcher } from "@/features/chat/ModeShell";
import { templates, useWork, workGroups } from "./useWork";
import "./work.css";

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

export function WorkView({ settings }: { settings: AppSettings }) {
  const work = useWork();
  const [brief, setBrief] = useState("");
  const [spaceId, setSpaceId] = useState("");
  const [access, setAccess] = useState<WorkTask["accessMode"]>("ask");
  const models = (settings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ key: p.id + "/" + m.id, label: p.name + " · " + (m.displayName || m.id) })));
  const [model, setModel] = useState(models[0]?.key ?? "");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [showList, setShowList] = useState(true);
  const [folder, setFolder] = useState("");
  const [readOnlyFolders, setReadOnlyFolders] = useState<string[]>([]);
  const tasksRef = useRef(work.state.tasks);
  tasksRef.current = work.state.tasks;
  const writableSpaces = work.state.spaces.filter(s => !s.readOnly);
  const selectedSpace = writableSpaces.find(s => s.id === spaceId)?.id || writableSpaces[0]?.id || "";
  const needs = work.state.tasks.filter(t => t.status === "needs-input");
  const task = work.task;
  const continuationSpace = writableSpaces.find(s => s.id === spaceId)?.id || writableSpaces.find(s => s.id === task?.spaceId)?.id || selectedSpace;
  const active = task?.status === "running" || task?.status === "needs-input";
  const nativeDrop = useRef<(paths: string[]) => void>(() => {});
  useEffect(() => subscribeWindowDragDrop(event => { if (event.payload.type === "drop") nativeDrop.current(event.payload.paths ?? []); }), []);
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
    const created = await work.mutate<WorkTask>("create", { spaceId: selectedSpace, brief, model, accessMode: access, attachments, runNow });
    work.setActive(created.id); setBrief(""); setAttachments([]); setShowList(false);
  });
  const selectTask = (id: string) => { work.setActive(id); setSpaceId(""); setShowList(false); };
  const taskButton = (entry: WorkTask) => <button className="work-task" key={entry.id} aria-current={work.active === entry.id ? "page" : undefined} onClick={() => selectTask(entry.id)}><span className={"work-status " + entry.status} aria-label={entry.status} />{entry.title}<small>{entry.status.replace("-", " ")}</small></button>;
  const checkpoints = useCallback<typeof checkpointRequest>((_space, params) => {
    const task = tasksRef.current.find(t => t.threadId === params.threadId);
    return workRequest("checkpoint", { taskId: task?.id, request: params });
  }, []);
  return <div className={"chat-layout work-layout" + (showList ? " chat-list-open" : "")}>
    <aside className="chat-sidebar" aria-label="Spaces and tasks">
      <ModeSwitcher />
      <button onClick={() => { work.setActive(null); setShowList(false); }}>＋ New task</button>
      <div className="chat-list">
        <section aria-label="Needs input inbox" className="work-inbox"><h2>Needs input · {needs.length}</h2>{needs.map(taskButton)}{!needs.length && <p className="chat-muted">You’re all caught up.</p>}</section>
        {work.state.spaces.map(space => <section className="work-space" key={space.id} aria-label={space.name}>
          <h2 title={space.path}>{space.name}</h2>
          {space.readOnly && <p className="chat-muted">Read-only history. Choose an available space to continue.</p>}
          {!!space.readOnlyFolders.length && <details><summary>{space.readOnlyFolders.length} read-only folders</summary>{space.readOnlyFolders.map(path => <p key={path} className="chat-muted">{path}</p>)}</details>}
          {workGroups.map(([label, statuses]) => {
            const entries = work.state.tasks.filter(t => t.spaceId === space.id && (statuses as readonly string[]).includes(t.status));
            return !!entries.length && <section key={label} aria-label={space.name + " " + label}><h3>{label}</h3>{entries.map(taskButton)}</section>;
          })}
        </section>)}
      </div>
      <button disabled={busy} onClick={() => { void run(async () => { const path = await pickWorkspacePath(); if (path) { setFolder(path); setReadOnlyFolders([]); work.setActive(null); setShowList(false); } }); }}>Add space…</button>
      <label className="work-concurrency">Concurrent tasks <select aria-label="Concurrent tasks" value={work.state.concurrency} disabled={busy} onChange={e => { void run(() => work.mutate("concurrency", { count: Number(e.target.value) })); }}>{[1,2,3,4,5,6,7,8].map(n => <option key={n}>{n}</option>)}</select></label>
      <button disabled={busy || !work.state.tasks.some(t => ["queued","running","needs-input"].includes(t.status))} onClick={() => { void run(() => work.mutate("stop-all")); }}>Stop all tasks</button>
    </aside>
    <main className="chat-main">
      <header className="chat-header"><button className="chat-list-toggle" aria-expanded={showList} onClick={() => setShowList(!showList)}>☰ Spaces</button><strong>{task?.title || "New task"}</strong>
        {task && <div className="work-actions"><span role="status">{task.status.replace("-", " ")}</span>{(active || task.status === "queued") && <button disabled={busy} onClick={() => { void run(() => work.mutate("stop", { taskId: task.id })); }}>Stop task</button>}
          {task.threadId && !task.importId && <RewindAction workspaceId={task.spaceId} threadId={task.threadId} running={active} />}
        </div>}
      </header>
      {work.error && <p role="alert" className="work-error">{work.error}</p>}
      {folder && <section className="work-add-space" aria-label="New space"><h2>New space</h2><p>{folder}</p><p className="chat-muted">This is the folder where tasks can create files. Add other folders for reading only.</p>
        {readOnlyFolders.map(path => <p key={path}>{path}<button onClick={() => setReadOnlyFolders(current => current.filter(p => p !== path))}>Remove</button></p>)}
        <div className="work-actions"><button disabled={busy} onClick={() => { void run(async () => { const paths = await pickWorkspacePaths(); setReadOnlyFolders(current => [...new Set([...current, ...paths])]); }); }}>Add read-only folders…</button>
          <button disabled={busy} onClick={() => { void run(async () => { const space = await work.mutate<{ id: string }>("add-space", { path: folder, readOnlyFolders }); setSpaceId(space.id); setAttachments([]); setFolder(""); }); }}>Create space</button>
          <button onClick={() => setFolder("")}>Cancel</button></div>
      </section>}
      {task ? <div className="work-task-view">
        <div className="chat-scroll" role="log" aria-label="Task progress" aria-live="polite"><div className="chat-column">
          <article className="chat-message user"><h2>Task brief</h2><Markdown value={task.brief} showFilePath={false} /><ul>{task.attachments.map(path => <li key={path}>{path}</li>)}</ul></article>
          {task.pending.map(request => <InputRequest key={request.id} request={request} busy={busy} respond={answer => { void run(() => work.mutate("respond", { taskId: task.id, requestId: request.id, answer })); }} />)}
          {task.error && <p role="alert">{task.error}</p>}
          {work.items.map(item => item.kind === "message" ? <article className={"chat-message " + item.role} key={item.id}><h2>{item.role === "user" ? "You" : "Assistant"}</h2><Markdown value={item.text} codeBlock showFilePath={false} /></article> : item.kind === "tool" ? <details className="work-tool" key={item.id}><summary>{item.title} · {item.status}</summary><pre>{item.detail}</pre>{item.output && <Markdown value={item.output} showFilePath={false} />}</details> : item.kind === "reasoning" ? <details key={item.id}><summary>Thinking</summary><Markdown value={item.content || item.summary} showFilePath={false} /></details> : null)}
          {task.status === "queued" && <p className="chat-muted">Waiting for an available slot. One task runs in each space at a time.</p>}
          {active && <p className="chat-muted">This task keeps running when you switch modes or hide the window.</p>}
          {task.importId ? <section aria-label="Continue imported task">
            <p className="chat-muted">Read-only Cowork history. Continue starts a new task with summarized context and the last 10 messages.</p>
            <label>Space <select aria-label="Continuation space" value={continuationSpace} onChange={e => setSpaceId(e.target.value)} disabled={busy}><option value="" disabled>Choose an available space</option>{writableSpaces.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
            <label>Model <select aria-label="Continuation model" value={model} onChange={e => setModel(e.target.value)} disabled={busy}>{!models.length && <option value="">Enable a provider in Code → Settings</option>}{models.map(m => <option key={m.key} value={m.key}>{m.label}</option>)}</select></label>
            <button disabled={busy || !selectedSpace || !model} onClick={() => { void run(async () => {
              const created = await work.mutate<WorkTask>("continue", { taskId: task.id, spaceId: continuationSpace, model });
              work.setActive(created.id);
            }); }}>Continue</button>
          </section> : !active && task.status !== "queued" && <button onClick={() => { setBrief("Continue this task:\n" + task.brief + "\n\nReviewed plan:\n" + task.plan.map(p => "- " + p.step).join("\n") + "\n\nAsk before changing any existing files."); setSpaceId(task.spaceId); setAccess("ask"); setModel(task.model); work.setActive(null); }}>Continue as a new task…</button>}
        </div></div>
        <aside className="work-inspector" aria-label="Task details">
          <section aria-label="Plan"><h2>Plan</h2>{task.plan.length ? <ul className="work-plan">{task.plan.map((step, index) => <li key={index}><input type="checkbox" aria-label={step.step} checked={step.status === "completed"} readOnly /><span>{step.step}</span></li>)}</ul> : <p className="chat-muted">The task’s checklist will appear here.</p>}</section>
          <section aria-label="Outputs"><h2>Outputs <small>{task.outputs.length}</small></h2>{!task.outputs.length && <p className="chat-muted">Created and modified files will appear here.</p>}
            {task.outputs.map(path => <div className="work-output" key={path}><strong>{path}</strong><div className="work-actions">{(["open", "reveal", ...(/\.(png|jpe?g|gif|webp|pdf)$/i.test(path) ? ["preview"] : [])] as const).map(action => <button key={action} disabled={busy} onClick={() => { void run(() => workOutput(task.id, path, action as "open" | "reveal" | "preview")); }}>{action === "open" ? "Open" : action === "reveal" ? "Reveal in Finder" : "Quick Look"}</button>)}</div></div>)}
          </section>
        </aside>
      </div> : <div className="chat-scroll"><div className="chat-column">
        <div className="chat-empty work-empty"><span className="chat-eyebrow">A space for getting things done</span><h1>What would you like to make?</h1><p>Bring a folder and a brief. Follow the plan, review requests, and find the finished files here.</p>
          <div className="chat-examples work-templates">{templates.map(template => <button key={template.title} onClick={() => { setBrief(template.brief); setAccess(template.access); }}>{template.title}<span aria-hidden>↗</span></button>)}</div>
        </div>
        <form className="work-composer" aria-label="New task" onSubmit={e => { e.preventDefault(); void create(true); }} onDragOver={e => e.preventDefault()}>
          <div className="work-fields"><label>Space <select aria-label="Space" value={selectedSpace} disabled={busy} onChange={e => { setSpaceId(e.target.value); setAttachments([]); }}><option value="" disabled>Choose a space</option>{writableSpaces.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
            <label>Model <select aria-label="Work model" value={model} disabled={busy} onChange={e => setModel(e.target.value)}>{!models.length && <option value="">Enable a provider in Code → Settings</option>}{models.map(m => <option key={m.key} value={m.key}>{m.label}</option>)}</select></label>
            <label>Access <select aria-label="Access mode" value={access} onChange={e => setAccess(e.target.value as WorkTask["accessMode"])}><option value="ask">Ask</option><option value="read-only">Read only</option><option value="full-access">Full access</option></select></label>
          </div>
          <label className="work-brief">Task brief<textarea aria-label="Task brief" placeholder="Describe the result you want…" value={brief} onChange={e => setBrief(e.target.value)} /></label>
          <p className="chat-muted">Drop attachments here. Files are copied into this space. Replacing existing files outside outputs/ always needs approval.</p>
          <ul className="chat-attachments">{attachments.map(path => <li key={path}>{path.split("/").pop()}<button type="button" aria-label={"Remove " + path} onClick={() => setAttachments(current => current.filter(p => p !== path))}>×</button></li>)}</ul>
          <div className="work-actions"><button type="button" disabled={busy || !selectedSpace} onClick={() => { void run(async () => { const paths = await pickWorkAttachments(); if (paths.length) await attach(paths); }); }}>Attach files…</button><span />
            <button type="button" disabled={busy || !brief.trim() || !selectedSpace || !model} onClick={() => { void create(false); }}>Queue</button><button type="submit" disabled={busy || !brief.trim() || !selectedSpace || !model}>Run now</button>
          </div>
        </form>
      </div></div>}
    </main>
    <Checkpoints requestCheckpoint={checkpoints} running={Object.fromEntries(work.state.tasks.filter(t => t.threadId).map(t => [t.threadId!, { isProcessing: ["running","needs-input"].includes(t.status) }]))} onChanged={async (_space, thread) => { const task = tasksRef.current.find(t => t.threadId === thread); if (task) await work.read(task); await work.refresh(); }} />
  </div>;
}

