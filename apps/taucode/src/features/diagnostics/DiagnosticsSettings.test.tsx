// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { diagnosticsRequest } from "@services/tauri";
import { buildDefaultSettings } from "@settings/hooks/useAppSettings";
import { createPreviewHandler } from "@/dev/uiPreview/handler";
import { DiagnosticsSettings } from "./DiagnosticsSettings";
import { DiagnosticBoundary } from "./DiagnosticBoundary";

vi.mock("@services/tauri", () => ({ diagnosticsRequest: vi.fn() }));
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllEnvs(); window.history.replaceState({}, "", "/"); });
function setup(search = "") {
  const { handler } = createPreviewHandler(undefined, search);
  vi.mocked(diagnosticsRequest).mockImplementation(async (action, params) => handler("diagnostics_request", { action, params }) as never);
  const appSettings = buildDefaultSettings();
  appSettings.features = { ...appSettings.features, diagnostics: true };
  const update = vi.fn().mockResolvedValue(undefined);
  render(<DiagnosticsSettings appSettings={appSettings} onUpdateAppSettings={update} />);
  return update;
}
it("renders fixture stats, filters issues, and exposes evidence and notes", async () => {
  setup();
  await screen.findByText("Engine failed to start");
  expect(document.querySelectorAll(".diagnostics-issue")).toHaveLength(6);
  expect(screen.getByText("Regressed")).toBeTruthy();
  expect(screen.getByText("27")).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "Status" }), { target: { value: "acknowledged" } });
  await waitFor(() => expect(document.querySelectorAll(".diagnostics-issue")).toHaveLength(1));
  fireEvent.click(screen.getByText("Sidebar panel could not render"));
  const detail = await screen.findByRole("region", { name: "Issue details" });
  expect(within(detail).getByText("Evidence sample")).toBeTruthy();
  expect(within(detail).getByText("fixture-model")).toBeTruthy();
  expect(within(detail).getByText(/Acknowledged: checking/)).toBeTruthy();
  fireEvent.change(within(detail).getByLabelText("Note"), { target: { value: "Verified the repair" } });
  fireEvent.click(within(detail).getByText("Mark fixed"));
  await screen.findByText("Verified the repair");
  expect(vi.mocked(diagnosticsRequest)).toHaveBeenCalledWith("diag_annotate", expect.objectContaining({ status: "fixed", note: "Verified the repair" }));
});
it("uses segmented levels, retention choices, copy feedback and confirmed clearing", async () => {
  const update = setup();
  await screen.findByText("Engine failed to start");
  fireEvent.click(screen.getByRole("button", { name: "Verbose" }));
  expect(update).toHaveBeenCalledWith(expect.objectContaining({ diagnostics: expect.objectContaining({ level: "verbose" }) }));
  const retention = screen.getByRole("combobox", { name: "Retention" });
  expect(within(retention).getAllByRole("option").map(option => option.getAttribute("value"))).toEqual(["1", "3", "7", "14", "30"]);
  fireEvent.change(retention, { target: { value: "14" } });
  expect(update).toHaveBeenCalledWith(expect.objectContaining({ diagnostics: expect.objectContaining({ retention: 14 }) }));
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  fireEvent.click(screen.getByRole("button", { name: "Copy Watcher prompt" }));
  await screen.findByRole("button", { name: "Watcher prompt copied" });
  expect(writeText).toHaveBeenCalledWith(expect.stringContaining("/loop 30m"));
  fireEvent.click(screen.getByText("Clear all data"));
  expect(vi.mocked(diagnosticsRequest)).not.toHaveBeenCalledWith("clear", expect.anything());
  fireEvent.click(screen.getByText("Confirm clear"));
  await screen.findByText("No issues. Turn on Diagnostics and use taucode normally.");
});
it("supports the empty preview switch", async () => {
  setup("?diagnostics=empty");
  await screen.findByText("Not generated", { exact: false });
  expect(document.querySelectorAll(".diagnostics-issue")).toHaveLength(0);
  expect(screen.getByText("No issues. Turn on Diagnostics and use taucode normally.")).toBeTruthy();
});
it("injects a recoverable sidebar crash only in preview", () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  window.history.replaceState({}, "", "/?crash=sidebar");
  vi.stubEnv("MODE", "production");
  const view = render(<DiagnosticBoundary panel="sidebar"><p>Sidebar ready</p></DiagnosticBoundary>);
  expect(screen.queryByRole("alert")).toBeNull();
  view.unmount();
  vi.stubEnv("MODE", "uipreview");
  render(<DiagnosticBoundary panel="sidebar"><p>Sidebar ready</p></DiagnosticBoundary>);
  expect(screen.getByRole("alert")).toBeTruthy();
  fireEvent.click(screen.getByText("Reload panel"));
  expect(screen.getByText("Sidebar ready")).toBeTruthy();
});
