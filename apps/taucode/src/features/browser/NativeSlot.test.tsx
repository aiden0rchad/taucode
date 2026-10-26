// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NativeSlot, roundedRect } from "./NativeSlot";
import { browserNativeBounds } from "@services/tauri";
import { intersects, registerNativeOverlay } from "@/features/design-system/nativeOverlays";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
vi.mock("@services/tauri", () => ({ browserNativeBounds: vi.fn().mockResolvedValue(undefined) }));
let callbacks: Map<number, FrameRequestCallback>;
let next: number;
const rect = { x: 12.4, y: 60.8, width: 400.4, height: 300.2 } as DOMRect;
const error = vi.fn();
beforeEach(() => {
  vi.clearAllMocks(); vi.mocked(browserNativeBounds).mockResolvedValue(undefined); callbacks = new Map(); next = 0;
  vi.stubGlobal("requestAnimationFrame", (fn: FrameRequestCallback) => { callbacks.set(++next, fn); return next; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => callbacks.delete(id));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(rect);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
async function frame() { await act(async () => { const batch = [...callbacks.values()]; callbacks.clear(); batch.forEach(fn => fn(0)); }); }
it("rounds logical pixels, batches layout changes and hides on unmount", async () => {
  const view = render(<NativeSlot tabId="one" ready blocked={false} onError={error} />);
  expect(roundedRect(rect)).toEqual({ x: 12, y: 61, w: 400, h: 300 });
  window.dispatchEvent(new Event("resize")); window.dispatchEvent(new Event("scroll"));
  expect(browserNativeBounds).not.toHaveBeenCalled();
  await frame();
  expect(browserNativeBounds).toHaveBeenCalledExactlyOnceWith("one", { x: 12, y: 61, w: 400, h: 300 }, true);
  await frame(); expect(browserNativeBounds).toHaveBeenCalledTimes(1);
  view.unmount(); expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), false);
});
it.each(["modal", "popover"])("hides behind an intersecting %s with a snapshot, then restores", async kind => {
  const slot = <NativeSlot tabId="one" ready blocked={false} snapshot="AAAA" onError={error} />;
  const view = render(slot); await frame();
  view.rerender(<>{slot}{kind === "modal" ? <ModalShell>Modal</ModalShell> : <PopoverSurface>Popover</PopoverSurface>}</>); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), false);
  expect(screen.getByAltText("Last page snapshot").getAttribute("src")).toBe("data:image/png;base64,AAAA");
  expect(screen.getByLabelText("Native browser page").getAttribute("data-native-hidden")).toBe("true");
  view.rerender(slot); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), true);
});
it("keeps the first load hidden and detaches the previous active tab", async () => {
  const view = render(<NativeSlot tabId="one" ready={false} blocked={false} onError={error} />); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), false);
  view.rerender(<NativeSlot tabId="two" ready blocked={false} onError={error} />); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("two", expect.anything(), true);
  view.rerender(<NativeSlot tabId="two" ready blocked onError={error} />); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("two", expect.anything(), false);
});
it("does not hide for nonintersecting overlays or touching edges", async () => {
  expect(intersects(rect, { x: rect.x + rect.width, y: rect.y, width: 30, height: 20 })).toBe(false);
  const overlay = document.createElement("div"); document.body.append(overlay);
  vi.spyOn(overlay, "getBoundingClientRect").mockReturnValue({ x: 800, y: 800, width: 20, height: 20 } as DOMRect);
  const unregister = registerNativeOverlay(overlay);
  render(<NativeSlot tabId="one" ready blocked={false} onError={error} />); await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), true);
  unregister(); overlay.remove();
});
