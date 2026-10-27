/** @vitest-environment jsdom */
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CodeSessionRail, type CodeSessionRailProps } from "./CodeSessionRail";
const panel = vi.hoisted(() => ({ openTool: vi.fn(), openPage: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@/features/panels/SidePanel", () => ({ useSidePanel: () => panel }));
vi.mock("@/features/design-system/components/modal/ModalShell", () => ({ ModalShell: ({ children }: { children: React.ReactNode }) => <div role="dialog" aria-label="New environment">{children}</div> }));
const task = { id: "task", workspaceId: "w", threadId: "t", command: "npm run dev", label: "Dev server", cwd: "/workspace", pid: 1, status: "running" as const, startedAt: Date.now() - 30000, endedAt: null, exitCode: null, output: "", outputTruncated: false, urls: ["http://localhost:3000"] };
function props(): CodeSessionRailProps {
  const workspace = { id: "w", name: "Project", path: "/workspace", connected: true, settings: { sidebarCollapsed: false } };
  return { workspace, workspaces: [workspace], branch: "main", branches: [{ name: "main", lastCommit: 0 }], canCheckoutBranch: true, onCheckoutBranch: vi.fn(), onSelectWorkspace: vi.fn(), profiles: [], engine: "pi", modelId: "model", onSaveEnvironment: vi.fn().mockResolvedValue(undefined), onSelectEnvironment: vi.fn(), onDeleteEnvironment: vi.fn().mockResolvedValue(undefined), remote: { mode: "local", host: "localhost:4732" }, usage: null, protocol: "pi-rpc", skills: [], skillsAvailable: false, memoryFiles: ["AGENTS.md"], connectors: [{ id: "browser", name: "Browser", enabled: true }], connectorError: null, connectorsLoading: false, outputs: ["reports/result.md"], onOpenFile: vi.fn().mockReturnValue(true), git: { enabled: true, files: 2, additions: 10, deletions: 3, error: null }, tasks: { tasks: [task], servers: [{ id: "server", taskId: "task", url: "http://localhost:3000", title: "Dev server", status: "running", startedAt: task.startedAt }], error: "", busy: false, canStart: true, start: vi.fn(), stop: vi.fn().mockResolvedValue(undefined), restart: vi.fn(), clearFinished: vi.fn(), refresh: vi.fn() }, tasksEnabled: true, browserEnabled: true, onOpenSettings: vi.fn(), onEditLaunchScripts: vi.fn() };
}
beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); });
afterEach(cleanup);
describe("Code session rail", () => {
  it("persists card collapse and keyboard reorder", () => {
    const data = props(); const view = render(<CodeSessionRail {...data} />);
    fireEvent.click(screen.getByRole("button", { name: "Environment" }));
    fireEvent.click(screen.getByRole("button", { name: "Move Context up" }));
    view.unmount();
    render(<CodeSessionRail {...data} />);
    expect(document.querySelector(".code-session-card-toggle")?.textContent).toBe("Context");
    expect(screen.getByRole("button", { name: "Environment" }).getAttribute("aria-expanded")).toBe("false");
    expect(screen.getByRole("button", { name: "Connectors (1)" })).toBeTruthy();
  });
  it("opens actual file/changes/browser panes and keeps stop separate", () => {
    const data = props(); render(<CodeSessionRail {...data} />);
    fireEvent.click(screen.getByRole("button", { name: "result.md" }));
    expect(data.onOpenFile).toHaveBeenCalledWith("reports/result.md");
    expect(panel.openTool).toHaveBeenLastCalledWith("files");
    fireEvent.click(screen.getByRole("button", { name: "Commit…" }));
    expect(panel.openTool).toHaveBeenLastCalledWith("git");
    fireEvent.click(screen.getByRole("button", { name: "Open server Dev server" }));
    expect(panel.openPage).toHaveBeenCalledWith("http://localhost:3000");
    expect(data.tasks.stop).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Stop server Dev server" }));
    expect(data.tasks.stop).toHaveBeenCalledWith("task");
  });
  it("reorders cards by drag while keeping keyboard controls available", () => {
    render(<CodeSessionRail {...props()} />);
    const dataTransfer = { setData: vi.fn(), effectAllowed: "none" };
    fireEvent.dragStart(screen.getByTitle("Drag to reorder Changes"), { dataTransfer });
    fireEvent.drop(screen.getByRole("region", { name: "Environment" }), { dataTransfer });
    expect(document.querySelector(".code-session-card-toggle")?.textContent).toBe("Changes2");
    expect(screen.getByRole("button", { name: "Move Changes down" })).toBeTruthy();
    expect(JSON.parse(localStorage.getItem("taucode.code.sessionRail")!).order[0]).toBe("Changes");
  });
  it("opens connector configuration from a connector disclosure", () => {
    const data = props(); render(<CodeSessionRail {...data} />);
    fireEvent.click(screen.getByRole("button", { name: "Configure Browser" }));
    expect(data.onOpenSettings).toHaveBeenCalledWith("connectors");
  });
  it("keeps unavailable usage honest and cleanup available when Terminal is off", () => {
    render(<CodeSessionRail {...props()} tasksEnabled={false} />);
    expect(screen.getByText("Usage not reported yet")).toBeTruthy();
    expect(screen.getByText("Skill inventory not reported")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Stop server Dev server" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start a background task" })).toBeNull();
    expect(screen.getByText("Enable Terminal to start tasks.")).toBeTruthy();
  });
  it("saves a named folder profile and leaves the form open on failure", async () => {
    const data = props(); vi.mocked(data.onSaveEnvironment).mockRejectedValueOnce(new Error("Disk full"));
    render(<CodeSessionRail {...data} />);
    fireEvent.click(screen.getByRole("button", { name: "Save environment…" }));
    expect(screen.getByLabelText("Name")).toBe(document.activeElement);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "My setup" } });
    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Disk full"));
    expect(data.onSaveEnvironment).toHaveBeenCalledWith("My setup", ["w"]);
    fireEvent.click(screen.getByRole("button", { name: "Create" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
  it("shows failed process actions instead of unhandled rejections", async () => {
    const data = props(); vi.mocked(data.tasks.stop).mockRejectedValueOnce(new Error("Already exited"));
    render(<CodeSessionRail {...data} />);
    fireEvent.click(screen.getByRole("button", { name: "Stop task Dev server" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Already exited"));
  });
});
