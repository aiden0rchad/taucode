// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { check } from "@tauri-apps/plugin-updater";
import { useUpdater } from "./useUpdater";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); localStorage.clear(); });

it("never checks, installs, or fetches notes even when enabled or an old update is pending", async () => {
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  localStorage.setItem("codexmonitor.pendingPostUpdateVersion", __APP_VERSION__);
  const { result, rerender } = renderHook(() => useUpdater({ enabled: true, autoCheckOnMount: true }));
  await act(async () => {
    await result.current.checkForUpdates({ announceNoUpdate: true });
    await result.current.startUpdate();
    result.current.dismiss();
    result.current.dismissPostUpdateNotice();
  });
  rerender();
  expect(check).not.toHaveBeenCalled();
  expect(fetch).not.toHaveBeenCalled();
  expect(result.current.state).toEqual({ stage: "idle" });
  expect(result.current.postUpdateNotice).toBeNull();
});
