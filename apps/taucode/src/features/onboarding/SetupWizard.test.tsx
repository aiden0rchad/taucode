// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SetupWizard } from "./SetupWizard";
import { previewStep, SETUP_PROGRESS, SETUP_SEEN, steps, type SetupStep } from "./setupState";
import * as api from "@services/tauri";
import { buildDefaultSettings } from "@settings/hooks/useAppSettings";
import { applyTheme } from "@/features/themes/applyTheme";
import type { AppSettings, WorkspaceInfo } from "@/types";

vi.mock("@services/tauri", () => ({ getAppSettings: vi.fn(), updateAppSettings: vi.fn(), getSetupStatus: vi.fn(), engineRuntime: vi.fn(), listWorkspaces: vi.fn(), pickWorkspacePaths: vi.fn(), addWorkspace: vi.fn(), isWorkspacePathDir: vi.fn(), updateWorkspaceSettings: vi.fn(), providerHasApiKey: vi.fn(), providerSetApiKey: vi.fn(), providerTestAndFetch: vi.fn(), openSetupPrivacy: vi.fn() }));
vi.mock("@/features/themes/store", () => ({ useThemes: () => ({ files: [] }) }));
vi.mock("@/features/themes/applyTheme", () => ({ applyTheme: vi.fn() }));
vi.mock("@services/events", () => ({ notifySetupChanged: vi.fn(), subscribeAppServerEvents: () => () => {}, subscribeImportsChanged: () => () => {}, subscribeSetupChanged: () => () => {} }));
let drop: (event: { payload: { type: string; paths: string[] } }) => void;
vi.mock("@services/dragDrop", () => ({ subscribeWindowDragDrop: (fn: typeof drop) => { drop = fn; return () => {}; } }));
let settings: AppSettings;
const folder: WorkspaceInfo = { id: "atlas", path: "/Projects/atlas", name: "atlas", connected: false, settings: { sidebarCollapsed: false } };
const finish = vi.fn();
const ready = [{ engine: "pi", state: "ready", message: "Ready" }, { engine: "omp", state: "pending", message: "Will install" }];
beforeEach(() => {
  vi.clearAllMocks(); localStorage.clear(); window.history.replaceState(null, "", "/");
  settings = buildDefaultSettings();
  vi.mocked(api.getAppSettings).mockImplementation(async () => settings);
  vi.mocked(api.updateAppSettings).mockImplementation(async next => { settings = next; return next; });
  vi.mocked(api.getSetupStatus).mockResolvedValue({ codegraph: true, permissions: { screen: true, accessibility: false } });
  vi.mocked(api.engineRuntime).mockResolvedValue(ready);
  vi.mocked(api.listWorkspaces).mockResolvedValue([]);
  vi.mocked(api.pickWorkspacePaths).mockResolvedValue([folder.path]);
  vi.mocked(api.isWorkspacePathDir).mockResolvedValue(true);
  vi.mocked(api.addWorkspace).mockResolvedValue(folder);
  vi.mocked(api.updateWorkspaceSettings).mockImplementation(async (_id, s) => ({ ...folder, settings: s }));
  vi.mocked(api.providerHasApiKey).mockResolvedValue(false);
  vi.mocked(api.providerSetApiKey).mockResolvedValue();
  vi.mocked(api.providerTestAndFetch).mockResolvedValue({ ok: true, latencyMs: 900, models: [{ id: "model-1", enabled: true }, { id: "model-2", enabled: true }] });
  vi.mocked(api.openSetupPrivacy).mockResolvedValue();
  finish.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllEnvs(); });
async function start(step: SetupStep = "welcome") {
  localStorage.setItem(SETUP_PROGRESS, JSON.stringify({ step, completed: [], skipped: [] }));
  const view = render(<SetupWizard onSettings={vi.fn()} onFinish={finish} />);
  await waitFor(() => expect(api.getAppSettings).toHaveBeenCalled());
  await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toBe(document.activeElement));
  await act(async () => {});
  return view;
}
async function click(name: string) { fireEvent.click(screen.getByRole("button", { name })); await act(async () => {}); }
async function heading(text: string) { await waitFor(() => expect(screen.getByRole("heading", { level: 1 }).textContent?.trim()).toBe(text)); }

it("navigates with direction, heading focus, Back and persisted skip", async () => {
  await start(); await click("Get started");
  expect(document.querySelector(".setup-forward.setup-exit")).toBeTruthy();
  await heading("Pick a look."); expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
  await click("Back"); expect(document.querySelector(".setup-back.setup-exit")).toBeTruthy();
  await heading("Welcome to taucode"); await click("Get started"); await heading("Pick a look.");
  await click("Skip for now"); await heading("Connect a model provider.");
  expect(JSON.parse(localStorage.getItem(SETUP_PROGRESS)!).skipped).toEqual(["look"]);
});
it("confirms Escape on first run, persists dismissal, and reopens a Squircle modal", async () => {
  const view = await start("look");
  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(screen.getByRole("alertdialog")).toBeTruthy(); await click("Confirm set up later");
  expect(localStorage.getItem(SETUP_SEEN)).toBe("done"); expect(screen.queryByRole("dialog")).toBeNull();
  view.unmount(); render(<SetupWizard onSettings={vi.fn()} onFinish={finish} />);
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent(window, new Event("taucode:run-setup")); await heading("Pick a look.");
  expect(document.querySelector(".setup-modal-card.squircle")).toBeTruthy();
  fireEvent.keyDown(document.activeElement!, { key: "Escape" }); expect(screen.queryByRole("dialog")).toBeNull();
});
it("applies the theme live and saves it on Continue; a save failure stays inline", async () => {
  await start("look"); await click("Claude Dark");
  expect(applyTheme).toHaveBeenCalledWith(expect.objectContaining({ id: "claude-dark" }), true, true, false);
  vi.mocked(api.updateAppSettings).mockRejectedValueOnce(new Error("Disk full"));
  await click("Continue"); expect(screen.getByRole("alert").textContent).toContain("Disk full"); await heading("Pick a look.");
  await click("Continue"); await heading("Connect a model provider."); expect(settings.theme).toBe("claude-dark");
});
it("stores keys only through Keychain, clears the field, tests, and saves a searchable default model", async () => {
  const logs = vi.spyOn(console, "log"), errors = vi.spyOn(console, "error");
  await start("provider"); await click("OpenAI");
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "private-test-key" } });
  await click("Test & continue");
  expect(api.providerSetApiKey).toHaveBeenCalledWith(expect.any(String), "private-test-key");
  expect(document.body.innerHTML).not.toContain("private-test-key");
  expect(JSON.stringify(vi.mocked(api.updateAppSettings).mock.calls)).not.toContain("private-test-key");
  expect(JSON.stringify([...logs.mock.calls, ...errors.mock.calls])).not.toContain("private-test-key");
  expect(screen.getByText("✓ Connected · 2 models")).toBeTruthy();
  fireEvent.change(screen.getByPlaceholderText("Search models…"), { target: { value: "model-2" } });
  expect(screen.queryByRole("button", { name: "model-1" })).toBeNull();
  await click("model-2"); await click("Continue"); await heading("Choose your engine.");
  expect(settings.agentPolicy?.defaultModel).toEqual({ providerId: settings.providers![0].id, modelId: "model-2" });
});
it("keeps failed provider tests on the step with a cleared key", async () => {
  await start("provider"); await click("OpenAI");
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "bad-key" } });
  vi.mocked(api.providerTestAndFetch).mockResolvedValueOnce({ ok: false, latencyMs: 900, models: [], error: "Key rejected" });
  await click("Test & continue"); expect(screen.getByRole("alert").textContent).toContain("Key rejected");
  expect((screen.getByLabelText("API key") as HTMLInputElement).value).toBe(""); await heading("Connect a model provider.");
});
it("enables an engine after prepare and supports cancellation without advancing", async () => {
  await start("engine"); await click("oh-my-pi");
  let resolve!: (value: typeof ready) => void;
  vi.mocked(api.engineRuntime).mockImplementation(async action => action === "prepare" ? new Promise(r => { resolve = r; }) : ready);
  await click("Continue"); expect(api.engineRuntime).toHaveBeenCalledWith("prepare", "omp");
  expect(screen.getByRole("progressbar")).toBeTruthy(); await click("Cancel");
  expect(api.engineRuntime).toHaveBeenCalledWith("cancel", "omp");
  await act(async () => resolve([{ engine: "omp", state: "ready", message: "Ready" }]));
  await heading("Choose your engine."); expect(settings.agentPolicy?.defaultEngine).toBe("pi");
  await click("Continue"); await heading("Open your first project.");
  expect(settings.agentPolicy?.defaultEngine).toBe("omp"); expect(settings.engines?.find(e => e.id === "omp")?.enabled).toBe(true);
});
it("chooses and adds a folder through workspace services, including codegraph", async () => {
  await start("project"); await click("Choose a folder…");
  await click("Code intelligence (codegraph)"); await click("Continue"); await heading("Turn on what you want.");
  expect(api.addWorkspace).toHaveBeenCalledWith(folder.path);
  expect(api.updateWorkspaceSettings).toHaveBeenCalledWith(folder.id, { sidebarCollapsed: false, codegraphEnabled: false });
});
it("accepts a native drop, rejects files, and hides unavailable codegraph", async () => {
  vi.mocked(api.getSetupStatus).mockResolvedValue({ codegraph: false, permissions: { screen: false, accessibility: false } });
  await start("project"); act(() => drop({ payload: { type: "drop", paths: ["/not-a-folder"] } }));
  expect(screen.queryByRole("button", { name: "Code intelligence (codegraph)" })).toBeNull();
  vi.mocked(api.isWorkspacePathDir).mockResolvedValue(false); await click("Continue");
  expect(screen.getByRole("alert").textContent).toContain("valid project folder"); expect(api.addWorkspace).not.toHaveBeenCalled();
});
it("saves feature defaults and omits Permissions when computer use is off", async () => {
  await start("features"); await click("Computer use"); await click("Git"); await click("Continue");
  await heading("You're all set."); expect(settings.features).toMatchObject({ computerUse: false, git: false, terminal: true });
  expect(within(screen.getByRole("list", { name: "Setup progress" })).queryByText("Permissions")).toBeNull();
});
it("rechecks permission status on focus and opens only the requested pane", async () => {
  await start("permissions");
  expect(api.openSetupPrivacy).not.toHaveBeenCalled();
  expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Skip for now" }) as HTMLButtonElement).disabled).toBe(false);
  const screenRow = within(screen.getByRole("group", { name: "Screen Recording" }));
  expect(screenRow.getByRole("status").textContent).toBe("Granted");
  fireEvent.click(screenRow.getByRole("button")); expect(api.openSetupPrivacy).toHaveBeenCalledWith("screen");
  const access = within(screen.getByRole("group", { name: "Accessibility" }));
  fireEvent.click(access.getByRole("button")); expect(api.openSetupPrivacy).toHaveBeenCalledWith("accessibility");
  vi.mocked(api.getSetupStatus).mockResolvedValue({ codegraph: true, permissions: { screen: true, accessibility: true } });
  fireEvent(window, new Event("focus")); await waitFor(() => expect(access.getByRole("status").textContent).toBe("Granted"));
  expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(false);
});
it("resumes progress, shows skipped recap, and completes to Chat without a project", async () => {
  localStorage.setItem(SETUP_PROGRESS, JSON.stringify({ step: "done", completed: ["look", "features"], skipped: ["provider", "engine", "project", "permissions"] }));
  render(<SetupWizard onSettings={vi.fn()} onFinish={finish} />); await act(async () => {});
  await heading("You're all set."); expect(document.querySelectorAll('.setup-recap [data-complete="true"]')).toHaveLength(2);
  expect(within(document.querySelector(".setup-recap") as HTMLElement).getAllByText("Set up later")).toHaveLength(4);
  await click("Start building"); expect(settings.setupComplete).toBe(true); expect(finish).toHaveBeenCalledWith("chat", undefined);
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("preselects an existing project and lands in Code", async () => {
  vi.mocked(api.listWorkspaces).mockResolvedValue([folder]); await start("done"); await click("Start building");
  expect(finish).toHaveBeenCalledWith("code", folder.id); expect(api.addWorkspace).not.toHaveBeenCalled();
});
it("moves card focus with arrows", async () => {
  await start("look"); const first = screen.getByRole("button", { name: "System" }); first.focus();
  fireEvent.keyDown(first, { key: "ArrowRight" }); expect(document.activeElement).toBe(screen.getByRole("button", { name: "Dark" }));
});
it.each(steps)("routes preview %s", step => { expect(previewStep(`?setup=${step}`)).toBe(step); });
it("only accepts valid preview flags", () => { expect(previewStep("?setup=1")).toBe("welcome"); expect(previewStep("?setup=0")).toBeUndefined(); expect(previewStep("?setup=oops")).toBeUndefined(); });
