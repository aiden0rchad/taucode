// @vitest-environment jsdom
import { useState } from "react";
import { act, cleanup, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { AppSettings } from "@/types";
import type { MenuViewAction } from "@services/events";
import { useUiScaleShortcuts } from "./useUiScaleShortcuts";
const host = vi.hoisted(() => ({ menu: (_action: MenuViewAction) => {}, zoom: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@services/events", () => ({ subscribeMenuViewAction: (listener: typeof host.menu) => { host.menu = listener; return () => { host.menu = () => {}; }; } }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ setZoom: host.zoom }) }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function setup(scale = 1) {
  const save = vi.fn(async (settings: AppSettings) => settings);
  const hook = renderHook(() => {
    const [settings, setSettings] = useState({ uiScale: scale } as AppSettings);
    return useUiScaleShortcuts({ settings, setSettings, saveSettings: save });
  });
  return { ...hook, save };
}
it("saves one scale step per native menu action, resets, and removes the listener", async () => {
  const { result, save, unmount } = setup();
  act(() => host.menu("zoom_in"));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(result.current.uiScale).toBe(1.1);
  expect(host.zoom).toHaveBeenLastCalledWith(1.1);
  act(() => host.menu("zoom_out"));
  expect(result.current.uiScale).toBe(1);
  act(() => host.menu("zoom_in"));
  act(() => host.menu("zoom_reset"));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(4));
  expect(host.zoom).toHaveBeenLastCalledWith(1);
  unmount();
  act(() => host.menu("zoom_in"));
  expect(save).toHaveBeenCalledTimes(4);
});
it("clamps menu zoom and leaves unrelated actions alone", async () => {
  const { result, save } = setup(3);
  act(() => host.menu("zoom_in"));
  act(() => host.menu("new_browser_tab"));
  expect(result.current.uiScale).toBe(3);
  expect(save).not.toHaveBeenCalled();
  act(() => host.menu("zoom_out"));
  await waitFor(() => expect(save).toHaveBeenCalledWith({ uiScale: 2.9 }));
});
it("keeps plus/minus keyboard aliases without handling consumed shortcuts twice", () => {
  const { result } = setup();
  const event = new KeyboardEvent("keydown", { key: "=", metaKey: true, cancelable: true });
  event.preventDefault();
  fireEvent(window, event);
  expect(result.current.uiScale).toBe(1);
  fireEvent.keyDown(window, { key: "+", metaKey: true, shiftKey: true });
  expect(result.current.uiScale).toBe(1.1);
  fireEvent.keyDown(window, { key: "-", metaKey: true });
  expect(result.current.uiScale).toBe(1);
});
