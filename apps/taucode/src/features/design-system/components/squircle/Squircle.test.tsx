// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Squircle } from "./Squircle";
import { chamferPath, roundedRectPath, squirclePath } from './squirclePath';
import type { CSSProperties } from 'react';

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it.each([false, true])("selects native corner support: %s", native => {
  const supports = vi.fn(() => native);
  vi.stubGlobal("CSS", { supports });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ width: 260, height: 120 } as DOMRect);
  const { container } = render(<Squircle><button>Child</button></Squircle>);
  expect(supports).toHaveBeenCalledWith("corner-shape", "squircle");
  expect(container.firstElementChild?.classList.contains(native ? "squircle-native" : "squircle-svg")).toBe(true);
  expect(container.querySelector(".squircle-border") !== null).toBe(!native);
  const background = container.querySelector<HTMLElement>(".squircle-background")!;
  expect(background.style.clipPath.startsWith("path(")).toBe(!native);
  if (!native) expect(container.querySelector("path")?.getAttribute("d")).toContain("259.5");
});

it.each(['squircle','round','chamfer'])('uses the %s path and updates on a theme event', kind => {
  vi.stubGlobal('CSS', { supports: () => false });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: 100, height: 60 } as DOMRect);
  const { container } = render(<Squircle style={{'--card-corners':kind} as CSSProperties} />);
  const expected = kind === 'round' ? roundedRectPath(100,60,20,.5) : kind === 'chamfer' ? chamferPath(100,60,10,2,.5) : squirclePath(100,60,20,.6,.5);
  expect(container.querySelector('.squircle-stroke')?.getAttribute('d')).toBe(expected);
  const element = container.firstElementChild as HTMLElement;
  element.style.setProperty('--card-corners','chamfer');
  element.style.setProperty('--card-chamfer','18px');
  act(() => window.dispatchEvent(new Event('taucode-theme-applied')));
  expect(container.querySelector('.squircle-stroke')?.getAttribute('d')).toBe(chamferPath(100,60,18,2,.5));
});

it('keeps gradients and glow IDs local, even with native corner support', () => {
  vi.stubGlobal('CSS', { supports: () => true });
  const style = {'--card-border-stops':'3','--card-border-angle':'135','--card-neon-glow':'1'} as CSSProperties;
  const { container, rerender } = render(<><Squircle style={style} /><Squircle style={style} /></>);
  const gradients = [...container.querySelectorAll('linearGradient')];
  expect(new Set(gradients.map(g => g.id)).size).toBe(2);
  for (const svg of container.querySelectorAll('.squircle-border')) {
    expect(svg.querySelectorAll('stop')).toHaveLength(3);
    expect(svg.querySelector('stop')?.getAttribute('style')).toContain('var(--card-border-1)');
    expect(svg.querySelector('.squircle-stroke')?.getAttribute('stroke')).toBe(`url(#${svg.querySelector('linearGradient')!.id})`);
    expect(svg.querySelector('feGaussianBlur')?.getAttribute('stdDeviation')).toBe('3');
  }
  rerender(<Squircle style={{...style,'--card-neon-glow':'0'} as CSSProperties} />);
  expect(container.querySelector('.squircle-glow')).toBeNull();
  expect(container.querySelector('feGaussianBlur')).toBeNull();
  rerender(<Squircle style={{'--card-corners':'chamfer'} as CSSProperties} />);
  expect(container.querySelector('.squircle-svg')).toBeTruthy();
  expect(container.querySelector('linearGradient')).toBeNull();
  expect(container.querySelector('.squircle-stroke')?.hasAttribute('stroke')).toBe(false);
  rerender(<Squircle />);
  expect(container.querySelector('.squircle-native')).toBeTruthy();
});

it('clips content separately so the gradient glow can escape', () => {
  const { container } = render(<Squircle className="squircle-clip" style={{'--card-border-stops':'2','--card-border-highlight':'1'} as CSSProperties}><button>Child</button></Squircle>);
  expect(container.querySelector('.squircle-content > button')).toBeTruthy();
  expect(container.querySelector('.squircle-content .squircle-border')).toBeNull();
  expect(container.querySelector('.squircle-highlight')).toBeTruthy();
});

it('preserves focused content when a native card switches to SVG', () => {
  vi.stubGlobal('CSS', { supports: () => true });
  const { container, rerender } = render(<Squircle className="squircle-clip"><input aria-label="Draft" defaultValue="Unsaved" /></Squircle>);
  const input = container.querySelector('input')!;
  input.focus();
  rerender(<Squircle className="squircle-clip" style={{'--card-corners':'chamfer'} as CSSProperties}><input aria-label="Draft" defaultValue="Unsaved" /></Squircle>);
  expect(container.querySelector('input')).toBe(input);
  expect(document.activeElement).toBe(input);
});

it("batches resize notifications, ignores unchanged sizes and cleans up", () => {
  vi.stubGlobal("CSS", { supports: () => false });
  let resize = () => {}, frame = () => {}, width = 260;
  const disconnect = vi.fn();
  vi.stubGlobal("ResizeObserver", class { constructor(callback: () => void) { resize = callback; } observe() {} disconnect = disconnect; });
  const raf = vi.fn((callback: () => void) => { frame = callback; return 1; });
  const cancel = vi.fn();
  vi.stubGlobal("requestAnimationFrame", raf);
  vi.stubGlobal("cancelAnimationFrame", cancel);
  const measure = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({ width, height: 120 }) as DOMRect);
  const styles = vi.spyOn(window, 'getComputedStyle');
  const { container, unmount } = render(<Squircle />);
  const original = container.querySelector("path")?.getAttribute("d");
  act(() => { resize(); resize(); });
  expect(raf).toHaveBeenCalledTimes(1);
  act(() => frame());
  expect(measure).toHaveBeenCalledTimes(2);
  expect(container.querySelector("path")?.getAttribute("d")).toBe(original);
  width = 300;
  act(() => { resize(); frame(); });
  expect(container.querySelector("path")?.getAttribute("d")).not.toBe(original);
  expect(styles).toHaveBeenCalledTimes(1);
  resize(); unmount();
  expect(disconnect).toHaveBeenCalledOnce();
  expect(cancel).toHaveBeenCalledWith(1);
});
