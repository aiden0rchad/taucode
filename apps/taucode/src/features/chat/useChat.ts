import { threadEffortKey } from "@/features/models/reasoning";
import { useCallback, useEffect, useRef, useState } from "react";
import { chatRequest } from "@services/tauri";
import { subscribeAppServerEvents } from "@services/events";
import { buildConversationItem, buildItemsFromThread, previewThreadName, upsertItem, mergeThreadItems } from "@utils/threadItems";
import { clampThreadName } from "@threads/utils/threadNaming";
import type { Chat, ChatMessage, ChatResponse, ChatThread } from "./chat";

export function useChat() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, ChatMessage[]>>({});
  const [running, setRunning] = useState<Record<string, boolean>>({});
  const [error, setError] = useState("");
  const revisions = useRef<Record<string, number>>({});
  const refresh = useCallback(async () => {
    const list = await chatRequest<Chat[]>("list");
    setChats(list);
    setRunning(current => ({ ...current, ...Object.fromEntries(list.map(c => [c.id, Boolean(c.running)])) }));
  }, []);
  const read = useCallback(async (id: string) => {
    const revision = revisions.current[id] ?? 0;
    const result = await chatRequest<ChatResponse>("read", { threadId: id });
    if (revision === (revisions.current[id] ?? 0)) {
      const items = buildItemsFromThread(result.result.thread).filter((item): item is ChatMessage => item.kind === "message");
      setMessages(current => {
        const remoteIds = new Set(items.map(item => item.id));
        const merged = mergeThreadItems(items, current[id] ?? []).filter(item => remoteIds.has(item.id) || item.id.startsWith("optimistic-user-")) as ChatMessage[];
        return { ...current, [id]: merged };
      });
    }
    return result.result.thread;
  }, []);
  useEffect(() => { void refresh().catch(e => setError(String(e))); }, [refresh]);
  useEffect(() => {
    if (import.meta.env.MODE === "uipreview") {
      void import("@/dev/uiPreview").then(({ selectConversation }) => selectConversation("chat", id => {
        setActive(id);
        void read(id).catch(e => setError(String(e)));
      }));
    }
  }, [read]);
  useEffect(() => subscribeAppServerEvents(event => {
    if (event.workspace_id !== "chats") return;
    const method = event.message.method;
    const params = (event.message.params ?? {}) as Record<string, unknown>;
    const id = String(params.threadId ?? "");
    if (!id) return;
    revisions.current[id] = (revisions.current[id] ?? 0) + 1;
    if (method === "item/started" || method === "item/completed") {
      const item = buildConversationItem(params.item as Record<string, unknown>);
      if (item?.kind === "message") setMessages(current => {
        const items = current[id] ?? [];
        return { ...current, [id]: upsertItem(items, item) as ChatMessage[] };
      });
    }
    if (method === "item/agentMessage/delta") setMessages(current => {
      const items = current[id] ?? [];
      const itemId = String(params.itemId);
      const delta = String(params.delta ?? "");
      return { ...current, [id]: items.some(i => i.id === itemId) ? items.map(i => i.id === itemId ? { ...i, text: i.text + delta } : i) : [...items, { id: itemId, kind: "message", role: "assistant", text: delta }] };
    });
    if (method === "turn/started") setRunning(current => ({ ...current, [id]: true }));
    if (method === "error") setError(String((params.error as { message?: string })?.message ?? params.message ?? "Chat failed"));
    if (method === "turn/completed") {
      setRunning(current => ({ ...current, [id]: false }));
      const turn = params.turn as { error?: { message?: string } } | undefined;
      if (turn?.error?.message) setError(turn.error.message);
      void read(id).then(() => refresh()).catch(e => setError(String(e)));
    }
  }), [read, refresh]);
  const open = async (id: string) => { setActive(id); await read(id); };
  const adopt = async (thread: ChatThread) => {
    setActive(thread.id);
    await read(thread.id);
    await refresh();
    return thread.id;
  };
  const fork = async (id: string, model: string | null, beforeUserItemId?: string) => {
    const result = await chatRequest<ChatResponse>("fork", { threadId: id, model, beforeUserItemId });
    return adopt(result.result.thread);
  };
  const send = async (text: string, images: string[], model: string | null, beforeUserItemId?: string, web = false, effort: string | null = null) => {
    let id = active;
    if (!id) {
      const result = await chatRequest<ChatResponse>("start", { model });
      id = await adopt(result.result.thread);
      if (web) await chatRequest("web", { threadId: id, web: true });
    } else if (beforeUserItemId) id = await fork(id, model, beforeUserItemId);
    const title = clampThreadName(previewThreadName(text, "New chat"));
    if (!chats.find(c => c.id === id)?.chat.title && title) await chatRequest("rename", { threadId: id, title });
    const optimisticId = `optimistic-user-${crypto.randomUUID()}`;
    revisions.current[id] = (revisions.current[id] ?? 0) + 1;
    setMessages(current => ({ ...current, [id]: [...(current[id] ?? []), { id: optimisticId, kind: "message", role: "user", text, images }] }));
    setRunning(current => ({ ...current, [id]: true }));
    try {
      if (effort) localStorage.setItem(threadEffortKey(id), effort);
      await chatRequest("send", { threadId: id, ...(effort ? { effort } : {}), input: [{ type: "text", text }, ...images.map(url => ({ type: "image", url }))] });
      await refresh().catch(e => setError(String(e)));
    } catch (e) {
      setMessages(current => ({ ...current, [id]: (current[id] ?? []).filter(item => item.id !== optimisticId) }));
      setRunning(current => ({ ...current, [id]: false })); throw e;
    }
  };
  return { chats, active, setActive, messages: active ? messages[active] ?? [] : [], running: active ? Boolean(running[active]) : false, error, setError, refresh, open, fork, send };
}
