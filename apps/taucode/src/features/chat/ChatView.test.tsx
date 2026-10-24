// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppServerEvent, AppSettings } from "@/types";
import { chatRequest, exportMarkdownFile, readChatAttachment } from "@services/tauri";
import { ChatView } from "./ChatView";
import type { Chat } from "./chat";

let listener: (event: AppServerEvent) => void;
let nativeDrop: (event: { payload: { type: string; paths: string[] } }) => void;
vi.mock("@services/dragDrop", () => ({ subscribeWindowDragDrop: (callback: typeof nativeDrop) => { nativeDrop = callback; return () => {}; } }));
vi.mock("@services/events", () => ({ subscribeAppServerEvents: (callback: typeof listener) => { listener = callback; return () => {}; } }));
vi.mock("@services/tauri", () => ({ chatRequest: vi.fn(), exportMarkdownFile: vi.fn().mockResolvedValue(null), getAppSettings: vi.fn(), updateAppSettings: vi.fn(), readChatAttachment: vi.fn() }));
vi.mock("./ModeShell", () => ({ ModeSwitcher: () => <nav aria-label="App mode" />, useModeSettings: () => undefined }));
const settings = { providers: [{ id: "p", name: "Local", enabled: true, models: [{ id: "one", enabled: true }, { id: "two", enabled: true, supportsImages: false }] }], agentPolicy: { defaultModel: { providerId: "p", modelId: "one" } }, mcpServers: [] } as unknown as AppSettings;
let rows: Chat[];
const thread = (id: string) => ({ id, model: "one", modelProvider: "p", turns: [{ id: "turn", items: [
  { id: "user", type: "userMessage", content: [{ type: "text", text: "**Original question**" }] },
  { id: "assistant", type: "agentMessage", text: "## Original answer" },
] }] });
beforeEach(() => {
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
afterEach(() => { cleanup(); vi.clearAllMocks(); });
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
    expect(screen.getByRole("button", { name: "Browser" }).closest(".main-header-actions")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });
  it("inlines native Tauri drops before sending, without giving paths to the model", async () => {
    vi.mocked(readChatAttachment).mockResolvedValue({ name: "native.txt", text: "Dropped text" });
    render(<ChatView settings={settings} />); await open();
    act(() => nativeDrop({ payload: { type: "drop", paths: ["/picked/native.txt"] } }));
    expect(await screen.findByText("native.txt")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Send ↑" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Send ↑" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "chat", input: [{ type: "text", text: "native.txt\n```\nDropped text\n```" }] }));
    expect(readChatAttachment).toHaveBeenCalledWith("/picked/native.txt");
  });
  it("pins, searches, trashes and restores local conversations", async () => {
    render(<ChatView settings={settings} />);
    expect(screen.queryByRole("menuitem", { name: "Pin" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Actions for My conversation" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Pin" }));
    expect(await screen.findByRole("region", { name: "Pinned" })).toBeTruthy();
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
    fireEvent.click(screen.getByRole("button", { name: "Export as Markdown" }));
    await waitFor(() => expect(exportMarkdownFile).toHaveBeenCalledWith(expect.stringContaining("Original answer"), "chat.md", "Export chat as Markdown"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Edit and resend" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Edit and resend" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "Edited question" } });
    fireEvent.click(screen.getByRole("button", { name: "Send ↑" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("fork", { threadId: "chat", model: "p/one", beforeUserItemId: "user" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "fork", input: [{ type: "text", text: "Edited question" }] }));
    await waitFor(() => expect(screen.getByLabelText("Chat model")).not.toHaveProperty("disabled", true));
    fireEvent.change(screen.getByLabelText("Chat model"), { target: { value: "p/two" } });
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("fork", { threadId: "fork", model: "p/two", beforeUserItemId: undefined }));
  });
  it("regenerates on a fork, streams text, and stops only the active chat", async () => {
    render(<ChatView settings={settings} />); await open();
    fireEvent.click(screen.getByRole("button", { name: "Regenerate last answer" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "fork", input: [{ type: "text", text: "**Original question**" }] }));
    await waitFor(() => expect(screen.getByLabelText("Chat model")).not.toHaveProperty("disabled", true));
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
    await waitFor(() => expect(screen.getByRole("button", { name: "Send ↑" })).not.toHaveProperty("disabled", true));
    const file = new File(["notes"], "notes.txt", { type: "text/plain" });
    Object.defineProperty(file, "text", { value: async () => "notes" });
    fireEvent.drop(screen.getByRole("textbox", { name: "Message" }), { dataTransfer: { files: [file] } });
    expect(await screen.findByText("notes.txt")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Send ↑" })).not.toHaveProperty("disabled", true));
    fireEvent.click(screen.getByRole("button", { name: "Send ↑" }));
    await waitFor(() => expect(chatRequest).toHaveBeenCalledWith("send", { threadId: "chat", input: [{ type: "text", text: "notes.txt\n```\nnotes\n```" }, { type: "image", url: expect.stringContaining("data:image/png;base64,") }] }));
    rerender(<ChatView settings={{ ...settings, mcpServers: [{ id: "taucode-browser", enabled: true } as never] }} />);
    expect(screen.getByRole("checkbox", { name: "Web" })).toBeTruthy();
  });
});
