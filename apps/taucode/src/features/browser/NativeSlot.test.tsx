// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NativeSlot, roundedRect } from "./NativeSlot";
import { browserNativeBounds } from "@services/tauri";
import { intersects, registerNativeOverlay } from "@/features/design-system/nativeOverlays";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
import { useResizablePanels } from "@/features/layout/hooks/useResizablePanels";
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
it("refreshes native placement when UI zoom changes the viewport with fixed slot bounds", async () => {
  render(<NativeSlot tabId="one" ready blocked={false} onError={error} />);
  await frame();
  vi.stubGlobal("innerWidth", window.innerWidth + 200);
  await frame();
  expect(browserNativeBounds).toHaveBeenCalledTimes(2);
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", { x: 12, y: 61, w: 400, h: 300 }, true);
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

it.each(["sidebar", "side panel"])("hides the native view behind the %s resize overlay and restores it on release", async kind => {
  localStorage.clear();
  function Panels() {
    const resize = useResizablePanels();
    return <div ref={resize.appRef} className="tabbed-side-panel">
      <div role="separator" onPointerDown={kind === "sidebar" ? resize.onSidebarResizeStart : resize.onRightPanelResizeStart} />
      <NativeSlot tabId="one" ready blocked={false} snapshot="AAAA" onError={error} />
    </div>;
  }
  render(<Panels />); await frame();
  const handle = screen.getByRole("separator");
  handle.setPointerCapture = vi.fn();
  handle.hasPointerCapture = vi.fn(() => true);
  handle.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(handle, { pointerId: 1, button: 0, isPrimary: true, clientX: 400 });
  expect(handle.setPointerCapture).toHaveBeenCalledWith(1);
  expect(document.querySelector<HTMLElement>(".panel-resize-overlay")?.style.cursor).toBe("col-resize");
  await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), false);
  expect(screen.getByLabelText("Native browser page").getAttribute("data-native-hidden")).toBe("true");
  expect(screen.getByAltText("Last page snapshot").getAttribute("src")).toBe("data:image/png;base64,AAAA");
  fireEvent.pointerUp(handle, { pointerId: 1 }); await frame();
  expect(document.querySelector(".panel-resize-overlay")).toBeNull();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expect.anything(), true);
});

it("preserves full page bounds while annotation blocking hides and restores the native view", async () => {
  const view = render(<NativeSlot tabId="one" ready blocked={false} onError={error} />);
  await frame();
  const expected = { x: 12, y: 61, w: 400, h: 300 };
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expected, true);
  vi.mocked(browserNativeBounds).mockClear();
  // A screenshot-backed annotation overlay changes visibility, not the viewport.
  view.rerender(<NativeSlot tabId="one" ready blocked snapshot="AAAA" onError={error} />);
  await frame(); await frame();
  expect(browserNativeBounds).toHaveBeenCalled();
  for (const [tab, bounds, visible] of vi.mocked(browserNativeBounds).mock.calls) {
    expect(tab).toBe("one"); expect(bounds).toEqual(expected); expect(visible).toBe(false);
  }
  expect(screen.getByLabelText("Native browser page").getAttribute("data-native-hidden")).toBe("true");
  view.rerender(<NativeSlot tabId="one" ready blocked={false} snapshot="AAAA" onError={error} />);
  await frame();
  expect(browserNativeBounds).toHaveBeenLastCalledWith("one", expected, true);
});
