// @vitest-environment jsdom
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { WindowResizeHandles } from "./WindowResizeHandles";

const mocks = vi.hoisted(() => ({ mac: false, resize: vi.fn().mockResolvedValue(undefined), size: vi.fn().mockResolvedValue(undefined), position: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@utils/platformPaths", () => ({ isMacPlatform: () => mocks.mac }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({
  PhysicalSize: class { constructor(public width: number, public height: number) {} },
  PhysicalPosition: class { constructor(public x: number, public y: number) {} },
  getCurrentWindow: () => ({ startResizeDragging: mocks.resize, setSize: mocks.size, setPosition: mocks.position, outerPosition: async () => ({ x: 100, y: 80 }), outerSize: async () => ({ width: 2000, height: 1400 }), scaleFactor: async () => 2, isFullscreen: async () => false, isMaximized: async () => false }),
}));
afterEach(() => { cleanup(); vi.clearAllMocks(); mocks.mac = false; });
it("routes supported native edges and corners without treating right clicks as drags", () => {
  const { container } = render(<WindowResizeHandles />);
  expect(container.querySelector("[data-tauri-drag-region]")).toBeNull();
  fireEvent.pointerDown(container.querySelector(".East")!, { button: 0 });
  expect(mocks.resize).toHaveBeenLastCalledWith("East");
  fireEvent.pointerDown(container.querySelector(".SouthEast")!, { button: 0 });
  expect(mocks.resize).toHaveBeenLastCalledWith("SouthEast");
  fireEvent.pointerDown(container.querySelector(".North")!, { button: 2 });
  expect(mocks.resize).toHaveBeenCalledTimes(2);
});
it("resizes macOS from physical cursor movement and preserves the opposite edge at minimum size", async () => {
  mocks.mac = true;
  const { container } = render(<WindowResizeHandles />);
  const handle = container.querySelector(".NorthWest")! as HTMLElement;
  handle.setPointerCapture = vi.fn(); handle.releasePointerCapture = vi.fn(); handle.hasPointerCapture = () => true;
  await act(async () => { fireEvent.pointerDown(handle, { button: 0, pointerId: 1, isPrimary: true, screenX: 50, screenY: 40 }); });
  await act(async () => {
    fireEvent.pointerMove(handle, { pointerId: 1, screenX: 950, screenY: 640 });
    fireEvent.pointerUp(handle, { pointerId: 1 });
  });
  expect(mocks.size).toHaveBeenCalledWith({ width: 720, height: 1200 });
  expect(mocks.position).toHaveBeenCalledWith({ x: 1380, y: 280 });
  expect(mocks.resize).not.toHaveBeenCalled();
  fireEvent.pointerUp(handle, { pointerId: 1 });
  expect(document.querySelector(".panel-resize-overlay")).toBeNull();
});
