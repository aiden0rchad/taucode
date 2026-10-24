// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Checkpoints, RewindAction } from "./Checkpoints";
import { checkpointRequest } from "@services/tauri";
import { openCheckpoints } from "@services/events";
import type { Checkpoint } from "@/types";
let onEvent: (event: { workspace_id: string; message: Record<string, unknown> }) => void;
vi.mock("@services/tauri", () => ({ checkpointRequest: vi.fn() }));
vi.mock("@services/events", async () => ({ ...await vi.importActual<typeof import("@services/events")>("@services/events"), subscribeAppServerEvents: (callback: typeof onEvent) => { onEvent = callback; return () => {}; } }));
const row: Checkpoint = { id: "checkpoint", commit: "abc", metadata: { threadId: "thread", turnId: "turn", userItemId: "user", kind: "before", timestamp: "2026-10-23T00:00:00Z", engine: "pi", preview: "fix the parser" }, skipped: ["big.bin"], warning: null, filesChanged: 2 };
const changed = vi.fn().mockResolvedValue(undefined);
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(checkpointRequest).mockImplementation(async (_, params) => {
    if (params.action === "list") return { checkpoints: [row] };
    if (params.action === "capabilities") return { conversation: true, engine: "pi" };
    if (params.action === "diff") return { changes: [{ path: "parser.rs", status: "modified", diff: "-old\n+new" }, { path: "added.txt", status: "added since", diff: "+added" }] };
    return { undoAvailable: params.action === "rewind", conversationChanged: true, text: "fix the parser" };
  });
});
afterEach(cleanup);
const open = (message = true) => act(() => openCheckpoints({ workspaceId: "workspace", threadId: "thread", ...(message ? { userItemId: "user" } : {}) }));
describe("checkpoints", () => {
  it("selects the second turn's before checkpoint by user item", async () => {
    const rows = ["before", "after", "before", "after"].map((kind, index) => ({
      ...row, id: `checkpoint-${index + 1}`,
      metadata: { ...row.metadata, kind, turnId: `turn-${Math.floor(index / 2) + 1}`, userItemId: `user-${Math.floor(index / 2) + 1}` },
    }));
    const original = vi.mocked(checkpointRequest).getMockImplementation()!;
    vi.mocked(checkpointRequest).mockImplementation((w, p) => p.action === "list" ? Promise.resolve({ checkpoints: rows as Checkpoint[] }) : original(w, p));
    render(<Checkpoints running={{}} onChanged={changed} />);
    act(() => openCheckpoints({ workspaceId: "workspace", threadId: "thread", userItemId: "user-2" }));
    await screen.findByText("Confirm rewind");
    fireEvent.click(screen.getByText("Confirm rewind"));
    await waitFor(() => expect(checkpointRequest).toHaveBeenCalledWith("workspace", expect.objectContaining({ action: "rewind", checkpointId: "checkpoint-3" })));
  });
  it("selects paths, warns about deletion, expands diffs, confirms and undoes", async () => {
    render(<Checkpoints running={{}} onChanged={changed} />); open();
    await screen.findByText("This file will be deleted.");
    expect((screen.getByLabelText("Mode") as HTMLSelectElement).value).toBe("both");
    expect(screen.getByText(/Restore 1 files, delete 1/).textContent).toContain("fix the parser");
    fireEvent.click(screen.getByText("View diff for parser.rs"));
    expect(screen.getByText("-old +new")).toBeTruthy();
    fireEvent.click(screen.getByLabelText(/added.txt/));
    expect(screen.getByText(/Restore 1 files, delete 0/)).toBeTruthy();
    fireEvent.click(screen.getByText("Confirm rewind"));
    await screen.findByText("Undo");
    expect(checkpointRequest).toHaveBeenCalledWith("workspace", expect.objectContaining({ action: "rewind", paths: ["parser.rs"], mode: "both" }));
    expect(changed).toHaveBeenCalledWith("workspace", "thread", "fix the parser");
    fireEvent.click(screen.getByText("Undo"));
    await waitFor(() => expect(screen.queryByText("Undo")).toBeNull());
  });
  it("lists checkpoints and skipped files", async () => {
    render(<Checkpoints running={{}} onChanged={changed} />); open(false);
    await screen.findByText("Rewind to here");
    expect(screen.getByText("1 files skipped")).toBeTruthy();
    fireEvent.click(screen.getByText("Rewind to here"));
    await screen.findByText("Confirm rewind");
  });
  it("disables conversation modes when the server lacks rollback", async () => {
    const original = vi.mocked(checkpointRequest).getMockImplementation()!;
    vi.mocked(checkpointRequest).mockImplementation((w, p) => p.action === "capabilities" ? Promise.resolve({ conversation: false, engine: "codex" }) : original(w, p));
    render(<Checkpoints running={{}} onChanged={changed} />); open();
    await screen.findByText("This file will be deleted.");
    expect((screen.getByLabelText("Mode") as HTMLSelectElement).value).toBe("files");
    expect((screen.getByRole("option", { name: "Conversation only" }) as HTMLOptionElement).disabled).toBe(true);
  });
  it("disables rewind while running and expires Undo on the next turn", async () => {
    const view = render(<><RewindAction workspaceId="workspace" threadId="thread" userItemId="user" running /><Checkpoints running={{}} onChanged={changed} /></>);
    expect((screen.getByText("Rewind…") as HTMLButtonElement).disabled).toBe(true);
    open(); await screen.findByText("This file will be deleted."); fireEvent.click(screen.getByText("Confirm rewind"));
    await screen.findByText("Undo");
    act(() => onEvent({ workspace_id: "workspace", message: { method: "turn/started", params: { threadId: "thread" } } }));
    expect(screen.queryByText("Undo")).toBeNull();
    view.rerender(<Checkpoints running={{ thread: { isProcessing: true } }} onChanged={changed} />);
    open(); await screen.findByText("This file will be deleted.");
    expect((screen.getByText("Confirm rewind") as HTMLButtonElement).disabled).toBe(true);
  });
  it("keeps Claude rewind files-only even if a server reports conversation support", async () => {
    const original = vi.mocked(checkpointRequest).getMockImplementation()!;
    vi.mocked(checkpointRequest).mockImplementation((w, p) => p.action === "capabilities" ? Promise.resolve({ conversation: true, engine: "claude" }) : original(w, p));
    render(<Checkpoints running={{}} onChanged={changed} />); open();
    await screen.findByText("This file will be deleted.");
    expect((screen.getByLabelText("Mode") as HTMLSelectElement).value).toBe("files");
    for (const name of ["Files and conversation", "Conversation only"]) {
      const option = screen.getByRole("option", { name }) as HTMLOptionElement;
      expect(option.disabled).toBe(true);
      expect(option.title).toBe("Conversation rewind isn't supported for Claude Code threads yet");
    }
    fireEvent.click(screen.getByText("Confirm rewind"));
    await waitFor(() => expect(checkpointRequest).toHaveBeenCalledWith("workspace", expect.objectContaining({ mode: "files" })));
  });
});
