import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
it("keeps browser controls centered with the shared 28px token", () => {
  const css = readFileSync("src/features/browser/browser.css", "utf8");
  expect(css).toMatch(/\.browser-toolbar \{ height: 36px/);
  expect(css).toContain("var(--icon-button-28, 28px)");
  expect(css).toMatch(/\.browser-address \{[^}]*height: 30px/);
  expect(css).toMatch(/\.browser-toolbar svg \{ width: 16px; height: 16px/);
});
