// @vitest-environment jsdom
import { useReducer, useRef } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppServerEvent, DebugEntry, TurnPlan } from "@/types";
import { useAppServerEvents } from "@app/hooks/useAppServerEvents";
import { isApprovalRequestMethod, isSupportedAppServerMethod } from "@utils/appServerEvents";
import { buildConversationItemFromThreadItem } from "@utils/threadItems";
import { useThreadEventHandlers } from "./useThreadEventHandlers";
import { initialState, threadReducer } from "./useThreadsReducer";

let receive: (event: AppServerEvent) => void;
vi.mock("@services/events", () => ({
  subscribeAppServerEvents: (listener: typeof receive) => { receive = listener; return () => {}; },
}));
vi.mock("@services/tauri", () => ({ respondToServerRequest: vi.fn() }));

function useReplay(onDebug: (entry: DebugEntry) => void) {
  const [state, dispatch] = useReducer(threadReducer, { ...initialState, maxItemsPerThread: null });
  const stateRef = useRef(state);
  stateRef.current = state;
  const handlers = useThreadEventHandlers({
    activeThreadId: null, dispatch,
    getItemsForThread: (id) => stateRef.current.itemsByThread[id] ?? [],
    planByThreadRef: useRef<Record<string, TurnPlan | null>>({}),
    getCustomName: () => undefined, isThreadHidden: () => false, setThreadLoaded: () => {},
    markProcessing: (threadId, isProcessing) => dispatch({ type: "markProcessing", threadId, isProcessing, timestamp: Date.now() }),
    markReviewing: (threadId, isReviewing) => dispatch({ type: "markReviewing", threadId, isReviewing }),
    setActiveTurnId: (threadId, turnId) => dispatch({ type: "setActiveTurnId", threadId, turnId }),
    getActiveTurnId: (id) => stateRef.current.activeTurnIdByThread[id] ?? null,
    safeMessageActivity: () => {}, recordThreadActivity: () => {}, pushThreadErrorMessage: () => {},
    onDebug, onWorkspaceConnected: () => {}, applyCollabThreadLinks: () => {},
    approvalAllowlistRef: useRef({}), pendingInterruptsRef: useRef(new Set<string>()),
  });
  useAppServerEvents(handlers);
  return state;
}

afterEach(cleanup);
it("renders a pi thread through canonical streaming, model, question and compaction events", () => {
  const { result } = renderHook(() => useReplay(vi.fn()));
  const send = (method: string, params: Record<string, unknown>) => act(() => receive({
    workspace_id: "pi-workspace", message: { method, params },
  }));
  send("thread/started", { thread: { id: "pi-session", engine: "pi", modelProvider: "first", turns: [] } });
  send("turn/started", { threadId: "pi-session", turn: { id: "pi-turn", status: "inProgress", items: [] } });
  const item = { type: "agentMessage", id: "pi-message", text: "", phase: null, memoryCitation: null, delivery: null, questions: null };
  send("item/started", { threadId: "pi-session", turnId: "pi-turn", item });
  send("item/agentMessage/delta", { threadId: "pi-session", turnId: "pi-turn", itemId: "pi-message", delta: "Hello from pi" });
  send("item/completed", { threadId: "pi-session", turnId: "pi-turn", item: { ...item, text: "Hello from pi" } });
  send("thread/settings/updated", { threadId: "pi-session", threadSettings: { modelProvider: "second", model: "model-2" } });
  send("item/completed", { threadId: "pi-session", turnId: "pi-turn", item: { type: "contextCompaction", id: "pi-compact" } });
  send("turn/completed", { threadId: "pi-session", turn: { id: "pi-turn", status: "completed", items: [] } });
  expect(result.current.threadsByWorkspace["pi-workspace"][0]).toMatchObject({ engine: "pi", modelProvider: "second" });
  expect(result.current.itemsByThread["pi-session"]).toEqual(expect.arrayContaining([
    expect.objectContaining({ text: "Hello from pi" }), expect.objectContaining({ toolType: "contextCompaction" }),
  ]));
  expect(result.current.threadStatusById["pi-session"].isProcessing).toBe(false);
});
const fixtures = import.meta.glob<string>("/src-tauri/tests/fixtures/codex-0.164/*.jsonl", {
  query: "?raw", import: "default", eager: true,
});

describe("0.164 recorded frontend traffic", () => {
  it.each(Object.entries(fixtures))("replays %s", (path, contents) => {
    const name = path.split("/").pop();
    const debug = vi.fn();
    const { result } = renderHook(() => useReplay(debug));
    const messages = contents.trim().split("\n")
      .map((line) => JSON.parse(line)).filter((row) => row.dir === "s2c").map((row) => row.msg);
    let approvalsShown = 0;
    let approvalsResolved = 0;
    for (const message of messages) {
      if (!message.method) continue; // Responses belong to the IPC request promises.
      expect(isSupportedAppServerMethod(message.method) || isApprovalRequestMethod(message.method), message.method).toBe(true);
      act(() => receive({ workspace_id: "fixture", message }));
      const params = message.params;
      if (isApprovalRequestMethod(message.method)) {
        expect(result.current.approvals.some((a) => a.request_id === message.id)).toBe(true);
        approvalsShown++;
      }
      if (message.method === "serverRequest/resolved") {
        expect(result.current.approvals.some((a) => a.request_id === params.requestId)).toBe(false);
        approvalsResolved++;
      }
      if (message.method === "thread/status/changed") {
        expect(result.current.threadStatusById[params.threadId]?.activeFlags).toEqual(params.status.activeFlags ?? []);
      }
      if (message.method === "thread/readState/changed") {
        expect(result.current.threadStatusById[params.threadId]?.hasUnread).toBe(params.readState.firstUnread !== null);
      }
      if (message.method === "item/completed") {
        const expected = buildConversationItemFromThreadItem(params.item);
        expect(expected).not.toBeNull();
        const item = result.current.itemsByThread[params.threadId]?.find((entry) => entry.id === params.item.id);
        expect(item?.kind, params.item.type).toBe(expected?.kind);
        if (expected?.kind === "tool") expect(item).toMatchObject({ toolType: expected.toolType });
        if (params.item.type === "agentMessage") expect(item).toMatchObject({ phase: params.item.phase, delivery: params.item.delivery });
      }
      if (message.method === "turn/completed") {
        expect(result.current.threadStatusById[params.threadId]?.lastTurn).toEqual(params.turn);
        expect(result.current.threadStatusById[params.threadId]?.isProcessing).toBe(false);
        expect(result.current.activeTurnIdByThread[params.threadId]).toBeNull();
        expect(params.turn.status).toBe(name === "resume_steer_interrupt.jsonl" ? "interrupted" : "completed");
      }
      if (message.method === "thread/name/updated") {
        expect(result.current.threadsByWorkspace.fixture.find((thread) => thread.id === params.threadId)?.name).toBe(params.threadName);
      }
    }
    expect(approvalsResolved).toBe(approvalsShown);
    expect(result.current.approvals).toEqual([]);
    expect(debug.mock.calls.filter(([entry]) => entry.label === "deprecationNotice")).toHaveLength(
      messages.some((message) => message.method === "deprecationNotice") ? 1 : 0,
    );
    if (name === "lifecycle.jsonl") {
      expect(Object.values(result.current.itemsByThread).flat()).toEqual(expect.arrayContaining([
        expect.objectContaining({ toolType: "contextCompaction" }),
      ]));
    }
  });

  it("clears server-resolved questions with string ids and keeps other workspaces", () => {
    const { result } = renderHook(() => useReplay(vi.fn()));
    const question = { id: "same", method: "item/tool/requestUserInput", params: {
      threadId: "thread", turnId: "turn", itemId: "item", questions: [{ id: "q", header: "Choice", question: "Continue?", options: [] }],
    } };
    act(() => { receive({ workspace_id: "one", message: question }); receive({ workspace_id: "two", message: question }); });
    expect(result.current.userInputRequests).toHaveLength(2);
    act(() => receive({ workspace_id: "one", message: { method: "serverRequest/resolved", params: { threadId: "thread", requestId: "same" } } }));
    expect(result.current.userInputRequests.map((request) => request.workspace_id)).toEqual(["two"]);
  });
});
