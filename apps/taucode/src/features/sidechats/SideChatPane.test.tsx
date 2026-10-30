// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { ComposerPolicyProvider } from "@/features/composer/components/ComposerPolicy";
import { defaultAgentPolicy } from "@settings/registry";
import type { ModelOption } from "@/types";
import { SideChatPane, SideChatSplit } from "./SideChatPane";
import { sideChatQuote } from "./useSideChats";

vi.mock("@/features/messages/components/Messages", () => ({ Messages: ({ onSendToMain, processingStartedAt, lastDurationMs }: { onSendToMain: (text: string) => void; processingStartedAt?: number | null; lastDurationMs?: number | null }) => <button data-start={processingStartedAt} data-duration={lastDurationMs} onClick={() => onSendToMain("Answer\nSecond line")}>Send to main</button> }));
afterEach(cleanup);
const chat = { id: "child", parentId: "main", workspaceId: "ws", title: "Research", engine: "pi" as const, model: "test/model" };
const props = () => ({ chat, items: [], models: [], running: false, messageProps: { openTargets: [], selectedOpenAppId: "" }, onSend: vi.fn().mockResolvedValue({ status: "sent" }), onInterrupt: vi.fn().mockResolvedValue(undefined), onClose: vi.fn(), onDelete: vi.fn(), onSendToMain: vi.fn() });
describe("Side chats", () => {
  it("resolves a late parent model with reasoning and puts the inheritance note only in the model list", async () => {
    localStorage.clear();
    const p = props();
    const parent: ModelOption = { id: "test/parent", model: "parent", displayName: "Parent display name", description: "", supportedReasoningEfforts: [], defaultReasoningEffort: "high", isDefault: true, reasoning: { supported: true, levels: ["low", "high"], default: "high", control: "effort", source: "api", detectedAt: 1 } };
    const other = { ...parent, id: "test/other", model: "other", displayName: "Other model" };
    const { rerender } = render(<SideChatPane {...p} chat={{ ...chat, model: null }} />);
    expect(screen.queryByText("Parent model")).toBeNull();
    rerender(<SideChatPane {...p} chat={{ ...chat, model: null }} models={[parent, other]} parentModel={parent} />);
    const chip = screen.getByRole("button", { name: "Parent display name · High" });
    expect(chip.textContent).not.toContain("Same as parent");
    fireEvent.click(chip);
    fireEvent.click(screen.getByRole("button", { name: "Parent display name" }));
    expect(screen.getByText("Same as parent").tagName).toBe("SMALL");
    fireEvent.click(screen.getByRole("option", { name: "Other model" }));
    await screen.findByRole("button", { name: "Other model · High" });
    fireEvent.change(screen.getByLabelText("Side chat message"), { target: { value: "Explore" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(p.onSend).toHaveBeenCalledWith("Explore", "test/other", "read-only", "high"));
  });
  it("sends with the parent model and read-only access; preserves blocked drafts", async () => {
    const p = props(); p.onSend.mockResolvedValue({ status: "blocked" });
    render(<SideChatPane {...p} />);
    fireEvent.change(screen.getByLabelText("Side chat message"), { target: { value: "Explore" } });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(p.onSend).toHaveBeenCalledWith("Explore", "test/model", "read-only", null));
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
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Close side chat" }));
    expect(p.onClose).toHaveBeenCalledOnce(); expect(p.onDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Actions for side chat" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Archive" }));
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


it("uses side chat timing instead of the parent's message props", () => {
  const p = props();
  render(<SideChatPane {...p} running processingStartedAt={100} lastDurationMs={200} messageProps={{ ...p.messageProps, ...{ processingStartedAt: 900, lastDurationMs: 800 } }} />);
  const message = screen.getByRole("button", { name: "Send to main" });
  expect(message.dataset.start).toBe("100");
  expect(message.dataset.duration).toBe("200");
});

it("cannot change the parent conversation team from the side-chat model picker", () => {
  render(<ComposerPolicyProvider scope="Parent" policy={defaultAgentPolicy} onChange={vi.fn()}><SideChatPane {...props()} /></ComposerPolicyProvider>);
  fireEvent.click(screen.getByRole("button", { name: /model ·/i }));
  expect(screen.queryByRole("button", { name: "Chain of Command" })).toBeNull();
});
