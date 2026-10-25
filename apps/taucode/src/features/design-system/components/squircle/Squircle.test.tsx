// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Squircle } from "./Squircle";

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
  resize(); unmount();
  expect(disconnect).toHaveBeenCalledOnce();
  expect(cancel).toHaveBeenCalledWith(1);
});
