// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createElement, Fragment } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
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
  await invoke("update_app_settings", { settings: { features: {} } });
  render(createElement(Fragment, null, createElement("style", null, settingsStyles), createElement(App)));
  const dialog = await screen.findByRole("dialog", { name: "Settings" }, { timeout: 5000 });
  for (const section of SETTINGS_GROUPS.flatMap(group => group.sections)) {
    fireEvent.click(within(dialog).getByRole("button", { name: SETTINGS_SECTION_LABELS[section] }));
    await waitFor(() => expect(dialog.querySelector(".settings-page"), section).toBeTruthy());
    expect(dialog.querySelector(".settings-page .settings-group"), section).toBeTruthy();
    expect(dialog.querySelector(".settings-page .settings-row"), section).toBeTruthy();
    expect(dialog.querySelector('fieldset, input[type="checkbox"], input[type="radio"]'), section).toBeNull();
    expect(within(dialog).queryByText("Engine runtimes"), section).toBeNull();
    for (const details of dialog.querySelectorAll(".settings-project-row")) details.open = true;
    for (const trigger of dialog.querySelectorAll(".settings-select-label")) {
      expect(getComputedStyle(trigger).whiteSpace, section).toBe("nowrap");
      expect(getComputedStyle(trigger).textOverflow, section).toBe("ellipsis");
      expect(trigger.closest(".settings-select-control").querySelector("select").title, section).toBeTruthy();
    }
    for (const row of dialog.querySelectorAll(".settings-control-row > .settings-field-row:not(.settings-field-row-long)")) {
      expect(getComputedStyle(row).display, section).toBe("grid");
      expect(getComputedStyle(row.parentElement).display, section).toBe("block");
    }
  }
  for (const selector of [".settings-master", ".settings-nav-list", ".settings-content"]) {
    const styles = getComputedStyle(dialog.querySelector(selector));
    expect(styles.overflowY, selector).toBe("auto");
    expect(styles.minHeight, selector).toBe("0");
  }
  fireEvent.click(within(dialog).getByRole("button", { name: "Computer Use" }));
  const value = "Background";
  expect(within(dialog).getByText(value, { selector: ".settings-select-label" })).toBeTruthy();
  expect(within(dialog).getByText("GPT-5 mini · Extended reasoning", { selector: ".settings-select-label" })).toBeTruthy();
  const typing = within(dialog).getByRole("combobox", { name: "Typing confirmation" }).closest(".settings-control-row");
  expect(typing.querySelector(".settings-field-row .settings-control-help")).toBeNull();
  expect(typing.querySelector(":scope > .settings-control-help").textContent).toContain("Sensitive mode allows typing");
  for (const trigger of dialog.querySelectorAll(".settings-select-label")) {
    expect(getComputedStyle(trigger).whiteSpace).toBe("nowrap");
    expect(getComputedStyle(trigger).textOverflow).toBe("ellipsis");
    expect(trigger.closest(".settings-select-control").querySelector("select").title).toBeTruthy();
  }
  const mode = within(dialog).getByRole("combobox", { name: "Input mode" });
  fireEvent.mouseDown(mode);
  const options = screen.getByRole("listbox", { name: "Input mode" });
  expect(within(options).getByText("Recommended")).toBeTruthy();
  expect(within(options).getByText("You can keep using your Mac.")).toBeTruthy();
  fireEvent.keyDown(mode, { key: "Escape" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Manage" }));
  expect(within(dialog).getByRole("heading", { name: "Engines" })).toBeTruthy();
  expect(await within(dialog).findByText("Installed and available engines")).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Models & Agents" }));
  const tabs = within(dialog).getByRole("tablist", { name: "Models & Agents tabs" });
  expect(within(tabs).getAllByRole("tab").map(button => button.textContent)).toEqual(["Defaults", "Subagents", "Roles", "Engines"]);
  for (const name of ["Defaults", "Subagents", "Roles"]) {
    fireEvent.click(within(tabs).getByRole("tab", { name }));
    expect(within(dialog).queryByText("Engine runtimes")).toBeNull();
    for (const trigger of dialog.querySelectorAll(".settings-select-label")) expect(getComputedStyle(trigger).whiteSpace, name).toBe("nowrap");
  }
}, 20000);


it("uses roomy dialog and row tokens with an opaque dialog and a fading translucent scrim", () => {
  expect(settingsStyles).toContain("width: min(1180px, 94vw)");
  expect(settingsStyles).toContain("height: min(900px, 92vh)");
  expect(settingsStyles).toContain("minmax(0, 1fr) clamp(280px, 38%, 420px)");
  expect(settingsStyles).toContain(".settings-window { background: var(--surface-app); }");
  expect(settingsStyles).toContain("background: rgba(0, 0, 0, 0.5)");
  expect(settingsStyles).toContain(':root[data-theme="light"] .settings-overlay > .ds-modal-backdrop { background: rgba(15, 15, 20, 0.28); }');
  expect(settingsStyles).toContain("animation-duration: 150ms");
  expect(settingsStyles).toMatch(/prefers-reduced-motion: reduce[^}]+settings-overlay > \.ds-modal-backdrop \{ animation: none/s);
  expect(settingsStyles).not.toContain("backdrop-filter");
  expect(settingsStyles).toContain(".settings-select-label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }");
});

it.each([[1440, "dark", "rgba(0, 0, 0, 0.5)"], [900, "light", "rgba(15, 15, 20, 0.28)"]])("layers Settings over the open side-chat panel at %spx in %s theme", async (width, theme, tint) => {
  vi.stubGlobal("innerWidth", width);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=code&thread=thread-rich&sidechat=thread-side&settings=computer&theme=" + theme);
  installUiPreview();
  await invoke("update_app_settings", { settings: { features: {} } });
  const styles = ["src/styles/ds-tokens.css", "src/styles/ds-modal.css", "src/features/panels/side-panel.css"].map(path => readFileSync(path, "utf8")).join("\n") + settingsStyles;
  render(createElement(Fragment, null, createElement("style", null, styles), createElement(App)));
  await screen.findByRole("textbox", { name: "Side chat message" });
  const dialog = await screen.findByRole("dialog", { name: "Settings" }, { timeout: 5000 });
  document.documentElement.dataset.theme = theme;
  const panel = document.querySelector(".tabbed-side-panel");
  expect(panel.hidden).toBe(false);
  expect(dialog.parentElement).toBe(document.body);
  const layer = element => {
    const value = getComputedStyle(element).zIndex;
    expect(value).toMatch(/^var\(--z-/);
    return Number(getComputedStyle(document.documentElement).getPropertyValue(value.slice(4, -1)));
  };
  const scrim = dialog.querySelector(".ds-modal-backdrop");
  expect(layer(dialog)).toBeGreaterThan(layer(panel));
  expect(layer(scrim)).toBeGreaterThan(layer(panel));
  expect(layer(dialog.querySelector(".ds-modal-card"))).toBeGreaterThan(layer(scrim));
  expect(getComputedStyle(scrim).backgroundColor).toBe(tint);
  expect(getComputedStyle(scrim).position).toBe("absolute");
  fireEvent.click(within(dialog).getByRole("button", { name: "Computer Use" }));
  fireEvent.mouseDown(within(dialog).getByRole("combobox", { name: "Input mode" }));
  const options = screen.getByRole("listbox", { name: "Input mode" });
  expect(options.parentElement).toBe(document.body);
  expect(layer(options)).toBeGreaterThan(layer(dialog));
}, 20000);
