import { useCallback, useEffect, useState } from "react";
import type { EngineKind, ThreadSummary, WorkspaceInfo } from "@/types";
import { createSideChat } from "@services/tauri";

export type OpenSideChat = { id: string; parentId: string; workspaceId: string; title: string; engine: EngineKind; model: string | null };

export function sideChatQuote(title: string, text: string) {
  return `From side chat: ${title.replace(/[\r\n]/g, " ")}\n${text.split("\n").map(line => `> ${line}`).join("\n")}\n`;
}

export function useSideChats({ workspace, threadId, threads, engine, model, refresh, list }: {
  workspace: WorkspaceInfo | null; threadId: string | null; threads: ThreadSummary[];
  engine: EngineKind; model: string | null;
  refresh: (workspaceId: string, threadId: string) => Promise<unknown>;
  list: (workspace: WorkspaceInfo) => Promise<unknown>;
}) {
  const [opened, setOpened] = useState<OpenSideChat | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const open = useCallback((thread: ThreadSummary) => {
    if (!thread.sideOf) return;
    setOpened({ id: thread.id, parentId: thread.sideOf.threadId, workspaceId: thread.sideOf.workspaceId, title: thread.name, engine: thread.engine ?? engine, model: thread.modelId ? (thread.modelId.includes("/") ? thread.modelId : `${thread.modelProvider}/${thread.modelId}`) : model });
    void refresh(thread.sideOf.workspaceId, thread.id);
  }, [engine, model, refresh]);
  const create = useCallback(async (atItemId?: string) => {
    if (!workspace || !threadId || creating) return;
    setCreating(true); setError(null);
    try {
      const response = await createSideChat(workspace.id, threadId, atItemId);
      const child = response.result.thread;
      setOpened({ id: child.id, parentId: threadId, workspaceId: workspace.id, title: "Side chat", engine, model });
      await refresh(workspace.id, child.id);
      await list(workspace);
    } catch (e) { setError(String(e)); }
    finally { setCreating(false); }
  }, [workspace, threadId, creating, engine, model, refresh, list]);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && !event.altKey && event.key.toLowerCase() === "k" && !event.defaultPrevented && threadId) {
        event.preventDefault(); void create();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [create, threadId]);
  const current = opened?.parentId === threadId && opened.workspaceId === workspace?.id ? opened : null;
  const title = current && threads.find(t => t.id === current.id)?.name;
  return { current: current ? { ...current, title: title ?? current.title } : null, create, open, close: () => setOpened(null), error, creating };
}
