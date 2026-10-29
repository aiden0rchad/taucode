// @vitest-environment jsdom
import { assertRowAlignment } from "@/test/rowAlignment.mjs";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppServerEvent, AppSettings } from "@/types";
import { chatRequest, exportMarkdownFile, readChatAttachment } from "@services/tauri";
import { ChatView } from "./ChatView";
import type { Chat } from "./chat";

let listener: (event: AppServerEvent) => void;
let nativeDrop: (event: { payload: { type: string; paths: string[] } }) => void;
vi.mock("@services/dragDrop", () => ({ subscribeWindowDragDrop: (callback: typeof nativeDrop) => { nativeDrop = callback; return () => {}; } }));
vi.mock("@services/events", () => ({ subscribeMenuViewAction: () => () => {}, subscribeAppServerEvents: (callback: typeof listener) => { listener = callback; return () => {}; } }));
vi.mock("@services/tauri", () => ({ subscribeBrowser: vi.fn().mockResolvedValue(() => {}), browserRequest: vi.fn().mockResolvedValue(undefined), chatRequest: vi.fn(), exportMarkdownFile: vi.fn().mockResolvedValue(null), getAppSettings: vi.fn(), updateAppSettings: vi.fn(), readChatAttachment: vi.fn() }));
vi.mock("./ModeShell", () => ({ ModeSwitcher: () => <nav aria-label="App mode" />, useModeSettings: () => undefined }));
const settings = { providers: [{ id: "p", name: "Local", enabled: true, models: [{ id: "one", enabled: true }, { id: "two", enabled: true, supportsImages: false }] }], agentPolicy: { defaultModel: { providerId: "p", modelId: "one" } }, mcpServers: [] } as unknown as AppSettings;
let rows: Chat[];
const thread = (id: string) => ({ id, model: "one", modelProvider: "p", turns: [{ id: "turn", items: [
  { id: "user", type: "userMessage", content: [{ type: "text", text: "**Original question**" }] },
  { id: "assistant", type: "agentMessage", text: "## Original answer" },
] }] });
beforeEach(() => {
  localStorage.clear();
  rows = [{ id: "chat", model: "one", modelProvider: "p", createdAt: 1, updatedAt: Date.now() / 1000, chat: { title: "My conversation", pinned: false, deletedAt: null } }];
  vi.mocked(chatRequest).mockImplementation(async (action, params = {}) => {
    if (action === "list") return rows;
    if (action === "read") return { result: { thread: thread(String(params.threadId)) } };
    if (action === "fork" || action === "start") {
      rows = [...rows.filter(c => c.id !== "fork"), { ...rows[0], id: "fork", model: String(params.model ?? "p/one").split("/")[1] }];
      return { result: { thread: { ...thread("fork"), model: rows[1].model } } };
    }
    if (action === "pin") rows = rows.map(c => ({ ...c, chat: { ...c.chat, pinned: Boolean(params.pinned) } }));
    if (action === "trash") rows = rows.map(c => ({ ...c, chat: { ...c.chat, deletedAt: Date.now() / 1000 } }));
    if (action === "restore") rows = rows.map(c => ({ ...c, chat: { ...c.chat, deletedAt: null } }));
    return {};
  });
});
afterEach(() => { assertRowAlignment(document.body); cleanup(); vi.clearAllMocks(); });
it("exposes the Chat layout and rows used by the WebKit audit", async () => {
  render(<ChatView settings={settings} />);
  const row = await screen.findByRole("button", { name: "My conversation" });
  expect(document.querySelector(".chat-layout:not(.work-layout) .chat-list .thread-row")).toBe(row);
  expect(document.querySelector(".app")).toBeNull();
});
async function open() {
  fireEvent.click(await screen.findByRole("button", { name: "My conversation" }));
  await screen.findByText("Original answer");
  await waitFor(() => expect(screen.getByRole("button", { name: "Regenerate last answer" })).not.toHaveProperty("disabled", true));
}
describe("Chat UI", () => {
  it("renders user and assistant markdown and shared shell controls", async () => {
    const onOpenSettings = vi.fn();
    render(<ChatView settings={settings} onOpenSettings={onOpenSettings} />); await open();
    expect(screen.getByRole("heading", { level: 2, name: "Original answer" })).toBeTruthy();
    expect(screen.getByText("Original question").tagName).toBe("STRONG");
    expect(screen.getByRole("combobox", { name: "Access mode" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("combobox", { name: "Access mode" }).closest(".composer-shell-left")).toBeTruthy();
    expect(screen.getByRole("button", { name: "one · ?" }).closest(".composer-shell-actions")).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Chat model" })).toBeNull();
    expect(screen.getByRole("button", { name: "Toggle side panel" }).closest(".workspace-titlebar")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });
  it("inlines native Tauri drops before sending, without giving paths to the model", async () => {
    vi.mocked(readChatAttachment).mockResolvedValue({ name: "native.txt", text: "Dropped text" });
    render(<ChatView settings={settings} />); await open();
    act(() => nativeDrop({ payload: { type: "drop", paths: ["/picked/native.txt"] } }));
    expect(await screen.findByText("native.txt")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Send" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "chat", input: [{ type: "text", text: "native.txt\n```\nDropped text\n```" }] }));
    expect(readChatAttachment).toHaveBeenCalledWith("/picked/native.txt");
  });
  it("pins, searches, trashes and restores local conversations", async () => {
    render(<ChatView settings={settings} />);
    expect(screen.queryByRole("menuitem", { name: "Pin" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Actions for My conversation" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Pin" }));
    expect(await screen.findByRole("region", { name: "Pinned" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Search conversations" }));
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "missing" } });
    expect(screen.queryByRole("button", { name: "My conversation" })).toBeNull();
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "" } });
    fireEvent.contextMenu(screen.getByRole("button", { name: "My conversation" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "My conversation" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Trash" }));
    fireEvent.click(await screen.findByRole("button", { name: "Actions for My conversation" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Restore" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("restore", { threadId: "chat" }));
  });
  it("forks before edit-resend and model changes; exports local markdown", async () => {
    render(<ChatView settings={settings} />); await open();
    fireEvent.click(screen.getByRole("button", { name: "Actions for thread" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Export" }));
    await waitFor(() => expect(exportMarkdownFile).toHaveBeenCalledWith(expect.stringContaining("Original answer"), "chat.md", "Export chat"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Edit and resend" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Edit and resend" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Edited question" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("fork", { threadId: "chat", model: "p/one", beforeUserItemId: "user" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "fork", input: [{ type: "text", text: "Edited question" }] }));
    await waitFor(() => expect(screen.getByRole("button", { name: "one · ?" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "one · ?" }));
    fireEvent.click(screen.getByRole("button", { name: "one" }));
    fireEvent.click(screen.getByRole("option", { name: "two" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("fork", { threadId: "fork", model: "p/two", beforeUserItemId: undefined }));
  });
  it("regenerates on a fork, streams text, and stops only the active chat", async () => {
    render(<ChatView settings={settings} />); await open();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate last answer" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "fork", input: [{ type: "text", text: "**Original question**" }] }));
    await waitFor(() => expect(screen.getByRole("button", { name: "one · ?" })).not.toHaveProperty("disabled", true));
    act(() => {
      listener({ workspace_id: "chats", message: { method: "turn/started", params: { threadId: "fork" } } });
      listener({ workspace_id: "chats", message: { method: "item/agentMessage/delta", params: { threadId: "fork", itemId: "stream", delta: "Streaming answer" } } });
    });
    expect(screen.getByText("Streaming answer")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("stop", { threadId: "fork" }));
  });
  it("supports pasted images and dropped text; hides Web without the connector", async () => {
    const { rerender } = render(<ChatView settings={settings} />); await open();
    expect(screen.queryByRole("checkbox", { name: "Web" })).toBeNull();
    const image = new File(["png"], "photo.png", { type: "image/png" });
    fireEvent.paste(screen.getByRole("textbox", { name: "Message" }), { clipboardData: { files: [image] } });
    expect(await screen.findByText("photo.png")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Send" })).not.toHaveProperty("disabled", true));
    const file = new File(["notes"], "notes.txt", { type: "text/plain" });
    Object.defineProperty(file, "text", { value: async () => "notes" });
    fireEvent.drop(screen.getByRole("textbox", { name: "Message" }), { dataTransfer: { files: [file] } });
    expect(await screen.findByText("notes.txt")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Send" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "chat", input: [{ type: "text", text: "notes.txt\n```\nnotes\n```" }, { type: "image", url: expect.stringContaining("data:image/png;base64,") }] }));
    rerender(<ChatView settings={{ ...settings, mcpServers: [{ id: "taucode-browser", enabled: true } as never] }} />);
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    expect(screen.getByRole("button", { name: "Turn on Web" })).toBeTruthy();
  });
});

it("includes the shared sidebar resize handle", () => {
  render(<ChatView settings={settings} />);
  expect(screen.getByRole("separator", { name: "Resize sidebar" })).toBeTruthy();
});


it("shows a sent user message before a reply and replaces its server echo", async () => {
  render(<ChatView settings={settings} />); await open();
  const original = vi.mocked(chatRequest).getMockImplementation()!;
  let finish!: (value: unknown) => void;
  vi.mocked(chatRequest).mockImplementation((action, params) => action === "send" ? new Promise(resolve => { finish = resolve; }) : original(action, params));
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Immediate prompt" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await screen.findByText("Immediate prompt", { selector: "p" });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "My conversation" })); });
  expect(screen.getAllByText("Immediate prompt", { selector: "p" })).toHaveLength(1);
  act(() => listener({ workspace_id: "chats", message: { method: "item/started", params: { threadId: "chat", item: { id: "canonical-user", type: "userMessage", content: [{ type: "text", text: "Immediate prompt" }] } } } }));
  expect(screen.getAllByText("Immediate prompt", { selector: "p" })).toHaveLength(1);
  await act(async () => { finish({}); });
});

it("removes an unaccepted optimistic message and preserves the draft after a send failure", async () => {
  render(<ChatView settings={settings} />); await open();
  const original = vi.mocked(chatRequest).getMockImplementation()!;
  vi.mocked(chatRequest).mockImplementation((action, params) => action === "send" ? Promise.reject(new Error("Send disconnected")) : original(action, params));
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Retry me" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await screen.findByText("Error: Send disconnected");
  expect(screen.queryByText("Retry me", { selector: "p" })).toBeNull();
  expect(screen.getByRole("textbox", { name: "Message" })).toHaveProperty("value", "Retry me");
});


it("keeps an accepted optimistic message if refreshing the chat list fails", async () => {
  render(<ChatView settings={settings} />); await open();
  const original = vi.mocked(chatRequest).getMockImplementation()!;
  vi.mocked(chatRequest).mockImplementation((action, params) => action === "send" ? Promise.resolve({}) : action === "list" ? Promise.reject(new Error("List unavailable")) : original(action, params));
  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Already accepted" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await screen.findByText("Error: List unavailable");
  await waitFor(() => expect(screen.getByRole("textbox", { name: "Message" })).toHaveProperty("value", ""));
  expect(screen.getByText("Already accepted", { selector: "p" })).toBeTruthy();
});
