// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { check } from "@tauri-apps/plugin-updater";
import { checkAppUpdate } from "../../../services/tauri";
import { UPDATE_CHECK_INTERVAL_MS, recordUpdateCheck, throttledUpdateCheck, useUpdater } from "./useUpdater";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("../../../services/tauri", () => ({ checkAppUpdate: vi.fn() }));
const checkMock = vi.mocked(checkAppUpdate);
const available = { status: "available" as const, currentVersion: __APP_VERSION__, latestVersion: "99.0.0", htmlUrl: "https://github.com/aiden0rchad/taucode/releases/tag/v99.0.0" };

beforeEach(() => { localStorage.clear(); checkMock.mockReset(); });

it("checks once on startup and shows a newer release without installing it", async () => {
  checkMock.mockResolvedValue(available);
  const { result, rerender } = renderHook(() => useUpdater({ enabled: true, autoCheckOnMount: true }));
  await waitFor(() => expect(result.current.state.stage).toBe("available"));
  rerender();
  expect(checkMock).toHaveBeenCalledTimes(1);
  expect(result.current.state.version).toBe("99.0.0");
  expect(check).not.toHaveBeenCalled();
});

it("reuses a check from the last day and re-checks after that", async () => {
  recordUpdateCheck({ status: "latest", currentVersion: __APP_VERSION__, latestVersion: __APP_VERSION__ }, 1_000);
  checkMock.mockResolvedValue(available);
  expect((await throttledUpdateCheck(1_000 + UPDATE_CHECK_INTERVAL_MS - 1)).status).toBe("latest");
  expect(checkMock).not.toHaveBeenCalled();
  expect((await throttledUpdateCheck(1_000 + UPDATE_CHECK_INTERVAL_MS)).status).toBe("available");
  expect(checkMock).toHaveBeenCalledTimes(1);
});

it("ignores a cached check made by another app version and never caches failures", async () => {
  recordUpdateCheck({ status: "latest", currentVersion: "0.0.1" }, Date.now());
  checkMock.mockResolvedValue({ status: "unavailable", currentVersion: __APP_VERSION__, message: "offline" });
  await throttledUpdateCheck();
  await throttledUpdateCheck();
  expect(checkMock).toHaveBeenCalledTimes(2);
});

it("stays quiet on startup when offline, up to date or the version was dismissed", async () => {
  for (const value of [{ status: "unavailable" as const, currentVersion: __APP_VERSION__, message: "offline" }, { status: "noReleases" as const, currentVersion: __APP_VERSION__ }]) {
    localStorage.clear();
    checkMock.mockResolvedValue(value);
    const { result } = renderHook(() => useUpdater({ enabled: true, autoCheckOnMount: true }));
    await waitFor(() => expect(checkMock).toHaveBeenCalled());
    await act(async () => {});
    expect(result.current.state.stage).toBe("idle");
    checkMock.mockClear();
  }
  localStorage.clear();
  checkMock.mockResolvedValue(available);
  const first = renderHook(() => useUpdater({ enabled: true }));
  await act(async () => { await first.result.current.checkForUpdates(); });
  expect(first.result.current.state.stage).toBe("available");
  act(() => first.result.current.dismiss());
  localStorage.removeItem("taucode.updates.lastCheck");
  const second = renderHook(() => useUpdater({ enabled: true }));
  await act(async () => { await second.result.current.checkForUpdates(); });
  expect(second.result.current.state.stage).toBe("idle");
});

it("announces the result of a manual check, including no releases", async () => {
  checkMock.mockResolvedValue({ status: "noReleases", currentVersion: __APP_VERSION__, message: "No releases published yet." });
  const { result } = renderHook(() => useUpdater({ enabled: true }));
  await act(async () => { await result.current.checkForUpdates({ announceNoUpdate: true }); });
  expect(result.current.state.stage).toBe("noReleases");
  checkMock.mockRejectedValue(new Error("ipc"));
  await act(async () => { await result.current.checkForUpdates({ announceNoUpdate: true }); });
  expect(result.current.state.stage).toBe("unavailable");
});
