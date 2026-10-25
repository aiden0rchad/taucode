import { describe, expect, it } from "vitest";
import { chamferPath, roundedRectPath, squirclePath } from "./squirclePath";

describe("squirclePath", () => {
  it("smooths a 260×120 window with four rotationally symmetric corners", () => {
    const path = squirclePath(260, 120);
    expect(path).toMatchSnapshot();
    expect(path).not.toBe(squirclePath(260, 120, 20, 0));
    const corners = [...path.matchAll(/C ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+)/g)].map(match => match.slice(1).map(Number));
    expect(corners).toHaveLength(8);
    for (let point = 0; point < 6; point += 2) {
      expect(corners[0][point] + corners[4][point]).toBeCloseTo(260, 3);
      expect(corners[0][point + 1] + corners[4][point + 1]).toBeCloseTo(120, 3);
      expect(corners[2][point] + corners[6][point]).toBeCloseTo(260, 3);
      expect(corners[2][point + 1] + corners[6][point + 1]).toBeCloseTo(120, 3);
    }
    expect(path).toContain("260 32");
    expect(path).toContain("0 88");
  });

  it("clamps radius and keeps degenerate boxes finite", () => {
    expect(squirclePath(12, 8, 200)).toBe(squirclePath(12, 8, 4));
    for (const [width, height] of [[0, 0], [0, 10], [1, 1], [0.1, 0.2], [-1, -2]]) {
      for (const inset of [0, 0.5]) expect(squirclePath(width, height, 20, 0.6, inset)).not.toMatch(/NaN|Infinity/);
    }
    expect(squirclePath(NaN, 10)).toBe("");
  });

  it.each([1, 2])("places the 1px stroke inside the box at %sx", scale => {
    const path = squirclePath(260, 120, 20, 0.6, 0.5);
    expect(path).toContain("259.5");
    expect(path).toContain("119.5");
    // Straight edges land on physical pixels at both display densities.
    const points = [...path.matchAll(/C ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+)/g)].flatMap(match => match.slice(1).map(Number));
    const xs = points.filter((_, index) => index % 2 === 0), ys = points.filter((_, index) => index % 2 === 1);
    expect((Math.min(...xs) - 0.5) * scale).toBe(0);
    expect((Math.max(...xs) + 0.5) * scale).toBe(260 * scale);
    expect((Math.min(...ys) - 0.5) * scale).toBe(0);
    expect((Math.max(...ys) + 0.5) * scale).toBe(120 * scale);
  });
});

describe('chamferPath', () => {
  it('cuts only TL/BR with six vertices and rounds TR/BL', () => {
    expect(chamferPath(100,60,10)).toBe('M 10 0 L 98 0 Q 100 0 100 2 L 100 50 L 90 60 L 2 60 Q 0 60 0 58 L 0 10 Z');
    expect(chamferPath(100,60,10).match(/L /g)).toHaveLength(5);
    expect(chamferPath(100,60,10).match(/Q /g)).toHaveLength(2);
    expect(roundedRectPath(100,60,10)).toBe(squirclePath(100,60,10,0));
  });
  it('clamps tiny boxes and rejects non-finite input', () => {
    expect(chamferPath(12,6,20)).toBe(chamferPath(12,6,2));
    for (const [w,h] of [[0,0],[1,1],[.1,.2],[-1,-2]]) {
      for (const inset of [0,.5]) expect(chamferPath(w,h,20,2,inset)).not.toMatch(/NaN|Infinity/);
    }
    expect(chamferPath(NaN,10)).toBe('');
    expect(chamferPath(10,10,Infinity)).toBe('');
  });
  it('insets both diagonals symmetrically by half a pixel', () => {
    const path = chamferPath(100,60,10,2,.5);
    expect(path).toContain('M 10.207 0.5');
    expect(path).toContain('L 99.5 49.793 L 89.793 59.5');
    expect(path).toContain('L 0.5 10.207');
    expect((10.207+.5-10)/Math.SQRT2).toBeCloseTo(.5,3);
  });
});
