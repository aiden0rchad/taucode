import { readFileSync } from "node:fs";
import postcss from "postcss";
import { expect, it } from "vitest";

it("disables reasoning animations and particles for reduced motion", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  const reduced = css.nodes.find(node => node.type === "atrule" && node.params === "(prefers-reduced-motion: reduce)");
  expect(reduced).toBeTruthy();
  const rules = Object.fromEntries(reduced.nodes.map(rule => [rule.selector, rule.nodes]));
  expect(rules[".reasoning-popover, .reasoning-popover *, .reasoning-popover *::before, .reasoning-popover *::after"]).toEqual(expect.arrayContaining([expect.objectContaining({ prop: "animation", value: "none", important: true })]));
  expect(rules[".reasoning-ember"]).toEqual(expect.arrayContaining([expect.objectContaining({ prop: "display", value: "none" })]));
  css.walkDecls("animation", declaration => {
    if (declaration.value.includes("infinite")) {
      expect(declaration.parent.selector).toMatch(/reasoning-popover\.burning|reasoning-ember/);
    }
  });
});
