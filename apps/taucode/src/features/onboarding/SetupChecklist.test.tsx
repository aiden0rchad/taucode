// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SetupChecklist, SETUP_SEEN } from "./SetupChecklist";
import { engineRuntime, getAppSettings, getSetupStatus, openSetupPrivacy, providerHasApiKey } from "@services/tauri";
import type { AppSettings } from "@/types";

vi.mock("@services/tauri", () => ({ engineRuntime: vi.fn(), getAppSettings: vi.fn(), getSetupStatus: vi.fn(), openSetupPrivacy: vi.fn(), providerHasApiKey: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks(); localStorage.clear();
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "pi", state: "pending", message: "Ready to install" }]);
  vi.mocked(getSetupStatus).mockResolvedValue({ permissions: { screen: true, accessibility: false }, codegraph: true });
  vi.mocked(getAppSettings).mockResolvedValue({ providers: [{ id: "local", enabled: true, auth: { type: "none" } }], features: { diagnostics: false } } as AppSettings);
  vi.mocked(providerHasApiKey).mockResolvedValue(true);
  vi.mocked(openSetupPrivacy).mockResolvedValue();
});
afterEach(cleanup);

it.each(["Skip for now", "Done"])("shows once and persists %s; About can reopen it", async label => {
  const view = render(<SetupChecklist onNavigate={vi.fn()} />);
  expect(screen.getByRole("dialog")).toBeTruthy();
  await waitFor(() => expect(within(screen.getByRole("group", { name: "Codegraph" })).getByRole("status").textContent).toBe("Ready"));
  expect(screen.getByText("Granted")).toBeTruthy();
  expect(screen.getByText("Not granted")).toBeTruthy();
  expect(screen.getByText("Off")).toBeTruthy();
  fireEvent.click(screen.getByText(label));
  expect(localStorage.getItem(SETUP_SEEN)).toBe("done");
  view.unmount();
  render(<SetupChecklist onNavigate={vi.fn()} />);
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent(window, new Event("taucode:setup-checklist"));
  expect(screen.getByRole("dialog")).toBeTruthy();
  await waitFor(() => expect(within(screen.getByRole("group", { name: "Codegraph" })).getByRole("status").textContent).toBe("Ready"));
});
it("installs pi, checks keys, refreshes on focus, and opens only the requested privacy pane", async () => {
  vi.mocked(getAppSettings).mockResolvedValue({ providers: [{ id: "key", enabled: true, auth: { type: "apiKey" } }] } as AppSettings);
  render(<SetupChecklist onNavigate={vi.fn()} />);
  await waitFor(() => expect(within(screen.getByRole("group", { name: "Codegraph" })).getByRole("status").textContent).toBe("Ready"));
  expect(providerHasApiKey).toHaveBeenCalledWith("key");
  expect(openSetupPrivacy).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Open System Settings — Screen Recording" }));
  fireEvent.click(screen.getByRole("button", { name: "Open System Settings — Accessibility" }));
  expect(openSetupPrivacy).toHaveBeenNthCalledWith(1, "screen");
  expect(openSetupPrivacy).toHaveBeenNthCalledWith(2, "accessibility");
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "pi", state: "ready", message: "Ready v1.4.0" }]);
  fireEvent.click(screen.getByText("Install pi"));
  await screen.findByText("Manage engines");
  expect(engineRuntime).toHaveBeenCalledWith("prepare", "pi");
  fireEvent.focus(window);
  await waitFor(() => expect(getSetupStatus).toHaveBeenCalledTimes(2));
});
it("navigates to Providers and Features without blocking use", async () => {
  const navigate = vi.fn(); render(<SetupChecklist onNavigate={navigate} />);
  await waitFor(() => expect(within(screen.getByRole("group", { name: "Codegraph" })).getByRole("status").textContent).toBe("Ready"));
  fireEvent.click(screen.getByText("Open Providers"));
  expect(navigate).toHaveBeenCalledWith("providers");
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent(window, new Event("taucode:setup-checklist"));
  await waitFor(() => expect(within(screen.getByRole("group", { name: "Codegraph" })).getByRole("status").textContent).toBe("Ready"));
  fireEvent.click(screen.getByText("Open Features"));
  expect(navigate).toHaveBeenCalledWith("features");
});

const rowStatus = (name: string) => within(screen.getByRole("group", { name })).getByRole("status");

it("renders six rows with one chip, at most one action, and essential-only progress", async () => {
  render(<SetupChecklist onNavigate={vi.fn()} />);
  await screen.findByText("1 of 2 essentials ready");
  const rows = screen.getAllByRole("group");
  expect(rows).toHaveLength(6);
  for (const row of rows) {
    expect(within(row).getAllByRole("status")).toHaveLength(1);
    expect(within(row).queryAllByRole("button").length).toBeLessThanOrEqual(1);
    expect(row.querySelector(".setup-icon svg")).toBeTruthy();
    expect(row.querySelector(".setup-row-copy p")?.textContent).toBeTruthy();
  }
  expect(screen.getByRole("progressbar")).toHaveProperty("value", 1);
  expect(screen.getByRole("progressbar")).toHaveProperty("max", 2);
  expect(document.querySelector(".setup-checklist-card.squircle.squircle-clip")).toBeTruthy();
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "pi", state: "ready", message: "Ready" }]);
  fireEvent.click(screen.getByRole("button", { name: "Install pi" }));
  await screen.findByText("2 of 2 essentials ready");
  // Optional permissions and tools never change essential progress.
  vi.mocked(getSetupStatus).mockResolvedValue({ permissions: { screen: false, accessibility: false }, codegraph: false });
  fireEvent.focus(window);
  await waitFor(() => expect(rowStatus("Codegraph").textContent).toBe("Optional"));
  expect(screen.getByRole("progressbar")).toHaveProperty("value", 2);
});

it.each([false, true])("maps chip labels to semantic tones (configured: %s)", async ready => {
  vi.mocked(getAppSettings).mockResolvedValue({ providers: ready ? [{ id: "local", enabled: true, auth: { type: "none" } }] : [], features: { diagnostics: ready } } as AppSettings);
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "pi", state: ready ? "ready" : "pending", message: "Engine status" }]);
  vi.mocked(getSetupStatus).mockResolvedValue({ permissions: { screen: ready, accessibility: ready }, codegraph: ready });
  render(<SetupChecklist onNavigate={vi.fn()} />);
  await waitFor(() => expect(rowStatus("Providers").textContent).toBe(ready ? "Ready" : "Needs setup"));
  for (const [name, label, tone] of [
    ["Providers", ready ? "Ready" : "Needs setup", ready ? "success" : "warning"],
    ["Engines", ready ? "Ready" : "Needs setup", ready ? "success" : "warning"],
    ["Screen Recording", ready ? "Granted" : "Not granted", ready ? "success" : "warning"],
    ["Accessibility", ready ? "Granted" : "Not granted", ready ? "success" : "warning"],
    ["Codegraph", ready ? "Ready" : "Optional", ready ? "success" : "muted"],
    ["Diagnostics", ready ? "On" : "Off", ready ? "success" : "muted"],
  ]) {
    expect(rowStatus(name).textContent).toBe(label);
    expect(rowStatus(name).dataset.tone).toBe(tone);
  }
  expect(screen.getByRole("progressbar")).toHaveProperty("value", ready ? 2 : 0);
});

it("shows spinners while checking and keeps Done enabled", () => {
  vi.mocked(getSetupStatus).mockReturnValue(new Promise(() => {}));
  vi.mocked(engineRuntime).mockReturnValue(new Promise(() => {}));
  vi.mocked(getAppSettings).mockReturnValue(new Promise(() => {}));
  render(<SetupChecklist onNavigate={vi.fn()} />);
  for (const chip of screen.getAllByRole("status")) {
    expect(chip.textContent).toBe("Checking");
    expect(chip.querySelector(".setup-spinner")).toBeTruthy();
  }
  expect(screen.getByRole("button", { name: "Done" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("replaces failed checks with red error chips and message tooltips, then recovers on focus", async () => {
  vi.mocked(getSetupStatus).mockRejectedValue(new Error("Permission preflight failed"));
  vi.mocked(engineRuntime).mockRejectedValue(new Error("Runtime unavailable"));
  vi.mocked(getAppSettings).mockRejectedValue(new Error("Settings unavailable"));
  render(<SetupChecklist onNavigate={vi.fn()} />);
  await waitFor(() => expect(screen.getAllByText("Error")).toHaveLength(6));
  for (const chip of screen.getAllByRole("status")) {
    expect(chip.dataset.tone).toBe("error");
    expect(chip.title).toMatch(/failed|unavailable/);
    expect(chip.getAttribute("aria-label")).toContain(chip.title);
  }
  expect(screen.getByRole("progressbar")).toHaveProperty("value", 0);
  expect(screen.getByRole("button", { name: "Done" })).toHaveProperty("disabled", false);
  vi.mocked(getSetupStatus).mockResolvedValue({ permissions: { screen: true, accessibility: false }, codegraph: true });
  fireEvent.focus(window);
  await waitFor(() => expect(rowStatus("Screen Recording").textContent).toBe("Granted"));
  expect(rowStatus("Screen Recording").title).toBe("");
  expect(openSetupPrivacy).not.toHaveBeenCalled();
});
