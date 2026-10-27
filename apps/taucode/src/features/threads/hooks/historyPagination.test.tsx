// @vitest-environment jsdom
import { useReducer, useRef } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { listThreadHistory, resumeThread } from "@services/tauri";
import type { ThreadTurnsListResponse } from "@protocol/v2/ThreadTurnsListResponse";
import { useThreadActions } from "./useThreadActions";
import { initialState, threadReducer } from "./useThreadsReducer";
import type { ConversationItem } from "@/types";

vi.mock("@services/tauri", () => ({ listThreadHistory: vi.fn(), resumeThread: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

function useHistory() {
  const [state, dispatch] = useReducer(threadReducer, initialState);
  const actions = useThreadActions({
    dispatch, ...state, threadSortKey: "updated_at", getCustomName: () => undefined,
    threadActivityRef: useRef({}), loadedThreadsRef: useRef({}), replaceOnResumeRef: useRef({}),
    applyCollabThreadLinksFromThread: () => {}, updateThreadParent: () => {}, onSubagentThreadDetected: () => {},
  });
  return { state, dispatch, ...actions };
}

function page(ids: string[], nextCursor: string | null): ThreadTurnsListResponse {
  return { data: [{ id: "turn", rootTurnId: null, itemsView: "full", status: "completed", error: null,
    startedAt: null, completedAt: null, durationMs: null,
    items: ids.map((id) => ({ id, type: "agentMessage", text: id, phase: null, memoryCitation: null, delivery: null, questions: null })),
  }], nextCursor, backwardsCursor: null };
}

describe("history pagination", () => {
  it("reconciles recent snapshot echoes once and leaves older repeated prompts separate", () => {
    const old: ConversationItem = { id: "old", kind: "message", role: "user", text: "Again" };
    const pending = { ...old, id: "optimistic-user-first" };
    const second = { ...old, id: "optimistic-user-second" };
    const echo = { ...old, id: "canonical-first" };
    const items = [old, pending, second];
    const state = { ...initialState, itemsByThread: { thread: items } };
    const action = { type: "prependThreadHistory" as const, threadId: "thread", snapshot: items, items: [old, echo], recent: true };
    expect(threadReducer(state, action).itemsByThread.thread.map(item => item.id)).toEqual(["old", "canonical-first", "optimistic-user-second"]);
    expect(threadReducer(state, { ...action, items: [{ ...old, id: "older" }], recent: false }).itemsByThread.thread.map(item => item.id)).toEqual(["older", "old", "optimistic-user-first", "optimistic-user-second"]);
  });

  it("refreshes unchanged items, preserves expanded history, and replaces it after rollback", async () => {
    vi.mocked(resumeThread).mockResolvedValue({ result: { thread: { id: "thread", turns: page(["recent"], null).data, historyCursor: "older" } } });
    const { result } = renderHook(useHistory);
    await act(async () => { await result.current.resumeThreadForWorkspace("ws", "thread"); });
    vi.mocked(listThreadHistory).mockResolvedValue(page(["old"], null));
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    const updated = page(["recent", "new"], null);
    Object.assign(updated.data[0].items[0], { text: "completed remote message", phase: "final_answer" });
    vi.mocked(resumeThread).mockResolvedValue({ result: { thread: { id: "thread", turns: updated.data, historyCursor: "older" } } });
    await act(async () => { await result.current.resumeThreadForWorkspace("ws", "thread", true); });
    expect(result.current.state.itemsByThread.thread.map((item) => item.id)).toEqual(["old", "recent", "new"]);
    expect(result.current.state.itemsByThread.thread[1]).toMatchObject({ text: "completed remote message", phase: "final_answer" });
    expect(result.current.historyCursorByThread.thread).toBeNull();
    vi.mocked(resumeThread).mockResolvedValue({ result: { thread: { id: "thread", turns: page(["recent"], null).data, historyCursor: "older" } } });
    await act(async () => { await result.current.resumeThreadForWorkspace("ws", "thread", true, true); });
    expect(result.current.state.itemsByThread.thread.map((item) => item.id)).toEqual(["recent"]);
    expect(result.current.historyCursorByThread.thread).toBe("older");
  });

  it("starts with the recent page and prepends older pages without losing overlapping live items", async () => {
    vi.mocked(resumeThread).mockResolvedValue({ result: { thread: { id: "thread", turns: page(["recent"], null).data, historyCursor: "older" } } });
    const { result } = renderHook(useHistory);
    await act(async () => { await result.current.resumeThreadForWorkspace("ws", "thread"); });
    expect(result.current.state.itemsByThread.thread.map((item) => item.id)).toEqual(["recent"]);
    let finish!: (value: ThreadTurnsListResponse) => void;
    vi.mocked(listThreadHistory).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    let loading!: Promise<void>;
    act(() => { loading = result.current.loadOlderHistory("ws", "thread"); });
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    expect(listThreadHistory).toHaveBeenCalledTimes(1);
    expect(listThreadHistory).toHaveBeenCalledWith("ws", "thread", "older");
    act(() => result.current.dispatch({ type: "upsertItem", workspaceId: "ws", threadId: "thread",
      item: { id: "recent", kind: "message", role: "assistant", text: "live update" } }));
    await act(async () => { finish(page(["old", "recent"], "oldest")); await loading; });
    expect(result.current.state.itemsByThread.thread.map((item) => item.id)).toEqual(["old", "recent"]);
    expect(result.current.state.itemsByThread.thread[1]).toMatchObject({ text: "live update" });
    vi.mocked(listThreadHistory).mockResolvedValue(page(["first", "old"], null));
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    expect(listThreadHistory).toHaveBeenLastCalledWith("ws", "thread", "oldest");
    expect(result.current.state.itemsByThread.thread.map((item) => item.id)).toEqual(["first", "old", "recent"]);
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    expect(listThreadHistory).toHaveBeenCalledTimes(2);
  });

  it("retains the cursor and items when a page fails or repeats its cursor", async () => {
    vi.mocked(resumeThread).mockResolvedValue({ result: { thread: { id: "thread", turns: [], historyCursor: "older" } } });
    const { result } = renderHook(useHistory);
    await act(async () => { await result.current.resumeThreadForWorkspace("ws", "thread"); });
    vi.mocked(listThreadHistory).mockRejectedValue(new Error("offline"));
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    expect(result.current.historyErrorByThread.thread).toContain("offline");
    expect(result.current.historyCursorByThread.thread).toBe("older");
    vi.mocked(listThreadHistory).mockResolvedValue(page([], "older"));
    await act(async () => { await result.current.loadOlderHistory("ws", "thread"); });
    expect(result.current.historyErrorByThread.thread).toContain("Repeated history cursor");
    expect(result.current.historyLoadingByThread.thread).toBe(false);
  });
});
