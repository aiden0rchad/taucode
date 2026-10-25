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
      expect(declaration.parent.selector).toMatch(/reasoning-popover\.burning|reasoning-ember|reasoning-sparkle/);
    }
  });
});

it("reserves stable popover, heading and chip geometry with animated snapping", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  const rule = selector => Object.fromEntries(css.nodes.find(node => node.selector === selector).nodes.map(node => [node.prop, node.value]));
  expect(rule(".reasoning-popover")).toMatchObject({ width: "320px", height: "176px", "overflow-x": "hidden" });
  expect(rule(".reasoning-chip-label").display).toBe("grid");
  expect(rule(".reasoning-chip-label > span")["grid-area"]).toBe("1 / 1");
  expect(rule(".reasoning-chip-sizer").visibility).toBe("hidden");
  expect(rule(".reasoning-level").width).toBe("100px");
  expect(rule(".reasoning-fill").transition).toBe("width 200ms ease-out");
  expect(rule(".reasoning-popover.dragging .reasoning-fill, .reasoning-popover.dragging .reasoning-thumb").transition).toBe("none");
});
