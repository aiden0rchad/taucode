// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { downloadAppUpdate, openAppUpdate, type AppUpdateProgress } from "@services/tauri";
import { UpdateDownload } from "./UpdateDownload";
vi.mock("@services/tauri", () => ({ downloadAppUpdate: vi.fn(), openAppUpdate: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("downloads only on click, shows progress, and opens only on a second click", async () => {
  let finish!: (path: string) => void;
  let progress!: (p: AppUpdateProgress) => void;
  vi.mocked(downloadAppUpdate).mockImplementation((_, listener) => { progress = listener; return new Promise(resolve => { finish = resolve; }); });
  render(<UpdateDownload version="2.0.0" available />);
  expect(downloadAppUpdate).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Download update"));
  expect(downloadAppUpdate).toHaveBeenCalledWith("2.0.0", expect.any(Function));
  act(() => progress({ downloaded: 50, total: 100 }));
  expect(screen.getByRole("progressbar")).toHaveProperty("value", 50);
  await act(async () => finish("/Downloads/update.dmg"));
  expect(openAppUpdate).not.toHaveBeenCalled();
  cleanup();
  render(<UpdateDownload version="2.0.0" available />);
  expect(downloadAppUpdate).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByText("Open installer"));
  expect(openAppUpdate).toHaveBeenCalledWith("/Downloads/update.dmg");
});
it("offers retry after a failed download", async () => {
  vi.mocked(downloadAppUpdate).mockRejectedValue("Network interrupted");
  render(<UpdateDownload version="3.0.0" available />);
  fireEvent.click(screen.getByText("Download update"));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Network interrupted");
  expect(screen.getByText("Download update")).toBeTruthy();
});
it("does not offer another platform's installer", () => {
  render(<UpdateDownload version="4.0.0" available={false} />);
  expect(screen.queryByRole("button")).toBeNull();
  expect(screen.getByText(/No compatible installer/)).toBeTruthy();
});
