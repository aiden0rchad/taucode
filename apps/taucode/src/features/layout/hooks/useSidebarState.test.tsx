// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useSidebarState } from "./useSidebarState";
import { SidebarResizeHandle } from "../components/SidebarResizeHandle";

beforeEach(() => localStorage.clear());
afterEach(cleanup);
it("shares persisted width and collapse across simultaneous mode consumers", () => {
  localStorage.setItem("codexmonitor.sidebarWidth", "340");
  const hooks = [renderHook(useSidebarState), renderHook(useSidebarState), renderHook(useSidebarState)];
  expect(hooks.map(h => h.result.current.sidebarWidth)).toEqual([340, 340, 340]);
  act(() => hooks[1].result.current.setSidebarWidth(900));
  expect(hooks.map(h => h.result.current.sidebarWidth)).toEqual([420, 420, 420]);
  expect(document.documentElement.style.getPropertyValue("--sidebar-width")).toBe("420px");
  act(() => hooks[2].result.current.setSidebarCollapsed(true));
  expect(hooks.every(h => h.result.current.sidebarCollapsed)).toBe(true);
  hooks.forEach(h => h.unmount());
  const resumed = renderHook(useSidebarState); expect(resumed.result.current.sidebarWidth).toBe(420); expect(resumed.result.current.sidebarCollapsed).toBe(true);
});
it("clamps drag and keyboard resizing and releases listeners on unmount", () => {
  const { unmount } = render(<SidebarResizeHandle />);
  const handle = screen.getByRole("separator");
  handle.setPointerCapture = vi.fn();
  handle.hasPointerCapture = vi.fn(() => true);
  handle.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(handle, { pointerId: 1, isPrimary: true, button: 0, clientX: 280 }); fireEvent.pointerMove(handle, { pointerId: 1, isPrimary: true, button: 0, clientX: 0 }); fireEvent.pointerUp(handle, { pointerId: 1 });
  expect(handle.setPointerCapture).toHaveBeenCalledWith(1);
  expect(localStorage.getItem("codexmonitor.sidebarWidth")).toBe("220");
  fireEvent.keyDown(handle, { key: "ArrowRight" }); expect(localStorage.getItem("codexmonitor.sidebarWidth")).toBe("230");
  fireEvent.pointerDown(handle, { pointerId: 1, isPrimary: true, button: 0, clientX: 230 }); unmount(); fireEvent.pointerMove(handle, { pointerId: 1, isPrimary: true, button: 0, clientX: 400 });
  expect(document.body.style.cursor).toBe(""); expect(localStorage.getItem("codexmonitor.sidebarWidth")).toBe("230");
});

it("paints once per frame without notifying consumers or saving until release", () => {
  let frame: FrameRequestCallback = () => {};
  const raf = vi.spyOn(window, "requestAnimationFrame").mockImplementation(callback => { frame = callback; return 1; });
  const cancel = vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
  let renders = 0;
  const observer = renderHook(() => { renders++; return useSidebarState(); });
  const { container } = render(<div className="workspace-shell"><SidebarResizeHandle /></div>);
  const shell = container.firstElementChild as HTMLElement;
  vi.spyOn(shell, "getBoundingClientRect").mockReturnValue({ width: 760 } as DOMRect);
  const handle = screen.getByRole("separator");
  handle.setPointerCapture = vi.fn(); handle.hasPointerCapture = () => true; handle.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(handle, { pointerId: 1, button: 0, isPrimary: true, clientX: 280 });
  const before = renders;
  const saved = localStorage.getItem("codexmonitor.sidebarWidth");
  const paint = vi.spyOn(document.documentElement.style, "setProperty");
  fireEvent.pointerMove(handle, { pointerId: 1, clientX: 310 });
  fireEvent.pointerMove(handle, { pointerId: 2, clientX: 420 });
  fireEvent.pointerMove(handle, { pointerId: 1, clientX: 330 });
  expect(raf).toHaveBeenCalledTimes(1);
  expect(paint).not.toHaveBeenCalled();
  act(() => frame(0));
  expect(document.documentElement.style.getPropertyValue("--sidebar-width")).toBe("330px");
  expect(shell.style.getPropertyValue("--sidebar-width")).toBe("240px");
  expect(renders).toBe(before);
  expect(localStorage.getItem("codexmonitor.sidebarWidth")).toBe(saved);
  fireEvent.pointerMove(handle, { pointerId: 1, clientX: 350 });
  fireEvent(window, new Event("blur"));
  expect(observer.result.current.sidebarWidth).toBe(350);
  expect(localStorage.getItem("codexmonitor.sidebarWidth")).toBe("350");
  expect(document.querySelector(".panel-resize-overlay")).toBeNull();
  expect(document.documentElement.classList.contains("panel-resizing")).toBe(false);
  paint.mockRestore(); raf.mockRestore(); cancel.mockRestore();
});
