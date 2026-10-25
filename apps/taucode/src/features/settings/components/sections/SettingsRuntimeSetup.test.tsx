// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { engineRuntime } from "@services/tauri";
import { SettingsRuntimeSetup } from "./SettingsRuntimeSetup";
vi.mock("@services/tauri", () => ({ engineRuntime: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("shows setup progress and actionable missing-tool errors", async () => {
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "pi", state: "pending", message: "Ready to set up on first use" }]);
  render(<SettingsRuntimeSetup />);
  fireEvent.click(await screen.findByText("Set up pi"));
  await waitFor(() => expect(engineRuntime).toHaveBeenCalledWith("prepare", "pi"));
  await waitFor(() => expect((screen.getByText("Set up pi") as HTMLButtonElement).disabled).toBe(false));
  vi.mocked(engineRuntime).mockRejectedValueOnce("Install system Node >=22.19 and npm");
  fireEvent.click(screen.getByText("Set up pi"));
  expect((await screen.findByRole("alert")).textContent).toContain("Install system Node >=22.19 and npm");
});
it("renders an active installation and prevents duplicate setup", async () => {
  vi.mocked(engineRuntime).mockResolvedValue([{ engine: "computer", state: "installing", message: "Installing Python dependencies…" }]);
  render(<SettingsRuntimeSetup />);
  expect((await screen.findByRole("status")).textContent).toBe("Installing");
  expect((screen.getByText("Set up computer") as HTMLButtonElement).disabled).toBe(true);
});
