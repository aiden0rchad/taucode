/** @vitest-environment jsdom */
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FileTreePanel } from "./FileTreePanel";
import { readWorkspaceFile } from "@services/tauri";
vi.mock("@services/tauri", () => ({ readWorkspaceFile: vi.fn() }));
vi.mock("@tanstack/react-virtual", () => ({ useVirtualizer: () => ({ getVirtualItems: () => [], getTotalSize: () => 0, measureElement: vi.fn() }) }));
vi.mock("./FilePreviewPopover", () => ({ FilePreviewPopover: ({ path, content, error, onClose }: { path: string; content: string; error?: string; onClose: () => void }) => <div role="dialog" aria-label={path}>{content}{error}<button onClick={onClose}>Close preview</button></div> }));
const props = { workspaceId: "w", workspacePath: "/workspace", files: [], modifiedFiles: [], isLoading: false, filePanelMode: "files" as const, onFilePanelModeChange: vi.fn(), canInsertText: true, openTargets: [], openAppIconById: {}, selectedOpenAppId: "", onSelectOpenAppId: vi.fn() };
beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);
describe("requested workspace file", () => {
  it("opens a reported file without a populated tree and reopens the same path", async () => {
    vi.mocked(readWorkspaceFile).mockResolvedValue({ content: "Report content", truncated: false });
    const { rerender } = render(<FileTreePanel {...props} requestedFile={{ path: "report.md", requestId: 1 }} />);
    await waitFor(() => expect(screen.getByRole("dialog", { name: "report.md" }).textContent).toContain("Report content"));
    expect(readWorkspaceFile).toHaveBeenCalledWith("w", "report.md");
    fireEvent.click(screen.getByRole("button", { name: "Close preview" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    rerender(<FileTreePanel {...props} requestedFile={{ path: "report.md", requestId: 2 }} />);
    await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
    expect(readWorkspaceFile).toHaveBeenCalledTimes(2);
  });
  it("does not show an old response after workspace navigation", async () => {
    let resolve!: (value: { content: string; truncated: boolean }) => void;
    vi.mocked(readWorkspaceFile).mockImplementation(() => new Promise(done => { resolve = done; }));
    const { rerender } = render(<FileTreePanel {...props} requestedFile={{ path: "old.md", requestId: 1 }} />);
    await waitFor(() => expect(readWorkspaceFile).toHaveBeenCalled());
    rerender(<FileTreePanel {...props} workspaceId="other" workspacePath="/other" requestedFile={null} />);
    resolve({ content: "Private old content", truncated: false });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.queryByText("Private old content")).toBeNull();
  });
});
