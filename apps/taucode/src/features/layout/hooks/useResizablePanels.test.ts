/** @vitest-environment jsdom */
import { act, cleanup, fireEvent, renderHook } from "@testing-library/react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getRightPanelMaxWidth, useResizablePanels } from "./useResizablePanels";

const widthKey = "codexmonitor.rightPanelWidth";
beforeEach(() => {
  localStorage.clear();
  vi.stubGlobal("innerWidth", 1860);
});
afterEach(() => { cleanup(); document.body.innerHTML = ""; vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function setup(renderedWidth = 900) {
  const hook = renderHook(useResizablePanels);
  const app = document.createElement("div");
  const panel = document.createElement("aside");
  panel.className = "tabbed-side-panel content-split";
  const handle = document.createElement("div");
  panel.append(handle); app.append(panel); document.body.append(app);
  hook.result.current.appRef.current = app;
  vi.spyOn(panel, "getBoundingClientRect").mockReturnValue({ width: renderedWidth, left: 0 } as DOMRect);
  Object.defineProperty(panel, "clientWidth", { value: 1000 });
  handle.setPointerCapture = vi.fn();
  handle.hasPointerCapture = vi.fn(() => true);
  handle.releasePointerCapture = vi.fn();
  const start = (callback = hook.result.current.onRightPanelResizeStart, clientX = 900) => act(() => callback({
    currentTarget: handle, clientX, clientY: 300, pointerId: 1, button: 0, preventDefault() {},
  } as unknown as ReactPointerEvent));
  const move = (clientX: number, clientY = 300, pointerId = 1) => act(() => fireEvent.pointerMove(handle, { clientX, clientY, pointerId }));
  const end = (type = "pointerup") => act(() => handle.dispatchEvent(new PointerEvent(type, { pointerId: 1 })));
  return { ...hook, app, handle, start, move, end };
}

describe("useResizablePanels", () => {
  it.each([
    [1860, 280, false, 1100],
    [1360, 280, false, 600],
    [1460, 420, true, 1100],
    [995, 280, false, 915],
    [1079, 280, false, 999],
    [1080, 280, false, 320],
    [900, 420, false, 820],
    [1000, 280, false, 920],
  ])("uses the layout's maximum in a %ipx window", (windowWidth, sidebarWidth, collapsed, expected) => {
    expect(getRightPanelMaxWidth(windowWidth, sidebarWidth, collapsed)).toBe(expected);
  });

  it("validates stored sizes", () => {
    localStorage.setItem("codexmonitor.sidebarWidth", "999");
    localStorage.setItem(widthKey, "100");
    localStorage.setItem("codexmonitor.planPanelHeight", "not-a-number");
    const { result } = renderHook(useResizablePanels);
    expect(result.current.sidebarWidth).toBe(420);
    expect(result.current.rightPanelWidth).toBe(320);
    expect(result.current.planPanelHeight).toBe(220);
  });

  it("keeps the preferred width while clamping on load, window resize and sidebar changes", () => {
    localStorage.setItem(widthKey, "900");
    vi.stubGlobal("innerWidth", 1360);
    const { result } = renderHook(useResizablePanels);
    expect(result.current.rightPanelWidth).toBe(600);
    act(() => {
      vi.stubGlobal("innerWidth", 1200);
      window.dispatchEvent(new Event("resize"));
    });
    expect(result.current.rightPanelWidth).toBe(440);
    expect(localStorage.getItem(widthKey)).toBe("900");
    act(() => {
      localStorage.setItem("codexmonitor.sidebarCollapsed", "true");
      window.dispatchEvent(new Event("taucode:sidebar"));
    });
    expect(result.current.rightPanelWidth).toBe(840);
    act(() => {
      vi.stubGlobal("innerWidth", 1860);
      window.dispatchEvent(new Event("resize"));
    });
    expect(result.current.rightPanelWidth).toBe(900);
    expect(localStorage.getItem(widthKey)).toBe("900");
  });

  it.each([
    [1860, 900, 650, 1100],
    [1360, 600, 920, 580],
    [1860, 750, 920, 730],
    [995, 900, 780, 915],
  ])("drags from the rendered width in a %ipx window", (windowWidth, rendered, pointerX, expected) => {
    localStorage.setItem(widthKey, "900");
    vi.stubGlobal("innerWidth", windowWidth);
    const hook = setup(rendered);
    hook.start();
    expect(hook.handle.setPointerCapture).toHaveBeenCalledWith(1);
    expect(hook.result.current.isResizing).toBe(true);
    hook.move(pointerX);
    expect(hook.app.style.getPropertyValue("--right-panel-width")).toBe(expected + "px");
    hook.end();
    expect(hook.result.current.rightPanelWidth).toBe(expected);
    expect(localStorage.getItem(widthKey)).toBe(String(expected));
    expect(hook.handle.releasePointerCapture).toHaveBeenCalledWith(1);
    expect(hook.result.current.isResizing).toBe(false);
    expect(document.querySelector(".panel-resize-overlay")).toBeNull();
  });

  it.each(["pointercancel", "lostpointercapture", "blur", "unmount"])("cleans up a drag on %s", reason => {
    const hook = setup();
    hook.start();
    hook.move(500, 300, 2);
    expect(hook.app.style.getPropertyValue("--right-panel-width")).toBe("");
    if (reason === "unmount") hook.unmount();
    else if (reason === "blur") act(() => window.dispatchEvent(new Event("blur")));
    else hook.end(reason);
    expect(document.querySelector(".panel-resize-overlay")).toBeNull();
    expect(document.body.style.cursor).toBe("");
    hook.move(500);
    expect(hook.app.style.getPropertyValue("--right-panel-width")).toBe("");
  });

  it.each([[750, 75], [250, 25]])("resizes the chat split with captured pointer events", (x, expected) => {
    const hook = setup();
    hook.start(hook.result.current.onChatDiffSplitPositionResizeStart, 500);
    hook.move(x); hook.end();
    expect(hook.result.current.chatDiffSplitPositionPercent).toBe(expected);
  });

  it.each([
    ["onPlanPanelResizeStart", "--plan-panel-height", "codexmonitor.planPanelHeight", 270],
    ["onTerminalPanelResizeStart", "--terminal-panel-height", "codexmonitor.terminalPanelHeight", 270],
    ["onDebugPanelResizeStart", "--debug-panel-height", "codexmonitor.debugPanelHeight", 230],
  ] as const)("resizes %s with pointer capture", (handler, property, key, expected) => {
    const hook = setup();
    hook.start(hook.result.current[handler]);
    expect(document.body.style.cursor).toBe("row-resize");
    hook.move(900, 250); hook.end();
    expect(hook.app.style.getPropertyValue(property)).toBe(expected + "px");
    expect(localStorage.getItem(key)).toBe(String(expected));
  });
});
