// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { SideChatPane, SideChatSplit } from "./SideChatPane";
import { sideChatQuote } from "./useSideChats";

vi.mock("@/features/messages/components/Messages", () => ({ Messages: ({ onSendToMain }: { onSendToMain: (text: string) => void }) => <button onClick={() => onSendToMain("Answer\nSecond line")}>Send to main</button> }));
afterEach(cleanup);
const chat = { id: "child", parentId: "main", workspaceId: "ws", title: "Research", engine: "pi" as const, model: "test/model" };
const props = () => ({ chat, items: [], models: [], running: false, messageProps: { openTargets: [], selectedOpenAppId: "" }, onSend: vi.fn().mockResolvedValue({ status: "sent" }), onInterrupt: vi.fn().mockResolvedValue(undefined), onClose: vi.fn(), onDelete: vi.fn(), onSendToMain: vi.fn() });
describe("Side chats", () => {
  it("sends with the parent model and read-only access; preserves blocked drafts", async () => {
    const p = props(); p.onSend.mockResolvedValue({ status: "blocked" });
    render(<SideChatPane {...p} />);
    fireEvent.change(screen.getByLabelText("Side chat message"), { target: { value: "Explore" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(p.onSend).toHaveBeenCalledWith("Explore", "test/model", "read-only"));
    expect((screen.getByLabelText("Side chat message") as HTMLTextAreaElement).value).toBe("Explore");
    expect(await screen.findByRole("alert")).toBeTruthy();
  });
  it("inserts a quote into the main composer without sending", () => {
    const p = props();
    function Harness() {
      const [draft, setDraft] = useState("Existing draft");
      return <><textarea aria-label="Main composer" readOnly value={draft} /><SideChatPane {...p} onSendToMain={text => setDraft(d => `${d}\n\n${sideChatQuote(chat.title, text)}`)} /></>;
    }
    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Send to main" }));
    expect((screen.getByLabelText("Main composer") as HTMLTextAreaElement).value).toBe("Existing draft\n\nFrom side chat: Research\n> Answer\n> Second line\n");
    expect(p.onSend).not.toHaveBeenCalled();
  });
  it("stops only the side chat and distinguishes close from archive", () => {
    const p = props(); render(<SideChatPane {...p} running />);
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(p.onInterrupt).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Close side chat" }));
    expect(p.onClose).toHaveBeenCalledOnce(); expect(p.onDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Archive" }));
    expect(p.onDelete).toHaveBeenCalledOnce();
  });
  it("supports keyboard resizing and a narrow sheet", () => {
    const { rerender } = render(<SideChatSplit main={<p>Main chat</p>}><p>Side conversation</p></SideChatSplit>);
    fireEvent.keyDown(screen.getByRole("separator"), { key: "ArrowLeft" });
    expect(screen.getByRole("separator").getAttribute("aria-valuenow")).toBe("50");
    rerender(<SideChatSplit compact main={<p>Main chat</p>}><p>Side conversation</p></SideChatSplit>);
    expect(screen.queryByText("Main chat")).toBeNull();
    expect(screen.queryByRole("separator")).toBeNull();
    expect(screen.getByRole("complementary", { name: "Side chat" })).toBeTruthy();
  });
});
