// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen, once } from "@tauri-apps/api/event";
import { clearMocks } from "@tauri-apps/api/mocks";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { buildItemsFromThread } from "@utils/threadItems";
import { chatGroups } from "@/features/chat/chat";
import { createFixtures } from "./fixtures";
import { createPreviewHandler } from "./handler";
import { installUiPreview, selectCode, selectConversation } from "./index";

afterEach(() => { clearMocks(); vi.restoreAllMocks(); vi.useRealTimers(); });

describe("UI preview IPC", () => {
  it("returns non-throwing empty defaults and logs unknown commands once", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { handler } = createPreviewHandler();
    expect(handler("unknown")).toBeNull();
    expect(handler("unknown", null)).toBeNull();
    expect(handler("list_unknown", new Uint8Array())).toEqual([]);
    expect(handler("work_request", { action: "unknown" })).toBeNull();
    expect(warn).toHaveBeenCalledTimes(3);
  });

  it("supplies typed rich fixtures, date groups, side chats and all work statuses", () => {
    const fixtures = createFixtures();
    expect(fixtures.settings.providers).toHaveLength(2);
    expect(fixtures.workspaces).toHaveLength(2);
    expect(fixtures.threads).toHaveLength(5);
    expect(fixtures.threads.find(t => t.id === "thread-empty")?.turns).toEqual([]);
    expect(fixtures.chats.find(c => c.id === "chat-streaming")?.running).toBe(true);
    const items = buildItemsFromThread(fixtures.threads[0]);
    expect(items.filter(i => i.kind === "tool").map(i => i.toolType)).toEqual(["plan", "commandExecution", "fileChange", "collabToolCall"]);
    expect(items.find(i => i.kind === "tool" && i.agentsTree)).toMatchObject({ agentsTree: [{ id: "agent-review", children: [{ id: "agent-tests" }] }] });
    expect(fixtures.threads[3].sideOf?.threadId).toBe("thread-rich");
    expect(fixtures.checkpoints.map(c => c.metadata.kind)).toEqual(["before", "after"]);
    expect(Object.fromEntries(Object.entries(chatGroups(fixtures.chats, "", false)).map(([key, value]) => [key, value.length]))).toEqual({ Pinned: 0, Today: 3, "Previous 7 days": 2, Older: 1 });
    expect(fixtures.work.tasks.map(t => t.status)).toEqual(["running", "needs-input", "queued", "done"]);
  });

  it("persists mutations, publishes completed turns, and isolates responses and sessions", () => {
    vi.useFakeTimers();
    const publish = vi.fn();
    const { handler, state } = createPreviewHandler(publish);
    handler("set_thread_name", { threadId: "thread-rich", name: "Renamed" });
    handler("send_user_message", { threadId: "thread-rich", text: "Hello" });
    expect(state.threads[0].name).toBe("Renamed");
    expect(state.threads[0].turns).toHaveLength(2);
    vi.runAllTimers();
    expect(publish).toHaveBeenLastCalledWith("workspace-atlas", "turn/completed", expect.objectContaining({ threadId: "thread-rich" }));
    handler("chat_request", { action: "rename", params: { threadId: "chat-1", title: "Chat renamed" } });
    handler("chat_request", { action: "start", params: {} });
    expect(state.chats).toHaveLength(7);
    expect(state.chats.find(c => c.id === "chat-1")?.chat.title).toBe("Chat renamed");
    handler("work_request", { action: "respond", params: { taskId: "task-needs-input", answer: { decision: "accept" } } });
    expect(state.work.tasks[1]).toMatchObject({ pending: [], status: "running" });
    handler("work_request", { action: "create", params: { brief: "New brief", spaceId: "space-studio", runNow: true } });
    expect(state.work.tasks[0]).toMatchObject({ title: "New brief", status: "running" });
    const workspaces = handler("list_workspaces") as unknown[];
    workspaces.pop();
    expect(state.workspaces).toHaveLength(2);
    expect(createPreviewHandler().state.threads[0].name).toBe("Polish the welcome message");
  });

  it("installs real Tauri mocks with listen, emit and unlisten", async () => {
    installUiPreview();
    expect(getCurrentWindow().label).toBe("main");
    const callback = vi.fn();
    const unlisten = await listen("preview-test", callback);
    await emit("preview-test", { value: 1 });
    expect(callback).toHaveBeenCalledWith(expect.objectContaining({ payload: { value: 1 } }));
    unlisten();
    await emit("preview-test", {});
    expect(callback).toHaveBeenCalledTimes(1);
    const oneTime = vi.fn();
    await once("preview-once", oneTime);
    await emit("preview-once");
    await emit("preview-once");
    expect(oneTime).toHaveBeenCalledTimes(1);
    expect(await invoke("list_workspaces")).toHaveLength(2);
  });

  it("returns checkpoint lists and diffs using the UI's request actions", () => {
    const { handler } = createPreviewHandler();
    expect(handler("checkpoint_request", { params: { action: "list", threadId: "thread-rich" } })).toMatchObject({ checkpoints: [{ id: "checkpoint-before" }, { id: "checkpoint-after" }] });
    expect(handler("checkpoint_request", { params: { action: "diff", threadId: "thread-rich", checkpointId: "checkpoint-before" } })).toMatchObject({ changes: [{ path: "src/greeting.ts", diff: expect.stringContaining("+export const greeting") }] });
  });

  it("selects URL state once, validates IDs and applies mode/theme settings", () => {
    window.history.replaceState(null, "", "/?mode=work&task=task-needs-input&theme=light");
    const { state } = createPreviewHandler(() => {}, window.location.search);
    expect(state.settings).toMatchObject({ theme: "light", windowModes: { main: "work" } });
    installUiPreview();
    const select = vi.fn();
    selectConversation("task", select);
    selectConversation("task", select);
    expect(select).toHaveBeenCalledExactlyOnceWith("task-needs-input");
    window.history.replaceState(null, "", "/?thread=thread-notes&settings=display");
    installUiPreview();
    const settings = vi.fn();
    selectCode(state.workspaces, select, settings);
    expect(select).toHaveBeenLastCalledWith("workspace-notes", "thread-notes");
    expect(settings).toHaveBeenCalledWith("display");
    window.history.replaceState(null, "", "/?chat=invalid");
    installUiPreview();
    selectConversation("chat", select);
    expect(select).toHaveBeenLastCalledWith("chat-1");
  });
});
