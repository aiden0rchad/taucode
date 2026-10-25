import { expect, it, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openSetupPrivacy } from "@services/tauri";
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
it("opens the exact macOS privacy URLs", async () => {
  await openSetupPrivacy("screen");
  await openSetupPrivacy("accessibility");
  expect(openUrl).toHaveBeenNthCalledWith(1, "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture");
  expect(openUrl).toHaveBeenNthCalledWith(2, "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
});
