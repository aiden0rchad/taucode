import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const read = name => readFileSync(`src/styles/${name}.css`, "utf8");
const tokens = css => Object.fromEntries([...css.matchAll(/(--[\w-]+):\s*(#[\da-f]{6});/gi)].map(([, key, value]) => [key, value]));
const dark = tokens(read("themes.dark"));
function luminance(hex) {
  const channels = hex.slice(1).match(/../g).map(channel => parseInt(channel, 16) / 255);
  return channels.reduce((sum, value, index) => sum + [0.2126, 0.7152, 0.0722][index] * (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4), 0);
}

it.each(["dark", "light", "system", "dim"])("keeps tree lines visible against the %s sidebar", theme => {
  const colors = { ...dark, ...tokens(read(`themes.${theme}`)) };
  const background = luminance(colors["--surface-sidebar"]);
  for (const [token, minimum] of [["--tree-line", 1.8], ["--tree-line-active", 3]]) {
    const foreground = luminance(colors[token]);
    const ratio = (Math.max(background, foreground) + 0.05) / (Math.min(background, foreground) + 0.05);
    expect(ratio, `${theme} ${token}`).toBeGreaterThanOrEqual(minimum);
  }
});

it("uses dedicated colors for 1px trunks and rounded elbows, including the project connection", () => {
  const css = read("visual-refresh");
  const rules = Object.fromEntries([...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selector, declarations]) => [selector.replace(/\/\*[\s\S]*?\*\//g, "").trim(), declarations]));
  expect(rules[".tree-branch::before, .tree-branch::after"]).toContain("border-color: var(--tree-line)");
  expect(rules[".tree-path::after, .tree-branch:has(~ .tree-path)::before"]).toContain("border-color: var(--tree-line-active)");
  expect(rules[".tree-branch:has(~ .tree-path)::after"]).toContain("border-left-color: var(--tree-line-active)");
  expect(rules[".tree-branch::before"]).toContain("border-left-width: 1px");
  expect(rules[".tree-branch::after"]).toContain("border-left-width: 1px");
  expect(rules[".tree-branch::after"]).toContain("border-bottom-width: 1px");
  expect(rules[".tree-branch::after"]).toContain("border-bottom-left-radius: 6px");
  expect(rules[".workspace-card-content .sidebar-tree > .tree-branch:first-child::after"]).toContain("top: -18px");
  expect(rules[".tree-last-child::before"]).toContain("display: none");
});
