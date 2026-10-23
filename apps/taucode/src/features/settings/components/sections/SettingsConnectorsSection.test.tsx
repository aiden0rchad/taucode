// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, McpServerEntry, WorkspaceInfo } from "@/types";
import { connectorRequest } from "@services/tauri";
import { SettingsConnectorsSection } from "./SettingsConnectorsSection";

vi.mock("@services/tauri", () => ({ connectorRequest: vi.fn() }));
const entry: McpServerEntry = { id: "example", name: "Example", enabled: true, approval: "ask", transport: { type: "http", url: "https://example.test/mcp", headers: {}, bearer: { secret: true } } };
const updateWorkspace = vi.fn();
const saveSettings = vi.fn();
function Harness() {
  const [settings, setSettings] = useState({ codegraphEnabled: true, mcpServers: [entry] } as AppSettings);
  const [workspace, setWorkspace] = useState({ id: "ws", name: "Project", settings: { sidebarCollapsed: false } } as WorkspaceInfo);
  return <SettingsConnectorsSection appSettings={settings} onUpdateAppSettings={async s => { saveSettings(s); setSettings(s); }} workspace={workspace} onUpdateWorkspaceSettings={async (id, patch) => { updateWorkspace(id, patch); setWorkspace({ ...workspace, settings: { ...workspace.settings, ...patch } }); }} />;
}
describe("Connectors settings", () => {
  afterEach(cleanup);
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(connectorRequest).mockImplementation(async params => params.action === "secrets" ? { bearer: true } : params.action === "test" ? { ok: true, latencyMs: 7, tools: [{ name: "read", description: "Reads a document" }] } : null);
  });
  it("masks saved credentials and sends secret values only to the credential command", async () => {
    render(<Harness />);
    expect(screen.queryByText("Delete CodeGraph")).toBeNull();
    fireEvent.click(screen.getByText("Edit Example"));
    await screen.findByText("Saved in Keychain");
    const token = screen.getByLabelText("Bearer token");
    expect(token.getAttribute("type")).toBe("password");
    expect((token as HTMLInputElement).value).toBe("");
    fireEvent.change(token, { target: { value: "private-token" } });
    fireEvent.click(screen.getByText("Save connector"));
    await waitFor(() => expect(connectorRequest).toHaveBeenCalledWith({ action: "setSecret", id: "example", name: "bearer", value: "private-token", workspaceId: undefined }));
    expect(JSON.stringify(saveSettings.mock.calls)).not.toContain("private-token");
    await waitFor(() => expect((token as HTMLInputElement).value).toBe(""));
  });
  it("shows tools, descriptions, latency and actionable errors", async () => {
    render(<Harness />);
    fireEvent.click(screen.getByText("Test Example"));
    await screen.findByText("Connected · 7 ms · 1 tools");
    expect(screen.getByText("Reads a document")).toBeTruthy();
    vi.mocked(connectorRequest).mockResolvedValueOnce({ ok: false, latencyMs: 3, error: "This server requires OAuth sign-in, which taucode doesn't support." });
    fireEvent.click(screen.getByText("Test Example"));
    expect((await screen.findByRole("alert")).textContent).toContain("OAuth sign-in");
  });
  it("persists workspace toggles and restores inheritance", async () => {
    render(<Harness />);
    fireEvent.change(screen.getByLabelText("Scope"), { target: { value: "workspace" } });
    fireEvent.click(screen.getByLabelText("Example"));
    await waitFor(() => expect(updateWorkspace).toHaveBeenCalledWith("ws", { mcpServers: [{ id: "example", enabled: false }] }));
    expect(saveSettings).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Use global setting for Example"));
    await waitFor(() => expect(updateWorkspace).toHaveBeenLastCalledWith("ws", { mcpServers: [] }));
  });
  it("adds workspace-only command connectors and rejects reserved IDs", async () => {
    render(<Harness />);
    fireEvent.change(screen.getByLabelText("Scope"), { target: { value: "workspace" } });
    fireEvent.click(screen.getByText("Add connector"));
    fireEvent.change(screen.getByLabelText("Connector ID"), { target: { value: "taucode-browser" } });
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Local" } });
    fireEvent.change(screen.getByLabelText("Command"), { target: { value: "server" } });
    fireEvent.click(screen.getByText("Save connector"));
    expect((await screen.findByRole("alert")).textContent).toContain("reserved");
    fireEvent.change(screen.getByLabelText("Connector ID"), { target: { value: "local" } });
    fireEvent.click(screen.getByText("Save connector"));
    await waitFor(() => expect(updateWorkspace).toHaveBeenCalledWith("ws", { mcpServers: [expect.objectContaining({ id: "local", transport: { type: "stdio", command: "server", args: [], env: {} } })] }));
  });
});
