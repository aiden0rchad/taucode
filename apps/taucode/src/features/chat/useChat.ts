import { threadEffortKey } from "@/features/models/reasoning";
import { useCallback, useEffect, useRef, useState } from "react";
import { chatRequest } from "@services/tauri";
import { subscribeAppServerEvents } from "@services/events";
import { buildConversationItem, buildItemsFromThread, previewThreadName, upsertItem, mergeThreadItems } from "@utils/threadItems";
import { clampThreadName } from "@threads/utils/threadNaming";
import type { QueuedMessage } from "@/types";
import type { Chat, ChatMessage, ChatResponse, ChatThread } from "./chat";
import { createQueueStore } from "@/features/composer/queueStore";

export const chatQueues = createQueueStore<QueuedMessage>();

export function useChat() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, ChatMessage[]>>({});
  const [running, setRunning] = useState<Record<string, boolean>>({});
  const [turnStatus, setTurnStatus] = useState<Record<string, string>>({});
  const [turnErrors, setTurnErrors] = useState<Record<string, string>>({});
  const [error, setError] = useState("");
  // Follow-ups written while a response runs, sent one at a time after it completes.
  const [queued, setQueued] = chatQueues.useQueue();
  const flushing = useRef(new Set<string>());
  useEffect(() => setError(""), [active]);
  const recordTurn = useCallback((id: string, turn: { status?: string; error?: { message?: string } } | undefined) => {
    if (!turn?.status) return;
    setTurnStatus(current => ({ ...current, [id]: turn.status! }));
    setTurnErrors(current => ({ ...current, [id]: turn.error?.message ?? "" }));
  }, []);
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
      const turns = result.result.thread.turns;
      if (Array.isArray(turns)) recordTurn(id, turns[turns.length - 1]);
      const items = buildItemsFromThread(result.result.thread).filter((item): item is ChatMessage => item.kind === "message");
      setMessages(current => {
        const remoteIds = new Set(items.map(item => item.id));
        const merged = mergeThreadItems(items, current[id] ?? []).filter(item => remoteIds.has(item.id) || item.id.startsWith("optimistic-user-")) as ChatMessage[];
        return { ...current, [id]: merged };
      });
    }
    return result.result.thread;
  }, [recordTurn]);
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
    if (method === "turn/started") {
      setRunning(current => ({ ...current, [id]: true }));
      recordTurn(id, { status: "inProgress" });
    }
    if (method === "error") setTurnErrors(current => ({ ...current, [id]: String((params.error as { message?: string })?.message ?? params.message ?? "Chat failed") }));
    if (method === "turn/completed") {
      setRunning(current => ({ ...current, [id]: false }));
      recordTurn(id, params.turn as { status?: string; error?: { message?: string } } | undefined);
      void read(id).then(() => refresh()).catch(e => setTurnErrors(current => ({ ...current, [id]: String(e) })));
    }
  }), [read, refresh, recordTurn]);
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
    await deliver(id, text, images, effort);
  };
  const optimistic = (id: string, text: string, images: string[]) => {
    const optimisticId = `optimistic-user-${crypto.randomUUID()}`;
    revisions.current[id] = (revisions.current[id] ?? 0) + 1;
    setMessages(current => ({ ...current, [id]: [...(current[id] ?? []), { id: optimisticId, kind: "message", role: "user", text, images }] }));
    return () => setMessages(current => ({ ...current, [id]: (current[id] ?? []).filter(item => item.id !== optimisticId) }));
  };
  const input = (text: string, images: string[]) => [{ type: "text", text }, ...images.map(url => ({ type: "image", url }))];
  const deliver = async (id: string, text: string, images: string[], effort: string | null) => {
    const undo = optimistic(id, text, images);
    setRunning(current => ({ ...current, [id]: true }));
    recordTurn(id, { status: "inProgress" });
    try {
      if (effort) localStorage.setItem(threadEffortKey(id), effort);
      await chatRequest("send", { threadId: id, ...(effort ? { effort } : {}), input: input(text, images) });
      await refresh().catch(e => setError(String(e)));
    } catch (e) {
      undo();
      setRunning(current => ({ ...current, [id]: false }));
      recordTurn(id, { status: "failed", error: { message: String(e) } });
      throw e;
    }
  };
  const queue = (text: string, images: string[]) => {
    if (!active) return;
    const item = { id: crypto.randomUUID(), text, images, createdAt: Date.now() };
    setQueued(current => ({ ...current, [active]: [...(current[active] ?? []), item] }));
  };
  const unqueue = (itemId: string) => {
    if (active) setQueued(current => ({ ...current, [active]: (current[active] ?? []).filter(item => item.id !== itemId) }));
  };
  /** Adds the message to the running response, or queues it if the response just ended. */
  const steer = async (text: string, images: string[]) => {
    if (!active) return "queued";
    const undo = optimistic(active, text, images);
    try {
      await chatRequest("steer", { threadId: active, input: input(text, images) });
      return "steered";
    } catch (e) {
      undo();
      if (!/No running|turn changed/.test(String(e))) throw e;
      queue(text, images);
      return "queued";
    }
  };
  useEffect(() => {
    for (const [id, items] of Object.entries(queued)) {
      // A stopped or failed response pauses the queue until the next one completes.
      if (!items.length || running[id] || turnStatus[id] !== "completed" || flushing.current.has(id)) continue;
      const [next, ...rest] = items;
      flushing.current.add(id);
      setQueued(current => ({ ...current, [id]: rest }));
      void deliver(id, next.text, next.images ?? [], localStorage.getItem(threadEffortKey(id)))
        .catch(e => {
          setQueued(current => ({ ...current, [id]: [next, ...(current[id] ?? [])] }));
          setTurnErrors(current => ({ ...current, [id]: String(e) }));
        })
        .finally(() => flushing.current.delete(id));
    }
  });
  const pausedReason = active && queued[active]?.length && !running[active] && turnStatus[active] && turnStatus[active] !== "completed"
    ? turnStatus[active] === "interrupted" ? "Paused because the response was stopped" : "Paused because the response failed"
    : null;
  return { chats, active, setActive, messages: active ? messages[active] ?? [] : [], running: active ? Boolean(running[active]) : false, turnStatus: active ? turnStatus[active] : undefined, error: error || (active ? turnErrors[active] ?? "" : ""), setError, refresh, open, fork, send, queue, unqueue, steer, queued: active ? queued[active] ?? [] : [], pausedReason };
}
