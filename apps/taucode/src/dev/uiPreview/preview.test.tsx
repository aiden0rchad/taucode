// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { StrictMode } from "react";
import App from "@/App";
import { installUiPreview } from "./index";

// This plugin ships an extensionless import that Node cannot resolve. Vite can.
vi.mock("tauri-plugin-liquid-glass-api", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  return { isGlassSupported: () => invoke("plugin:liquid-glass|is_glass_supported"), setLiquidGlassEffect: () => invoke("plugin:liquid-glass|set_liquid_glass_effect") };
});

afterEach(async () => { cleanup(); await new Promise(resolve => setTimeout(resolve, 50)); clearMocks(); vi.unstubAllEnvs(); vi.restoreAllMocks(); });

it.each([
  ["?mode=code&thread=thread-rich", "Update the greeting, run the tests"],
  ["?mode=chat&chat=chat-1", "A starting point"],
  ["?mode=work&task=task-needs-input", "Approval required"],
  ["?settings=display&theme=light", "Display"],
])("mounts the real app at %s", async (query, text) => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/" + query);
  installUiPreview();
  const warn = vi.spyOn(console, "warn");
  const error = vi.spyOn(console, "error");
  render(<StrictMode><App /></StrictMode>);
  await waitFor(() => expect(screen.getAllByText(new RegExp(text)).length).toBeGreaterThan(0), { timeout: 10000 });
  expect(warn.mock.calls.filter(([message]) => String(message).includes("Unhandled IPC"))).toEqual([]);
  expect(error).not.toHaveBeenCalled();
}, 15000);

it("sends a chat through real IPC and event subscriptions", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=chat&chat=chat-1");
  installUiPreview();
  render(<StrictMode><App /></StrictMode>);
  await screen.findByText(/A starting point/);
  const input = screen.getByRole("textbox", { name: "Message" });
  fireEvent.change(input, { target: { value: "A preview question" } });
  fireEvent.click(screen.getByRole("button", { name: "Send ↑" }));
  await screen.findByText(/Preview response:/);
});
