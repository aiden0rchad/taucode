// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useSideChats } from "./useSideChats";
import { createSideChat } from "@services/tauri";
import type { EngineKind, ThreadSummary, WorkspaceInfo } from "@/types";
vi.mock("@services/tauri", () => ({ createSideChat: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const workspace: WorkspaceInfo = { id: "ws", name: "Project", path: "/project", connected: true, settings: { sidebarCollapsed: false } };
const refresh = vi.fn().mockResolvedValue(null);
const list = vi.fn().mockResolvedValue(null);
function Harness({ engine = "pi", threads = [] }: { engine?: EngineKind; threads?: ThreadSummary[] }) {
  const side = useSideChats({ workspace, threadId: "main", threads, engine, model: "provider/model", refresh, list });
  return <><button disabled={Boolean(side.disabledReason)} title={side.disabledReason} onClick={() => void side.create()}>Header side chat</button><button disabled={Boolean(side.messageDisabledReason)} title={side.messageDisabledReason} onClick={() => void side.create("answer-1")}>Message side chat</button>{side.current && <div role="region" aria-label="Open split">{side.current.id}<button onClick={side.close}>Close</button></div>}{side.error && <p role="alert">{side.error}</p>}</>;
}
describe("Side chat creation", () => {
  it("forks the latest context from the header and hides the split on close", async () => {
    vi.mocked(createSideChat).mockResolvedValue({ result: { thread: { id: "side" } } });
    render(<Harness />); fireEvent.click(screen.getByText("Header side chat"));
    expect(await screen.findByRole("region", { name: "Open split" })).toBeTruthy();
    expect(createSideChat).toHaveBeenCalledWith("ws", "main", undefined);
    await waitFor(() => expect(refresh).toHaveBeenCalledWith("ws", "side"));
    fireEvent.click(screen.getByText("Close")); expect(screen.queryByRole("region")).toBeNull();
  });
  it("forks the selected message and binds Command-Shift-K to the latest context", async () => {
    vi.mocked(createSideChat).mockResolvedValue({ result: { thread: { id: "side" } } });
    render(<Harness />); fireEvent.click(screen.getByText("Message side chat"));
    await waitFor(() => expect(createSideChat).toHaveBeenCalledWith("ws", "main", "answer-1"));
    await waitFor(() => expect(list).toHaveBeenCalledOnce());
    fireEvent.keyDown(window, { key: "K", metaKey: true, shiftKey: true });
    await waitFor(() => expect(createSideChat).toHaveBeenLastCalledWith("ws", "main", undefined));
  });
  it("disables Claude buttons and shortcuts unless latest-session fork is advertised", async () => {
    const { rerender } = render(<Harness engine="claude" />);
    const header = screen.getByText("Header side chat") as HTMLButtonElement;
    const message = screen.getByText("Message side chat") as HTMLButtonElement;
    expect(header.disabled).toBe(true);
    expect(header.title).toBe("Claude Code adapter doesn't support forking sessions");
    expect(message.disabled).toBe(true);
    expect(message.title).toContain("Message-level");
    fireEvent.keyDown(window, { key: "K", metaKey: true, shiftKey: true });
    expect(createSideChat).not.toHaveBeenCalled();
    vi.mocked(createSideChat).mockResolvedValue({ result: { thread: { id: "side" } } });
    rerender(<Harness engine="claude" threads={[{ id: "main", name: "Claude", updatedAt: 1, capabilities: { fork: true, rewind: false } }]} />);
    expect(header.disabled).toBe(false);
    expect(message.disabled).toBe(true);
    fireEvent.click(header);
    await waitFor(() => expect(createSideChat).toHaveBeenCalledWith("ws", "main", undefined));
  });
});
