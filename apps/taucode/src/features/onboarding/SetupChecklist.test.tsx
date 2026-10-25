// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  await screen.findByText("Installed");
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
  await screen.findByText("Installed");
});
it("installs pi, checks keys, refreshes on focus, and opens only the requested privacy pane", async () => {
  vi.mocked(getAppSettings).mockResolvedValue({ providers: [{ id: "key", enabled: true, auth: { type: "apiKey" } }] } as AppSettings);
  render(<SetupChecklist onNavigate={vi.fn()} />);
  await screen.findByText("Installed");
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
  await screen.findByText("Installed");
  fireEvent.click(screen.getByText("Open Providers"));
  expect(navigate).toHaveBeenCalledWith("providers");
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent(window, new Event("taucode:setup-checklist"));
  await screen.findByText("Installed");
  fireEvent.click(screen.getByText("Open Features"));
  expect(navigate).toHaveBeenCalledWith("features");
});
