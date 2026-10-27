// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { annotationPoint, BrowserAnnotations, selectionScript, type BrowserCapture } from "./BrowserAnnotations";
import { browserRequest } from "@services/tauri";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn() }));
const capture: BrowserCapture = { image: "data:image/png;base64,AAAA", width: 400, height: 300, url: "http://localhost/", tab: "one", page: { width: innerWidth, height: innerHeight, scrollX, scrollY, timeOrigin: performance.timeOrigin } };
const stroke = vi.fn(), strokeRect = vi.fn(), fillText = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("PointerEvent", MouseEvent);
  vi.stubGlobal("Image", class { naturalWidth = 800; naturalHeight = 600; src = ""; decode = async () => {}; });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage: vi.fn(), scale: vi.fn(), stroke, strokeRect, fillText, beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn() } as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue("data:image/png;base64,RESULT");
  vi.spyOn(SVGElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 400, height: 300 } as DOMRect);
  SVGElement.prototype.setPointerCapture = vi.fn(); SVGElement.prototype.hasPointerCapture = () => false;
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("maps a letterboxed resized screenshot without shifting marks", () => {
  expect(annotationPoint(300, 200, { left: 100, top: 100, width: 400, height: 200 }, { width: 400, height: 400 })).toEqual({ x: 200, y: 200 });
  expect(annotationPoint(100, 20, { left: 100, top: 100, width: 400, height: 200 }, { width: 400, height: 400 })).toEqual({ x: 0, y: 0 });
});

it("supports drawing, undo and redo, then adds one PNG to the draft", async () => {
  const onAdd = vi.fn(), onClose = vi.fn();
  render(<BrowserAnnotations capture={capture} onAdd={onAdd} onClose={onClose} />);
  const image = screen.getByRole("img");
  fireEvent.pointerDown(image, { clientX: 20, clientY: 30, button: 0 });
  fireEvent.pointerMove(image, { clientX: 70, clientY: 80 }); fireEvent.pointerUp(image);
  expect(document.querySelectorAll("polyline")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Undo annotation" }));
  expect(document.querySelectorAll("polyline")).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "Redo annotation" }));
  expect(document.querySelectorAll("polyline")).toHaveLength(1);
  expect(onAdd).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Add to chat" }));
  await waitFor(() => expect(onAdd).toHaveBeenCalledExactlyOnceWith({ image: "data:image/png;base64,RESULT", text: "Browser: http://localhost/" }));
  expect(stroke).toHaveBeenCalledOnce(); expect(onClose).toHaveBeenCalledExactlyOnceWith("add");
});

it("includes selected element metadata and survives an origin-policy error", async () => {
  const onAdd = vi.fn();
  vi.mocked(browserRequest).mockRejectedValueOnce(new Error("Browser origin blocked by policy")).mockResolvedValueOnce({ x: .1, y: .2, width: .3, height: .1, label: "h1 Welcome" });
  render(<BrowserAnnotations capture={capture} onAdd={onAdd} onClose={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Select element" }));
  fireEvent.pointerDown(screen.getByRole("img"), { clientX: 50, clientY: 80, button: 0 });
  await screen.findByRole("alert"); expect(screen.getByRole("alert").textContent).toContain("origin blocked");
  fireEvent.pointerDown(screen.getByRole("img"), { clientX: 50, clientY: 80, button: 0 });
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "Add to chat" }));
  await waitFor(() => expect(onAdd).toHaveBeenCalledWith({ image: "data:image/png;base64,RESULT", text: "Browser: http://localhost/\nSelected element: h1 Welcome" }));
  expect(strokeRect).toHaveBeenCalledWith(40, 60, 120, expect.closeTo(30));
});

it("does not add a capture after the editor closes during export", async () => {
  let finish!: () => void;
  vi.stubGlobal("Image", class { src = ""; decode = () => new Promise<void>(resolve => { finish = resolve; }); });
  const onAdd = vi.fn(), onClose = vi.fn();
  render(<BrowserAnnotations capture={capture} onAdd={onAdd} onClose={onClose} />);
  fireEvent.click(screen.getByRole("button", { name: "Add to chat" }));
  fireEvent.click(screen.getByRole("button", { name: "Close annotations" }));
  await act(async () => finish());
  expect(onAdd).not.toHaveBeenCalled(); expect(onClose).toHaveBeenCalledExactlyOnceWith("dismiss");
});

it("reads an element label without exposing input values and rejects a changed URL", () => {
  const input = document.createElement("input"); input.value = "secret"; input.setAttribute("aria-label", "Password");
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => input) });
  const run = new Function(`return (${selectionScript({ x: 30, y: 40 }, { ...capture, url: location.href })})()`);
  expect(run().label).toBe("input Password");
  expect(JSON.stringify(run())).not.toContain("secret");
  const parent = document.createElement("div"); parent.append(input); Object.defineProperty(parent, "innerText", { value: "secret descendant" });
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: () => parent });
  expect(run().label).toBe("div");
  expect(() => new Function(`return (${selectionScript({ x: 0, y: 0 }, { ...capture, url: "http://changed/" })})()`)()).toThrow("Page changed");
  expect(() => new Function(`return (${selectionScript({ x: 0, y: 0 }, { ...capture, url: location.href, page: { ...capture.page!, scrollY: 300 } })})()`)()).toThrow("Page moved");
});
