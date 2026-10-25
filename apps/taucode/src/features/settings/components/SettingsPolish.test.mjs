// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createElement, Fragment } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import App from "@/App";
import { installUiPreview } from "@/dev/uiPreview";
import { SETTINGS_GROUPS } from "./SettingsNav";
import { SETTINGS_SECTION_LABELS } from "./settingsViewConstants";

const settingsStyles = readFileSync("src/styles/settings.css", "utf8");
vi.mock("@pierre/diffs/react", async () => ({
  ...await vi.importActual("@pierre/diffs/react"),
  WorkerPoolContextProvider: ({ children }) => children,
}));
afterEach(() => { cleanup(); clearMocks(); vi.unstubAllGlobals(); vi.unstubAllEnvs(); localStorage.clear(); });
it("uses shared settings pages, groups and rows in every section, with independent scrolling and engine navigation", async () => {
  localStorage.clear();
  vi.stubGlobal("innerWidth", 1440);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?settings=providers");
  installUiPreview();
  render(createElement(Fragment, null, createElement("style", null, settingsStyles), createElement(App)));
  const dialog = await screen.findByRole("dialog", { name: "Settings" });
  for (const section of SETTINGS_GROUPS.flatMap(group => group.sections)) {
    fireEvent.click(within(dialog).getByRole("button", { name: SETTINGS_SECTION_LABELS[section] }));
    await waitFor(() => expect(dialog.querySelector(".settings-page"), section).toBeTruthy());
    expect(dialog.querySelector(".settings-page .settings-group"), section).toBeTruthy();
    expect(dialog.querySelector(".settings-page .settings-row"), section).toBeTruthy();
    expect(dialog.querySelector('fieldset, input[type="checkbox"], input[type="radio"]'), section).toBeNull();
    expect(within(dialog).queryByText("Engine runtimes"), section).toBeNull();
  }
  for (const selector of [".settings-master", ".settings-nav-list", ".settings-content"]) {
    const styles = getComputedStyle(dialog.querySelector(selector));
    expect(styles.overflowY, selector).toBe("auto");
    expect(styles.minHeight, selector).toBe("0");
  }
  fireEvent.click(within(dialog).getByRole("button", { name: "Computer Use" }));
  const value = "Background (recommended): you can keep using your Mac";
  expect(within(dialog).getByText(value, { selector: ".settings-select-value" })).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Manage" }));
  expect(within(dialog).getByRole("heading", { name: "Models & Agents" })).toBeTruthy();
  expect(await within(dialog).findByText("Engine runtimes")).toBeTruthy();
  const tabs = within(dialog).getByRole("tablist", { name: "Models & Agents tabs" });
  expect(within(tabs).getAllByRole("tab").map(button => button.textContent)).toEqual(["Defaults", "Subagents", "Roles", "Engines"]);
  for (const name of ["Defaults", "Subagents", "Roles"]) {
    fireEvent.click(within(tabs).getByRole("tab", { name }));
    expect(within(dialog).queryByText("Engine runtimes")).toBeNull();
  }
}, 20000);

