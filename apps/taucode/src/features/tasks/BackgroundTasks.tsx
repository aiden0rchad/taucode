import { useEffect, useState } from "react";
import { ExternalLink, Play, RotateCcw, Square, Terminal } from "lucide-react";
import type { LaunchScriptEntry } from "@/types";
import type { ManagedTask, ManagedTasksController } from "./useManagedTasks";
import "./background-tasks.css";

function elapsed(task: ManagedTask, now: number) {
  const seconds = Math.max(0, Math.floor(((task.endedAt ?? now) - task.startedAt) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
export function BackgroundTasks({ controller, commands = [], commandsEnabled = true, showCommandInput = true, onOpenUrl }: {
  controller: ManagedTasksController;
  commands?: LaunchScriptEntry[];
  commandsEnabled?: boolean;
  showCommandInput?: boolean;
  onOpenUrl: (url: string) => void;
}) {
  const [command, setCommand] = useState("");
  const [now, setNow] = useState(Date.now);
  const running = controller.tasks.filter(task => task.status === "running");
  const finished = controller.tasks.filter(task => task.status !== "running");
  useEffect(() => {
    if (!running.length) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running.length]);
  const run = async (value: string, label?: string) => {
    try { await controller.start(value, label); setCommand(current => current === value ? "" : current); } catch { /* The controller displays the error. */ }
  };
  const canStart = controller.canStart && commandsEnabled;
  const taskRow = (task: ManagedTask) => <div className="managed-task-row" key={task.id}><details className="managed-task">
    <summary>
      <span className={`managed-task-status is-${task.status}`} aria-label={task.status} />
      <span className="managed-task-name">{task.label}</span>
      <span className="managed-task-time">{elapsed(task, now)}</span>
      <span className="managed-task-state">{task.status}{task.exitCode !== null && task.status !== "running" ? ` · ${task.exitCode}` : ""}</span>
    </summary>
    <div className="managed-task-detail">
      <code className="managed-task-command">{task.command}</code>
      <div className="managed-task-meta">{task.cwd}{task.pid ? ` · PID ${task.pid}` : ""}</div>
      {task.status !== "running" && <div className="managed-task-actions">
        <button type="button" className="secondary" disabled={controller.busy || !canStart} onClick={() => { void controller.restart(task.id).catch(() => {}); }}><RotateCcw size={12} /> Restart</button>
      </div>}
      {task.outputTruncated && <p className="managed-task-meta">Showing the latest 128 KB of output.</p>}
      <pre className="managed-task-output" tabIndex={0} aria-label={`Output for ${task.label}`}>{task.output || (task.status === "running" ? "Waiting for output…" : "No output.")}</pre>
    </div>
  </details>{task.status === "running" && <button type="button" className="secondary managed-task-stop" aria-label={`Stop ${task.label}`} disabled={controller.busy} onClick={() => { void controller.stop(task.id).catch(() => {}); }}><Square size={12} /> Stop</button>}</div>;
  return <section className="background-tasks" aria-label="Background tasks">
    {showCommandInput ? <form className="managed-task-form" onSubmit={event => { event.preventDefault(); if (command.trim()) void run(command); }}>
      <label htmlFor="managed-task-command"><Terminal size={14} /> Run a background task</label>
      <div className="managed-task-input-row">
        <input id="managed-task-command" spellCheck={false} autoCorrect="off" autoCapitalize="off" value={command} onChange={event => setCommand(event.target.value)} placeholder={commandsEnabled ? (controller.canStart ? "Command in this session…" : "Select a session to run a task") : "Enable Terminal in Features to start tasks"} disabled={!canStart} />
        <button className="secondary" type="submit" disabled={controller.busy || !canStart || !command.trim()}><Play size={12} /> Run</button>
      </div>
    </form> : <p className="managed-task-empty">Previews started by this task appear here. They keep running until you stop them or quit taucode.</p>}
    {!commandsEnabled && <p className="managed-task-empty">{showCommandInput ? "Enable Terminal in Features to start tasks." : "Restart requires Work and Terminal enabled, and a task with write access. You can still stop previews."}</p>}
    {commands.some(entry => entry.script.trim()) && <div className="managed-task-launchers" aria-label="Configured commands">
      {commands.filter(entry => entry.script.trim()).map(entry => <button type="button" className="secondary" key={entry.id} title={entry.script} disabled={controller.busy || !canStart} onClick={() => { void run(entry.script, entry.label?.trim() || undefined); }}><Play size={12} />{entry.label?.trim() || entry.script}</button>)}
    </div>}
    {controller.error && <p className="managed-task-error" role="alert">{controller.error}</p>}
    {controller.servers.length > 0 && <div className="managed-servers" aria-label="Detected servers">
      <div className="managed-task-heading" title="Addresses detected in task output; server readiness has not been checked.">Servers <span>Addresses from task output</span></div>
      {controller.servers.map(server => <div className="managed-server" key={server.id}>
        <span className={`managed-task-status is-${server.status}`} aria-label={server.status} />
        <button type="button" className="managed-server-url" title={`Open ${server.url} in Browser`} onClick={() => onOpenUrl(server.url)}>{server.url}<ExternalLink size={12} /></button>
        <span className="managed-task-state">{server.status}</span>
      </div>)}
    </div>}
    <details className="managed-task-group" open>
      <summary>Running <span>{running.length}</span></summary>
      {running.length ? running.map(taskRow) : <p className="managed-task-empty">No running tasks.</p>}
    </details>
    <details className="managed-task-group">
      <summary>Finished <span>{finished.length}</span></summary>
      {finished.map(taskRow)}
      {finished.length > 0 && <button type="button" className="ghost managed-task-clear" disabled={controller.busy} onClick={() => { void controller.clearFinished().catch(() => {}); }}>Clear finished</button>}
    </details>
    <p className="managed-task-footnote">History lasts until the backend restarts.</p>
  </section>;
}
