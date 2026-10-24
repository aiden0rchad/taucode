// @vitest-environment node
import { describe, expect, it } from "vitest";

// @ts-expect-error Frontend tsconfig intentionally excludes Node types.
import { readFileSync } from "node:fs";
const css: string = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");
function luminance(hex: string) {
  const channels = hex.slice(1).match(/../g)!.map(value => {
    const channel = parseInt(value, 16) / 255;
    return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4;
  });
  return channels[0] * .2126 + channels[1] * .7152 + channels[2] * .0722;
}
function contrast(a: string, b: string) {
  const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (values[0] + .05) / (values[1] + .05);
}
describe("opaque accessible themes", () => {
  for (const [theme, selector] of [["dark", ":root"], ["dim", ':root[data-theme="dim"]'], ["light", ':root[data-theme="light"]']] as const) {
    it(`${theme} text, status, accent and input boundaries meet AA`, () => {
      const body = css.slice(css.indexOf(selector)).split("{")[1].split("}")[0];
      const tokens = Object.fromEntries([...body.matchAll(/--([\w-]+):\s*(#[\da-f]{6});/g)].map(m => [m[1], m[2]]));
      for (const surface of ["bg-app", "bg-sidebar", "bg-surface", "bg-surface-raised", "bg-input"]) {
        for (const text of ["text", "text-muted", "text-faint", "accent", "danger", "warning", "success"]) {
          expect(contrast(tokens[text], tokens[surface]), `${theme}: ${text} on ${surface}`).toBeGreaterThanOrEqual(4.5);
        }
        expect(contrast(tokens["border-strong"], tokens[surface]), `${theme}: input boundary on ${surface}`).toBeGreaterThanOrEqual(3);
      }
      expect(contrast(tokens["accent-text"], tokens.accent)).toBeGreaterThanOrEqual(4.5);
    });
  }
  it("has no glass effects or translucent surface declarations", () => {
    const sheets = import.meta.glob("../**/*.css");
    for (const path of Object.keys(sheets)) {
      const source: string = readFileSync(new URL(path, import.meta.url), "utf8");
      expect(source.includes(["backdrop", "filter"].join("-")), path).toBe(false);
      expect(/background(?:-color)?:\s*rgba\(/.test(source), path).toBe(false);
    }
  });
});
