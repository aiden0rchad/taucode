import { useCallback, useEffect, useRef, useState } from "react";
import type { ConversationItem, WorkState, WorkTask } from "@/types";
import { workRequest } from "@services/tauri";
import { subscribeAppServerEvents, subscribeImportsChanged } from "@services/events";
import { buildConversationItem, buildItemsFromThread } from "@utils/threadItems";

export const workGroups = [
  ["Running", ["running"]], ["Needs input", ["needs-input"]], ["Queued", ["queued"]], ["Done", ["done", "failed", "stopped"]],
] as const;
export const templates = [
  { title: "Organize a folder", brief: "Inspect this folder, propose an organization plan, then organize it. Ask before moving or replacing existing files.", access: "ask" },
  { title: "Research brief", brief: "Research the topic below and write a two-page brief in outputs/ with sources and a concise summary:\n", access: "ask" },
  { title: "Spreadsheet from documents", brief: "Read the documents in this space and create a summary spreadsheet in outputs/. Include source filenames and verify totals.", access: "ask" },
  { title: "Slide deck from notes", brief: "Turn the notes in this space into a clear slide deck in outputs/. Start with an outline, then create the slides.", access: "ask" },
  { title: "Clean up downloads", brief: "Read-only first: inspect this folder and propose a cleanup checklist. Do not move, overwrite, or delete anything. Wait for approval in a separate follow-up task.", access: "read-only" },
] as const;
export function useWork() {
  const [state, setState] = useState<WorkState>({ spaces: [], tasks: [], concurrency: 2 });
  const [active, setActive] = useState<string | null>(null);
  const [items, setItems] = useState<ConversationItem[]>([]);
  const [error, setError] = useState("");
  const activeRef = useRef<WorkTask | undefined>(undefined);
  activeRef.current = state.tasks.find(t => t.id === active);
  const revision = useRef(0);
  const refresh = useCallback(async () => setState(await workRequest<WorkState>("list")), []);
  const read = useCallback(async (task: WorkTask) => {
    if (!task.threadId) { setItems([]); return; }
    const version = revision.current;
    const result = await workRequest<{ result: { thread: Record<string, unknown> } }>("read", { taskId: task.id });
    if (activeRef.current?.id === task.id && version === revision.current) setItems(buildItemsFromThread(result.result.thread));
  }, []);
  useEffect(() => { void refresh().catch(e => setError(String(e))); }, [refresh]);
  useEffect(() => subscribeImportsChanged(() => { void refresh().catch(e => setError(String(e))); }), [refresh]);
  useEffect(() => {
    setItems([]);
    revision.current++;
    if (activeRef.current) void read(activeRef.current).catch(e => setError(String(e)));
  }, [active, read]);
  useEffect(() => subscribeAppServerEvents(event => {
    const method = event.message.method;
    const params = (event.message.params ?? {}) as Record<string, unknown>;
    if (method === "work/error") setError(String(params.message));
    if (method === "work/updated") {
      void refresh().catch(e => setError(String(e)));
      return;
    }
    const task = activeRef.current;
    if (!task || task.spaceId !== event.workspace_id || task.threadId !== params.threadId) return;
    revision.current++;
    if (method === "item/started" || method === "item/completed") {
      const item = buildConversationItem(params.item as Record<string, unknown>);
      if (item) setItems(current => current.some(i => i.id === item.id) ? current.map(i => i.id === item.id ? item : i) : [...current, item]);
    }
    if (method === "item/agentMessage/delta") {
      const id = String(params.itemId);
      const delta = String(params.delta ?? "");
      setItems(current => current.some(i => i.id === id) ? current.map(i => i.id === id && i.kind === "message" ? { ...i, text: i.text + delta } : i) : [...current, { id, kind: "message", role: "assistant", text: delta }]);
    }
    if (method === "turn/completed") void read(task).catch(e => setError(String(e)));
  }), [read, refresh]);
  // A queued selection acquires its thread when the backend starts it.
  const threadId = state.tasks.find(t => t.id === active)?.threadId;
  useEffect(() => {
    if (threadId && activeRef.current) void read(activeRef.current).catch(e => setError(String(e)));
  }, [threadId, read]);
  const mutate = async <T,>(action: string, params: Record<string, unknown> = {}) => {
    const result = await workRequest<T>(action, params);
    await refresh();
    return result;
  };
  return { state, active, setActive, task: state.tasks.find(t => t.id === active), items, error, setError, refresh, read, mutate };
}

