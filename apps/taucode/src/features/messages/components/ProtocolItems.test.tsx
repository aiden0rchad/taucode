// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { ThreadItem } from "@protocol/v2/ThreadItem";
import { buildConversationItemFromThreadItem } from "@utils/threadItems";
import { Messages } from "./Messages";

vi.mock("../hooks/useFileLinkOpener", () => ({ useFileLinkOpener: () => ({ openFileLink: vi.fn(), showFileLinkMenu: vi.fn() }) }));
beforeAll(() => { HTMLElement.prototype.scrollIntoView = vi.fn(); });
afterEach(cleanup);

const items: ThreadItem[] = [
  { type: "subAgentActivity", id: "sub", kind: "completed", agentThreadId: "child", agentPath: "/root/child", model: "model", reasoningEffort: null },
  { type: "dynamicToolCall", id: "dynamic", namespace: "tools", tool: "lookup", arguments: { query: "sample" }, status: "completed", contentItems: [], success: true, durationMs: 5 },
  { type: "hookPrompt", id: "hook", fragments: [{ hookRunId: "run", text: "hook context" }] },
  { type: "functionCallOutput", id: "function", name: "lookup", namespace: null, output: "function result" },
  { type: "sleep", id: "sleep", durationMs: 1200 },
  { type: "contextCompaction", id: "compact" },
];

function show(item: Record<string, unknown>) {
  const converted = buildConversationItemFromThreadItem(item)!;
  return render(<Messages items={[converted]} threadId="thread" isThinking={false} openTargets={[]} selectedOpenAppId="" />);
}

describe("0.164 item renderers", () => {
  it("renders browser MCP screenshots with the existing image lightbox", () => {
    show({ type:"mcpToolCall", id:"browser", server:"taucode-browser", tool:"browser_take_screenshot", status:"completed", arguments:{}, result:{content:[{type:"text",text:"Screenshot"},{type:"image",mimeType:"image/png",data:"AAAA"}]} });
    expect(screen.getByAltText("Generated image 1").getAttribute("src")).toBe("data:image/png;base64,AAAA");
    fireEvent.click(screen.getByRole("button",{name:"Open image 1"}));
    expect(screen.getByRole("dialog")).toBeTruthy();
  });
  it.each(items)("renders $type as a compact expandable row", (item) => {
    const { container } = show(item);
    expect(container.querySelector(".tool-inline-row")).not.toBeNull();
    const toggle = screen.getByRole("button", { name: "Toggle tool details" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(container.textContent).not.toBe("");
    if (item.type === "hookPrompt") expect(screen.getByText("hook context")).toBeTruthy();
    if (item.type === "functionCallOutput") expect(screen.getByText("function result")).toBeTruthy();
    if (item.type === "sleep") expect(screen.getByText("1200 ms")).toBeTruthy();
    if (item.type === "contextCompaction") expect(screen.getByText("Context compaction")).toBeTruthy();
  });

  it("renders generated images and opens the existing lightbox", () => {
    show({ type: "imageGeneration", id: "image", result: "AAAA", status: "completed", revisedPrompt: null, failure: null } satisfies ThreadItem);
    expect(screen.getByAltText("Generated image 1").getAttribute("src")).toBe("data:image/png;base64,AAAA");
    fireEvent.click(screen.getByRole("button", { name: "Open image 1" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("preserves structured function output", () => {
    show({ type: "functionCallOutput", id: "structured", name: "lookup", namespace: null, output: [{ type: "input_text", text: "structured result" }] });
    fireEvent.click(screen.getByRole("button", { name: "Toggle tool details" }));
    expect(screen.getByText(/structured result/)).toBeTruthy();
  });

  it("handles image file references without inventing a download URL", () => {
    show({ type: "imageGeneration", id: "file", fileId: "file-id", status: "completed", revisedPrompt: null, failure: null } satisfies ThreadItem);
    fireEvent.click(screen.getByRole("button", { name: "Toggle tool details" }));
    expect(screen.getByText("Image preview unavailable.")).toBeTruthy();
  });

  it("keeps unknown future items collapsed and displays their payload on demand", () => {
    show({ type: "futureItem", id: "future", value: "new payload" });
    expect(screen.getByText("futureItem")).toBeTruthy();
    expect(screen.queryByText(/new payload/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Toggle tool details" }));
    expect(screen.getByText(/new payload/)).toBeTruthy();
  });

  it("loads older history when scrolled to the top", () => {
    const load = vi.fn().mockResolvedValue(undefined);
    const { container } = render(<Messages items={[]} threadId="thread" isThinking={false} openTargets={[]} selectedOpenAppId=""
      hasOlderHistory onLoadOlderHistory={load} />);
    fireEvent.scroll(container.querySelector(".messages")!);
    expect(load).toHaveBeenCalledOnce();
  });

  it("preserves the viewport when the load-older button prepends a page", () => {
    const recent = buildConversationItemFromThreadItem({ type: "agentMessage", id: "recent", text: "Recent" })!;
    const older = buildConversationItemFromThreadItem({ type: "agentMessage", id: "older", text: "Older" })!;
    const props = { threadId: "thread", isThinking: false, openTargets: [], selectedOpenAppId: "", hasOlderHistory: true, onLoadOlderHistory: vi.fn().mockResolvedValue(undefined) };
    const { container, rerender } = render(<Messages {...props} items={[recent]} />);
    const scroller = container.querySelector(".messages")!;
    Object.defineProperty(scroller, "scrollHeight", { configurable: true, value: 500 });
    scroller.scrollTop = 20;
    fireEvent.click(screen.getByRole("button", { name: "Load older messages" }));
    rerender(<Messages {...props} items={[recent]} isLoadingHistory />);
    Object.defineProperty(scroller, "scrollHeight", { configurable: true, value: 800 });
    rerender(<Messages {...props} items={[older, recent]} />);
    expect(scroller.scrollTop).toBe(320);
    expect(props.onLoadOlderHistory).toHaveBeenCalledOnce();
  });
});
