// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { codegraphStatus, rebuildCodegraph, type CodegraphStatus } from "@services/tauri";
import { subscribeCodegraphStatus } from "@services/events";
import { CodegraphProjectSettings } from "./CodegraphProjectSettings";

vi.mock("@services/tauri", () => ({ codegraphStatus: vi.fn(), rebuildCodegraph: vi.fn() }));
vi.mock("@services/events", () => ({ subscribeCodegraphStatus: vi.fn() }));
const workspace = { id: "ws", name: "Project", path: "/project", connected: true, settings: { sidebarCollapsed: false } };
let listener: (id: string, status: CodegraphStatus) => void;
const unsubscribe = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(codegraphStatus).mockResolvedValue({ state: "ready", detail: "Private index is ready." });
  vi.mocked(rebuildCodegraph).mockResolvedValue({ state: "indexing", detail: "Building private index…" });
  vi.mocked(subscribeCodegraphStatus).mockImplementation((callback) => { listener = callback; return unsubscribe; });
});
afterEach(cleanup);

describe("CodeGraph project settings", () => {
  it("inherits settings, persists overrides, rebuilds and follows status events", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    const view = render(<CodegraphProjectSettings workspace={workspace} globalEnabled onUpdate={onUpdate} />);
    await screen.findByText("ready: Private index is ready.");
    const toggle = screen.getByRole("button", { name: "CodeGraph for Project" });
    expect(toggle.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(toggle);
    await waitFor(() => expect(onUpdate).toHaveBeenCalledWith("ws", { codegraphEnabled: false }));
    await waitFor(() => expect(toggle.hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Rebuild index" }));
    await screen.findByText("indexing: Building private index…");
    expect(rebuildCodegraph).toHaveBeenCalledWith("ws");
    expect(screen.getByRole("button", { name: "Rebuild index" }).hasAttribute("disabled")).toBe(true);
    act(() => listener("other", { state: "failed", detail: "Wrong workspace" }));
    expect(screen.queryByText(/Wrong workspace/)).toBeNull();
    act(() => listener("ws", { state: "failed", detail: "Indexing timed out" }));
    await screen.findByText("failed: Indexing timed out");
    view.unmount();
    expect(unsubscribe).toHaveBeenCalled();
  });

  it("resets an override and respects the global disable", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(<CodegraphProjectSettings workspace={{ ...workspace, settings: { sidebarCollapsed: false, codegraphEnabled: false } }} globalEnabled={false} onUpdate={onUpdate} />);
    expect(screen.getByRole("status").textContent).toBe("disabled");
    expect(screen.getByRole("button", { name: "CodeGraph for Project" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Use app setting" }));
    await waitFor(() => expect(onUpdate).toHaveBeenCalledWith("ws", { codegraphEnabled: null }));
  });

  it("renders unavailable status and rebuild errors", async () => {
    vi.mocked(codegraphStatus).mockResolvedValue({ state: "unavailable", detail: "Install codegraph" });
    vi.mocked(rebuildCodegraph).mockRejectedValue(new Error("Daemon disconnected"));
    render(<CodegraphProjectSettings workspace={workspace} globalEnabled onUpdate={vi.fn()} />);
    await screen.findByText("unavailable: Install codegraph");
    fireEvent.click(screen.getByRole("button", { name: "Rebuild index" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("alert").textContent).toContain("Daemon disconnected");
  });
});
