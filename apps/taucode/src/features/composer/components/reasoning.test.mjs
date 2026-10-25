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
      expect(declaration.parent.selector).toMatch(/reasoning-popover\.burning|reasoning-ember|reasoning-particle|reasoning-fill::before/);
    }
  });
});

it("reserves stable popover, heading and chip geometry with animated snapping", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  const rule = selector => Object.fromEntries(css.nodes.find(node => node.selector === selector).nodes.map(node => [node.prop, node.value]));
  expect(rule(".reasoning-popover")).toMatchObject({ width: "300px", padding: "12px 14px", "--popover-max-height": "360px", "overflow-x": "hidden" });
  expect(rule(".reasoning-popover").height).toBeUndefined();
  expect(rule(".reasoning-popover")["min-height"]).toBeUndefined();
  expect(rule(".reasoning-heading")).toMatchObject({ height: "40px", "align-items": "center" });
  expect(rule(".reasoning-slider").margin).toBe("15px 0 3px");
  expect(rule(".reasoning-notice:empty").display).toBe("none");
  expect(rule(".composer-model-options")["overflow-y"]).toBe("auto");
  expect(rule(".reasoning-chip-label").display).toBe("grid");
  expect(rule(".reasoning-chip-label > span")["grid-area"]).toBe("1 / 1");
  expect(rule(".reasoning-chip-sizer").visibility).toBe("hidden");
  expect(rule(".reasoning-level").width).toBe("100px");
  expect(rule(".reasoning-fill").transition).toBe("width 200ms ease-out");
  expect(rule(".reasoning-slider").height).toBe("24px");
  expect(rule(".reasoning-fill")).toMatchObject({ height: "24px", overflow: "hidden" });
  expect(rule(".reasoning-thumb")).toMatchObject({ height: "30px", top: "-3px" });
  expect(rule(".reasoning-popover input")).toMatchObject({ height: "30px", top: "-3px" });
  expect(rule(".reasoning-tick")).toMatchObject({ height: "4px", top: "10px" });
  expect(rule(".reasoning-slider:has(input:focus-visible)").outline).toBe("2px solid var(--reasoning-color)");
  expect(rule(".reasoning-popover.dragging .reasoning-fill, .reasoning-popover.dragging .reasoning-thumb").transition).toBe("none");
});

it("flows two identical gradient tiles using only translation, with matching endpoints", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  const flow = css.nodes.find(node => node.type === "atrule" && node.params === "reasoning-flow");
  const declarations = [];
  flow.walkDecls(declaration => declarations.push([declaration.prop, declaration.value]));
  expect(declarations).toEqual([["transform", "translateX(0)"], ["transform", "translateX(-50%)"]]);
  const layer = css.nodes.find(node => node.selector === ".reasoning-fill::before");
  const values = Object.fromEntries(layer.nodes.map(node => [node.prop, node.value]));
  expect(values.width).toBe("200%");
  expect(values.animation).toBe("reasoning-flow 7s linear infinite");
  const stops = postcss.list.comma(values.background.slice("linear-gradient(".length, -1)).slice(1);
  const colors = stops.map(stop => stop.match(/^var\([^)]+\)/)[0]);
  expect(colors[0]).toBe(colors[colors.length - 1]);
  expect(colors.slice(0, 4)).toEqual(colors.slice(3));
  expect(stops).toEqual(["var(--flow-a) 0%", "var(--flow-b) calc(100% / 6)", "var(--flow-c) calc(100% / 3)", "var(--flow-a) 50%", "var(--flow-b) calc(200% / 3)", "var(--flow-c) calc(500% / 6)", "var(--flow-a) 100%"]);
  const drift = css.nodes.find(node => node.type === "atrule" && node.params === "reasoning-drift");
  drift.walkDecls(declaration => expect(["transform", "opacity"]).toContain(declaration.prop));
});

it("keeps particle base durations fixed in CSS between 4.5 and 5.5 seconds", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  const particle = css.nodes.find(node => node.selector === ".reasoning-particle");
  expect(particle.nodes.find(node => node.prop === "animation").value).toBe("reasoning-drift 5.5s linear infinite");
  const durations = [];
  css.walkRules(/^\.reasoning-particle:nth-child/, rule => rule.walkDecls("animation-duration", declaration => durations.push(declaration.value)));
  expect(durations).toEqual(["5.25s", "5s", "4.75s", "4.5s"]);
});

it("pushes views in opposite directions using transform and opacity only", () => {
  const css = postcss.parse(readFileSync("src/features/composer/components/reasoning.css", "utf8"));
  for (const direction of ["in", "out"]) {
    const frames = css.nodes.find(node => node.type === "atrule" && node.params === "reasoning-view-" + direction);
    frames.walkDecls(declaration => expect(["transform", "opacity"]).toContain(declaration.prop));
  }
  const offset = selector => css.nodes.find(node => node.selector === selector).nodes.find(node => node.prop === "--view-offset").value;
  expect(offset(".reasoning-view.forward")).toBe("24px");
  expect(offset(".reasoning-view.back")).toBe("-24px");
});
