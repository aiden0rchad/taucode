// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ComputerApprovals } from "./ComputerApprovals";
import { ComputerRunItem } from "./ComputerRunItem";
import { defaultComputerSettings, parseComputerRun } from "./computer";
import { SettingsComputerSection } from "@settings/components/sections/SettingsComputerSection";
import { computerRequest, computerRequestPermissions } from "@services/tauri";
import type { AppSettings } from "@/types";
vi.mock("@services/tauri", () => ({ computerRequest: vi.fn(), computerRequestPermissions: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const step = { step: 1, thumbnail: "aGVsbG8=", action: { kind: "click" }, reflection: "Found button" };
describe("computer use", () => {
  it("defaults to background and persists explicit fallback and cursor settings", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={save} />);
    expect((screen.getByLabelText("Input mode") as HTMLSelectElement).value).toBe("background");
    expect((screen.getByLabelText("Allow foreground fallback (approval required for each step)") as HTMLInputElement).checked).toBe(false);
    expect((screen.getByLabelText("Show virtual cursor") as HTMLInputElement).checked).toBe(true);
    fireEvent.change(screen.getByLabelText("Input mode"), { target: { value: "foreground" } });
    fireEvent.click(screen.getByLabelText("Allow foreground fallback (approval required for each step)"));
    fireEvent.click(screen.getByLabelText("Show virtual cursor"));
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ mode: "foreground", allowForegroundFallback: true, virtualCursor: false }) })));
  });
  it.each([
    { screen: false, accessibility: false },
    { screen: true, accessibility: false },
    { screen: false, accessibility: true },
    { screen: true, accessibility: true },
  ])("requests access and opens only missing panes: %j", async permissions => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions: { screen: false, accessibility: false } });
    vi.mocked(computerRequestPermissions).mockResolvedValue(permissions);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Request access" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false));
    expect(computerRequestPermissions).toHaveBeenCalledTimes(1);
    for (const pane of ["screen", "accessibility"] as const) {
      const call = expect(computerRequest);
      if (permissions[pane]) call.not.toHaveBeenCalledWith({ action: "permissions", pane });
      else call.toHaveBeenCalledWith({ action: "permissions", pane });
      expect(screen.getByText(`${pane === "screen" ? "Screen Recording" : "Accessibility"}: ${permissions[pane] ? "Granted" : "Not granted"}`)).toBeTruthy();
    }
  });
  it.each([false, true])("requests once on enable only when permissions are missing (granted: %s)", async granted => {
    const permissions = { screen: granted, accessibility: granted };
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions });
    vi.mocked(computerRequestPermissions).mockResolvedValue(permissions);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByLabelText("Enable Computer Use"));
    await waitFor(() => expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false));
    expect(computerRequestPermissions).toHaveBeenCalledTimes(granted ? 0 : 1);
    fireEvent.click(screen.getByLabelText("Enable Computer Use"));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Refresh permissions" })); });
    expect(computerRequestPermissions).toHaveBeenCalledTimes(granted ? 0 : 1);
  });
  it("explains Screen Recording restart and wires Quit & Reopen", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    expect(screen.getByText("Screen Recording takes effect only after quitting and reopening taucode.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Quit & Reopen" }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "restart" }));
  });
  it("shows request failures and allows retry", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    vi.mocked(computerRequestPermissions).mockRejectedValue(new Error("Permission request failed"));
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Request access" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Permission request failed");
    expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false);
  });
  it("renders persisted tool steps and validates unknown output", () => {
    const run = parseComputerRun(JSON.stringify({ computerRun: { steps: [step], summary: "Done" } }));
    expect(run).not.toBeNull(); expect(parseComputerRun('{"computerRun":{"steps":[{}]}}')).toBeNull();
    render(<ComputerRunItem run={run!} />);
    expect(screen.getByText("Step 1: click")).toBeTruthy(); expect(screen.getByAltText("Screen before step 1")).toBeTruthy(); expect(screen.getByText("Done")).toBeTruthy();
  });
  it("shows provider failures from a stored run in the thread item", async () => {
    const error = "Provider error: Model unavailable";
    vi.mocked(computerRequest).mockResolvedValue({ id: "run-id", steps: [{ ...step, action: { kind: "fail", reason: error }, error }], error });
    render(<ComputerRunItem run={{ id: "run-id", stored: true, steps: [] }} />);
    await screen.findByText("Step 1: fail");
    expect(screen.getAllByText(error).length).toBeGreaterThan(0);
    expect(computerRequest).toHaveBeenCalledWith({ action: "runLog", id: "run-id" });
  });
  it("shows approval reason and target without expanding the thread step", () => {
    render(<ComputerRunItem run={{ steps: [{ ...step, kind: "approval_wait", reason: "Destructive click", target: "Delete account", predictMs: 12, executeMs: 0, waitMs: 900 }] }} />);
    expect(screen.getByText("Approval required: Destructive click (Delete account)")).toBeTruthy();
    expect(screen.getByText("Step 1: Approval wait")).toBeTruthy();
    expect(screen.getByText("Predict: 12 ms · Execute: 0 ms · Wait: 900 ms")).toBeTruthy();
  });
  it.each([true, false])("routes one-action approval from the main UI (accept: %s)", async accept => {
    vi.mocked(computerRequest).mockResolvedValue({ running: true, steps: [step], approval: { id: "approval", reason: "Secure field", action: { kind: "type" }, app: "com.apple.TextEdit" } });
    render(<ComputerApprovals />);
    await screen.findByText("Computer Use approval needed");
    expect(screen.getByText("Secure field")).toBeTruthy();
    fireEvent.keyDown(window, { key: "Enter" });
    expect(computerRequest).not.toHaveBeenCalledWith(expect.objectContaining({ action: "approve" }));
    const globalShortcut = vi.fn();
    window.addEventListener("keydown", globalShortcut);
    fireEvent.keyDown(screen.getByText("Approve", { selector: "button" }), { key: "Enter" });
    window.removeEventListener("keydown", globalShortcut);
    expect(globalShortcut).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText(accept ? "Approve" : "Decline", { selector: "button" }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "approve", id: "approval", accept }));
    expect(screen.queryByText("Allow for session")).toBeNull();
  });
  it("shows registry models, coordinate presets and permission states", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions: { screen: true, accessibility: false } });
    const settings = { computer: defaultComputerSettings, providers: [{ id: "local", name: "Local", enabled: true, kind: "ollama", models: [{ id: "vision", enabled: true }] }] } as AppSettings;
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={settings} onUpdateAppSettings={save} />);
    await screen.findByText("Screen Recording: Granted"); expect(screen.getByText("Accessibility: Not granted")).toBeTruthy();
    expect(screen.getAllByText("Local / vision")).toHaveLength(2);
    expect((screen.getByLabelText("Grounding mode") as HTMLSelectElement).value).toBe("pixels");
    expect(screen.queryByLabelText("Grounding coordinates")).toBeNull();
    fireEvent.change(screen.getByLabelText("Grounding mode"), { target: { value: "ui-tars" } });
    fireEvent.change(screen.getByLabelText("Grounding coordinates"), { target: { value: "1000x1000" } });
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ groundingMode: "ui-tars", groundingWidth: 1000, groundingHeight: 1000 }) })));
    fireEvent.click(screen.getByText("Open System Settings — Accessibility")); expect(computerRequest).toHaveBeenCalledWith({ action: "permissions", pane: "accessibility" });
  });
});
