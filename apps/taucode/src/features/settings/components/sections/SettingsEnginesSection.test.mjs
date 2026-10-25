// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createElement, Fragment } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { engineRuntime, pickEngineExecutable, testEngine } from "@services/tauri";
import { SettingsEnginesSection } from "./SettingsEnginesSection";

vi.mock("@services/tauri", () => ({ engineRuntime: vi.fn(), testEngine: vi.fn(), pickEngineExecutable: vi.fn() }));
const custom = { id: "gemini", name: "Gemini CLI", protocol: "acp", command: "gemini", args: ["--experimental-acp"], builtIn: false, enabled: true };
const save = vi.fn().mockResolvedValue(undefined), flags = vi.fn(), setup = vi.fn();
function show(engines = [custom]) {
  return render(createElement(Fragment, null, createElement("style", null, readFileSync("src/styles/settings.css", "utf8") + readFileSync("src/features/settings/components/sections/settings-engines.css", "utf8")), createElement(SettingsEnginesSection, { appSettings: { engines }, onUpdateAppSettings: save, onOpenCodexFlags: flags, onOpenCodexSetup: setup })));
}
beforeEach(() => {
  vi.mocked(engineRuntime).mockResolvedValue([
    { engine: "pi", state: "ready", message: "Ready v1.4.0" },
    { engine: "omp", state: "pending", message: "Not installed" },
    { engine: "codex", state: "ready", message: "Ready v0.1.0" },
    { engine: "claude-acp", state: "error", message: "Runtime failed to start" },
  ]);
  vi.mocked(testEngine).mockResolvedValue({ engine: custom.id, state: "ready", message: "Protocol handshake passed · v1.2.3", version: "1.2.3" });
  vi.mocked(pickEngineExecutable).mockResolvedValue("/bin/gemini");
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("keeps each engine in one row with one action column and only offers needed primary actions", async () => {
  const { container } = show();
  await screen.findByText("Ready · v1.4.0");
  const rows = container.querySelectorAll(".engine-row");
  expect(rows).toHaveLength(5);
  for (const row of rows) {
    expect(row.children).toHaveLength(2);
    expect(getComputedStyle(row).gridTemplateColumns).toBe("minmax(0, 1fr) auto");
    expect(row.querySelectorAll(":scope > .engine-actions")).toHaveLength(1);
    expect(row.children[0].querySelector("strong")).toBeTruthy();
  }
  const pi = screen.getByLabelText("pi engine");
  expect(within(pi).getByText("Default")).toBeTruthy();
  expect(within(pi).queryByRole("button", { name: "Install" })).toBeNull();
  expect(within(pi).queryByRole("button", { name: "Set as default" })).toBeNull();
  const omp = screen.getByLabelText("oh-my-pi engine");
  expect(omp.classList.contains("is-disabled")).toBe(true);
  expect(within(omp).getByText("Disabled")).toBeTruthy();
  expect(within(omp).getByText("~1.1 GB")).toBeTruthy();
  expect(within(omp).getByRole("button", { name: "Install" })).toBeTruthy();
  expect(within(screen.getByLabelText("Claude Code (ACP) engine")).getByRole("button", { name: "Retry" })).toBeTruthy();
  expect(within(screen.getByRole("combobox", { name: "Default engine" })).getAllByRole("option").map(e => e.textContent)).toEqual(["pi", "Codex"]);
  expect(screen.getByRole("button", { name: "Add engine" }).closest(".settings-group-title")).toBeTruthy();
});

it("shows progress and Cancel while installing, with no Install action", async () => {
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "omp", state: "installing", message: "Installing", progress: 42 }]);
  show([{ ...custom, id: "omp", builtIn: true }]);
  const progress = await screen.findByRole("progressbar", { name: "Installing oh-my-pi" });
  expect(progress.getAttribute("value")).toBe("42");
  const row = screen.getByLabelText("oh-my-pi engine");
  expect(within(row).getByText("Installing 42%…")).toBeTruthy();
  expect(within(row).queryByRole("button", { name: "Install" })).toBeNull();
  fireEvent.click(within(row).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(engineRuntime).toHaveBeenCalledWith("cancel", "omp"));
});

it("uses icon menu actions for defaults, Codex flags and details", async () => {
  show(); await screen.findByText("Ready · v1.4.0");
  fireEvent.click(screen.getByRole("button", { name: "Actions for Codex" }));
  for (const item of screen.getAllByRole("menuitem")) expect(item.querySelector("svg")).toBeTruthy();
  fireEvent.click(screen.getByRole("menuitem", { name: "Set as default" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ agentPolicy: expect.objectContaining({ defaultEngine: "codex" }) })));
  fireEvent.click(screen.getByRole("button", { name: "Actions for Codex" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Codex flags" })); expect(flags).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "Actions for Claude Code (ACP)" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "View logs/details" }));
  expect(within(screen.getByRole("dialog")).getByText("Runtime failed to start")).toBeTruthy();
});

it("tests a custom engine from its menu and confirms deletion", async () => {
  show();
  fireEvent.click(screen.getByRole("button", { name: "Actions for Gemini CLI" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Test connection" }));
  expect(await screen.findByText("Protocol handshake passed · v1.2.3")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.getByRole("option", { name: "Gemini CLI" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Actions for Gemini CLI" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(within(screen.getByRole("dialog", { name: "Delete Gemini CLI?" })).getByRole("button", { name: "Delete engine" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ engines: expect.not.arrayContaining([custom]) })));
});

it("validates the modal inline, browses for an executable, tests and saves structured fields", async () => {
  show(); fireEvent.click(screen.getByRole("button", { name: "Add engine" }));
  const dialog = within(screen.getByRole("dialog", { name: "Add engine" }));
  expect((dialog.getByRole("button", { name: "Save engine" })).disabled).toBe(true);
  fireEvent.change(dialog.getByRole("textbox", { name: "Name" }), { target: { value: "Gemini" } });
  fireEvent.click(dialog.getByRole("button", { name: "Browse…" }));
  await waitFor(() => expect((dialog.getByRole("textbox", { name: "Command" })).value).toBe("/bin/gemini"));
  fireEvent.change(dialog.getByRole("textbox", { name: "Arguments" }), { target: { value: "--experimental-acp\n--quiet" } });
  fireEvent.click(dialog.getByRole("button", { name: "Add variable" }));
  fireEvent.change(dialog.getByRole("textbox", { name: "Environment key 1" }), { target: { value: "API_TOKEN" } });
  expect(dialog.getByRole("alert").textContent).toContain("use a provider instead");
  expect((dialog.getByRole("button", { name: "Test" })).disabled).toBe(true);
  fireEvent.change(dialog.getByRole("textbox", { name: "Environment key 1" }), { target: { value: "COLOR" } });
  fireEvent.change(dialog.getByRole("textbox", { name: "Environment value 1" }), { target: { value: "0" } });
  fireEvent.click(dialog.getByRole("button", { name: "Test" }));
  expect((await dialog.findByRole("status")).textContent).toContain("v1.2.3");
  expect(testEngine).toHaveBeenCalledWith(expect.objectContaining({ protocol: "acp", command: "/bin/gemini", args: ["--experimental-acp", "--quiet"], env: { COLOR: "0" } }));
  fireEvent.click(dialog.getByRole("button", { name: "Save engine" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(save).toHaveBeenCalled();
});

it("shows a failed handshake's stderr excerpt in the edit modal", async () => {
  vi.mocked(testEngine).mockRejectedValue(new Error("Engine exited before handshake\nStderr: startup failed"));
  show(); fireEvent.click(screen.getByRole("button", { name: "Actions for Gemini CLI" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Edit" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Test" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Stderr: startup failed");
});

it("opens existing Codex setup when its runtime is missing", async () => {
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "codex", state: "pending", message: "Install Codex" }]);
  show();
  fireEvent.click(within(screen.getByLabelText("Codex engine")).getByRole("button", { name: "Install" }));
  expect(setup).toHaveBeenCalledOnce();
});
