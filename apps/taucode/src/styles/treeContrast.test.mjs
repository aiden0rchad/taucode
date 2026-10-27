// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { builtins } from '@/features/themes/builtins';

const read = name => readFileSync(`src/styles/${name}.css`, "utf8");
const tokens = css => Object.fromEntries([...css.matchAll(/(--[\w-]+):\s*(#[\da-f]{6});/gi)].map(([, key, value]) => [key, value]));
const dark = tokens(read("themes.dark"));
it("defines the window radius and shared group card treatments", () => {
  const css = read("visual-refresh");
  expect(css).toContain("--radius-window: 12px;");
  expect(css).toMatch(/\.sidebar \.project-group-card \{[^}]*--radius-window: 8px;/);
  expect(css).toContain("--squircle-border: color-mix(in srgb, var(--tree-line) 60%, transparent);");
  expect(css).toContain(".sidebar .project-group-card.is-selected { --squircle-border: var(--tree-line-active); }");
  expect(css).toContain(".sidebar .workspace-group-content { gap: 6px; }");
});
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

it("uses rounded SVG guides and keeps selected highlights off the branches", () => {
  const css = read("visual-refresh");
  expect(css).toContain('.tree-guide-normal { stroke: var(--tree-line); }');
  expect(css).toContain('.tree-guide-active { stroke: var(--tree-line-active); }');
  expect(css).toContain('filter: drop-shadow(0 0 4px var(--tree-line-active))');
  expect(css).toContain('--tree-row-highlight: 9px');
  expect(css).toContain('--tree-row-highlight: 6px');
  expect(css).not.toContain('.tree-branch::before');
  const themes = readFileSync('src/features/themes/themes.css','utf8');
  expect(themes).not.toMatch(/inset 2px 0/);
  for (const [,selector,body] of (css+themes+read('sidebar')).matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (selector.includes('thread-row') && selector.includes('active')) expect(body).not.toMatch(/border-left/);
  }
});

it.each(builtins)('keeps the $name selected row background out of the guide area', theme => {
  const style = document.createElement('style');
  style.textContent = read('visual-refresh') + readFileSync('src/features/themes/themes.css','utf8');
  const root = document.documentElement;
  const previous = {...root.dataset};
  Object.assign(root.dataset,{theme:theme.appearance,themeId:theme.id,translucent:String(!!theme.window?.translucent),neonPalette:String(!!theme.effects?.neonGlow)});
  const container = document.createElement('div');
  container.innerHTML = '<aside class="sidebar"><div class="sidebar-tree-node"><button class="thread-row active">Selected</button></div></aside>';
  document.head.append(style); document.body.append(container);
  try { expect(getComputedStyle(container.querySelector('button')).backgroundColor).toBe('rgba(0, 0, 0, 0)'); }
  finally { style.remove(); container.remove(); for (const key of Object.keys(root.dataset)) delete root.dataset[key]; Object.assign(root.dataset,previous); }
});

it("lets collapsible sidebar grids shrink with the sidebar", () => {
  const css = read("visual-refresh");
  expect(css).toMatch(/\.sidebar \.workspace-card-content,[^{]*\{ grid-template-columns: minmax\(0, 1fr\); \}/);
  expect(css).toMatch(/\.sidebar \.workspace-card-content-inner,[^{]*\.sidebar \.thread-list \{ min-width: 0; \}/);
});
