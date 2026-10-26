import { expect, it } from "vitest";
import { initialState, threadReducer } from "../useThreadsReducer";

it("reconciles optimistic user echoes and removes only the failed pending item", () => {
  const pending = { id: "optimistic-user-1", kind: "message" as const, role: "user" as const, text: "Question" };
  let state = threadReducer(initialState, { type: "upsertItem", workspaceId: "ws", threadId: "thread", item: pending });
  state = threadReducer(state, { type: "upsertItem", workspaceId: "ws", threadId: "thread", item: { ...pending, id: "server-user" } });
  state = threadReducer(state, { type: "removeItem", threadId: "thread", itemId: pending.id });
  expect(state.itemsByThread.thread).toEqual([{ ...pending, id: "server-user", images: undefined }]);
  state = threadReducer(state, { type: "upsertItem", workspaceId: "ws", threadId: "thread", item: { ...pending, id: "optimistic-user-failed" } });
  state = threadReducer(state, { type: "removeItem", threadId: "thread", itemId: "optimistic-user-failed" });
  expect(state.itemsByThread.thread.map(item => item.id)).toEqual(["server-user"]);
});
