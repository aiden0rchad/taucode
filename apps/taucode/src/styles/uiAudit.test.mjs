// @vitest-environment jsdom
import { createElement as h, Fragment } from "react";
import { readdirSync, readFileSync } from "node:fs";
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SettingsNav, SETTINGS_GROUPS } from "@settings/components/SettingsNav";
import { SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { headerActionsOverflow } from "@app/components/headerLayout";
import { WorkConcurrency } from "@/features/work/WorkConcurrency";
const styles = readdirSync("src", { recursive: true }).filter(file => file.endsWith(".css")).map(file => [file, readFileSync(`src/${file}`, "utf8")]);
afterEach(cleanup);
it("has no literal font sizes below 11px, including rem equivalents", () => {
  expect(styles.length).toBeGreaterThan(50);
  const small = styles.flatMap(([file, css]) => [...css.matchAll(/font-size\s*:\s*([\d.]+)(px|rem)\b/g)].filter(match => Number(match[1]) * (match[2] === "rem" ? 16 : 1) < 11).map(match => `${file}: ${match[0]}`));
  expect(small).toEqual([]);
});
it.each([960, 1136, 1440])("moves overflowing header actions into the menu at %ipx", width => {
  const available = width - 280 - 320;
  const secondary = [100, 28, 28];
  const overflow = headerActionsOverflow(available, secondary);
  expect(overflow).toBe(width === 960);
  const occupied = 240 + (overflow ? 28 : secondary.reduce((a, b) => a + b) + 16);
  expect(occupied).toBeLessThanOrEqual(available);
  // Long branch/action labels must collapse even at the middle audit width.
  if (width === 1136) expect(headerActionsOverflow(available, [220, 100, 28, 28])).toBe(true);
});
it("orders settings and names switches from their row without overriding explicit names", () => {
  render(h(Fragment, null, h(SettingsNav, { activeSection: "providers", onSelectSection: () => {} }), h(SettingsToggleRow, { title: "Notifications" }, h(SettingsToggleSwitch, { pressed: false })), h(SettingsToggleRow, { title: "CodeGraph" }, h(SettingsToggleSwitch, { pressed: true, "aria-label": "Enable CodeGraph" }))));
  expect(SETTINGS_GROUPS.flatMap(group => group.sections)).toEqual(["appearance", "features", "engines", "codex-flags", "providers", "agents", "connectors", "composer", "display", "shortcuts", "dictation", "git", "open-apps", "projects", "environments", "browser", "computer", "server", "import", "about"]);
  expect(screen.getByRole("button", { name: "Models & Agents" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Notifications" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Enable CodeGraph" })).toBeTruthy();
});
it("keeps concurrency closed until requested and portals it outside the sidebar", () => {
  const change = vi.fn();
  const { container } = render(h("aside", null, h(WorkConcurrency, { value: 2, disabled: false, onChange: change })));
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Space options" }));
  const dialog = screen.getByRole("dialog", { name: "Space options" });
  expect(container.contains(dialog)).toBe(false);
  fireEvent.change(screen.getByRole("combobox", { name: "Concurrent tasks" }), { target: { value: "3" } });
  expect(change).toHaveBeenCalledWith(3);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("gives selected sidebar rows at least 1.25:1 contrast in both themes", () => {
  const luminance = (hex) => {
    const rgb = hex.slice(1).match(/../g).map(value => parseInt(value, 16) / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
    return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
  };
  for (const [sidebar, selected] of [["#f0f0f2", "#d0d4dd"], ["#1b1b1e", "#303034"]]) {
    const [a, b] = [luminance(sidebar), luminance(selected)].sort((x, y) => y - x);
    expect((a + .05) / (b + .05)).toBeGreaterThanOrEqual(1.25);
    expect(styles.some(([, css]) => css.includes(selected)), selected).toBe(true);
  }
});

it("uses distinct settings icons and shared switches throughout settings", () => {
  const { container } = render(h(SettingsNav, { activeSection: "providers", onSelectSection: () => {} }));
  const icons = [...container.querySelectorAll(".settings-nav svg")].map(icon => icon.getAttribute("class"));
  expect(icons).toHaveLength(20);
  expect(new Set(icons).size).toBe(20);
  expect(icons[0]).toContain("lucide-palette");
  expect(icons[4]).toContain("lucide-key-round");
  const files = readdirSync("src/features/settings", { recursive: true }).filter(file => file.endsWith(".tsx") && !file.includes(".test."));
  expect(files.filter(file => /type=["']checkbox["']/.test(readFileSync(`src/features/settings/${file}`, "utf8")))).toEqual([]);
});
it("keeps the compact composer controls and Changes header in flow", () => {
  const css = styles.map(([, css]) => css).join("\n");
  expect(css).toContain("container: composer / inline-size");
  const compact = css.slice(css.indexOf("@container composer (max-width: 420px)"));
  expect(compact).toContain(".composer-shell .reasoning-chip-label");
  expect(compact).toContain(".composer-shell .reasoning-chip-dot { display: inline-block;");
  expect(compact).toContain(".composer-access-chip:focus-within");
  expect(css).toMatch(/\.composer-shell-left\s*\{[^}]*flex-shrink: 0/);
  expect(css).toMatch(/\.changes-plan\s*\{[^}]*position: static/);
  expect(css).toMatch(/\.git-panel-header\s*\{[^}]*position: static/);
  expect(css).toContain("font-size: var(--minimum-font-size, 11px)");
});

it("places all desktop mode sidebars and conversations in one explicit grid row without the legacy 960px override", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  expect(base).not.toContain("@media (max-width: 960px)");
  const css = readFileSync("src/features/panels/side-panel.css", "utf8");
  expect(css).toContain(".workspace-main > :is(.app.layout-desktop, .chat-layout) { grid-template-columns: var(--layout-sidebar-column) minmax(0, 1fr); grid-template-rows: minmax(0, 1fr);");
  expect(css).toMatch(/> \.sidebar\s*\{ grid-column: 1; grid-row: 1;/);
  expect(css).toMatch(/> :is\(\.main, \.chat-main\)\s*\{ grid-column: 2; grid-row: 1;/);
  expect(css).toMatch(/\.workspace-shell.sidebar-overlay[^}]*> \.sidebar\s*\{ position: absolute;/);
});
