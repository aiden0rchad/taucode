import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { managedTasksRequest } from "@services/tauri";

export type ManagedTask = {
  id: string;
  workspaceId: string;
  threadId: string;
  command: string;
  label: string;
  cwd: string;
  pid: number | null;
  status: "running" | "completed" | "failed" | "stopped";
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  output: string;
  outputTruncated: boolean;
  urls: string[];
};
export type ManagedServer = {
  id: string;
  taskId: string;
  url: string;
  title: string;
  status: ManagedTask["status"];
  startedAt: number;
};

export type ManagedTasksTransport = (workspaceId: string, threadId: string, action: string, params: Record<string, unknown>) => Promise<ManagedTask[]>;
export function useManagedTasks(workspaceId: string | null, threadId: string | null, transport: ManagedTasksTransport = managedTasksRequest) {
  const key = JSON.stringify([workspaceId, threadId]);
  const scope = useRef(key);
  scope.current = key;
  const sequence = useRef(0);
  const mutating = useRef(new Set<string>());
  const mounted = useRef(true);
  const [state, setState] = useState({ key, tasks: [] as ManagedTask[], error: "", busy: false });
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const request = useCallback(async (action: string, params: Record<string, unknown> = {}) => {
    if (!workspaceId || !threadId) throw new Error("Select a workspace session to run a task");
    const mutation = action !== "list";
    if (mutating.current.has(key)) {
      if (!mutation) return;
      throw new Error("A task action is already in progress");
    }
    if (mutation) mutating.current.add(key);
    const version = ++sequence.current;
    if (mutation) setState(previous => ({ key, tasks: previous.key === key ? previous.tasks : [], error: "", busy: true }));
    try {
      const tasks = await transport(workspaceId, threadId, action, params);
      if (mounted.current && scope.current === key && sequence.current === version) {
        setState(previous => ({ key, tasks, error: "", busy: mutation ? false : previous.key === key && previous.busy }));
      }
    } catch (error) {
      if (mounted.current && scope.current === key && sequence.current === version) {
        setState(previous => ({ key, tasks: previous.key === key ? previous.tasks : [], error: String(error), busy: false }));
      }
      throw error;
    } finally {
      if (mutation) mutating.current.delete(key);
      if (mutation && mounted.current && scope.current === key) setState(previous => ({ ...previous, busy: false }));
    }
  }, [key, workspaceId, threadId, transport]);
  const refresh = useCallback(() => request("list"), [request]);
  useEffect(() => {
    setState({ key, tasks: [], error: "", busy: false });
    if (!workspaceId || !threadId) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try { await refresh(); } catch { /* Error is shown; the next poll can recover. */ }
      if (!cancelled) timer = setTimeout(() => { void poll(); }, document.hidden ? 10000 : 2000);
    };
    void poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [key, workspaceId, threadId, refresh]);
  const tasks = useMemo(() => state.key === key ? state.tasks : [], [state.key, state.tasks, key]);
  const servers = useMemo<ManagedServer[]>(() => tasks.flatMap(task => task.urls.map(url => ({
    id: `${task.id}:${url}`, taskId: task.id, url, title: task.label, status: task.status, startedAt: task.startedAt,
  }))), [tasks]);
  return {
    tasks, servers, error: state.key === key ? state.error : "", busy: state.key === key && state.busy,
    canStart: Boolean(workspaceId && threadId), refresh,
    start: useCallback((command: string, label?: string) => request("start", { command, label }), [request]),
    stop: useCallback((id: string) => request("stop", { id }), [request]),
    restart: useCallback((id: string) => request("restart", { id }), [request]),
    clearFinished: useCallback(() => request("clearFinished"), [request]),
  };
}
export type ManagedTasksController = ReturnType<typeof useManagedTasks>;
