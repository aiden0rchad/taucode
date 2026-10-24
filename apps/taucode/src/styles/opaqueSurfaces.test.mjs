import { expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";

const styles = Object.fromEntries(readdirSync("src", { recursive: true }).filter(path => String(path).endsWith(".css")).map(path => [`/src/${path}`, readFileSync(`src/${path}`, "utf8")]));

it("keeps all CSS free of backdrop filters", () => {
  for (const [path, css] of Object.entries(styles)) expect(css, path).not.toContain("backdrop-filter");
});

it("preserves the font stacks and opaque theme surfaces", () => {
  const dark = String(styles["/src/styles/themes.dark.css"]);
  expect(dark).toContain("--ui-font-family: system-ui");
  expect(dark).toContain("--code-font-family: ui-monospace");
  expect(dark).toContain("--surface-card: #1b1b1b");
  for (const theme of ["dark", "light", "dim", "system"]) {
    const css = String(styles[`/src/styles/themes.${theme}.css`]);
    expect(css).toContain("--surface-app:");
    expect(css).not.toMatch(/--surface-[\w-]+:\s*(?:rgba|hsla)\(/);
  }
});
