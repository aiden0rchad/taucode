import { readdirSync, readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("uses one named layer scale for every CSS z-index", () => {
  const tokens = readFileSync("src/styles/ds-tokens.css", "utf8");
  const layers = Object.fromEntries([...tokens.matchAll(/(--z-[\w-]+):\s*(-?\d+);/g)].map(([, name, value]) => [name, Number(value)]));
  const order = ["base", "sticky", "panel", "sheet", "popover", "modal-scrim", "modal", "toast", "tooltip"].map(name => layers[`--z-${name}`]);
  for (let index = 1; index < order.length; index++) expect(order[index]).toBeGreaterThan(order[index - 1]);
  for (const path of readdirSync("src", { recursive: true }).filter(path => String(path).endsWith(".css"))) {
    const css = readFileSync(`src/${path}`, "utf8");
    for (const [, value] of css.matchAll(/(?<![\w-])z-index:\s*([^;}]+)/g)) {
      expect(value, String(path)).toMatch(/^var\(--z-[\w-]+\)$/);
      expect(layers, String(path)).toHaveProperty(value.slice(4, -1));
    }
    if (path !== "styles/ds-tokens.css") expect(css, String(path)).not.toMatch(/--z-[\w-]+:\s*-?\d/);
  }
});
