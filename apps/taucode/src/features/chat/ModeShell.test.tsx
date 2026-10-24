// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppSettings } from "@/types";
import { ModeShell, ModeSwitcher, modeShortcut } from "./ModeShell";
import { getAppSettings, setWindowMode } from "@services/tauri";
vi.mock("@services/tauri", () => ({ getAppSettings: vi.fn(), setWindowMode: vi.fn().mockResolvedValue(undefined) }));
vi.mock("./ChatView", () => ({ ChatView: () => <><ModeSwitcher /><p>Chat content</p></> }));
vi.mock("@/features/layout/hooks/useWindowLabel", () => ({ useWindowLabel: () => "second-window" }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
describe("mode switching", () => {
  it("restores this window's mode and persists changes", async () => {
    vi.mocked(getAppSettings).mockResolvedValue({ windowModes: { main: "code", "second-window": "chat" } } as unknown as AppSettings);
    render(<ModeShell><ModeSwitcher /><p>Code content</p></ModeShell>);
    expect(await screen.findByText("Chat content")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Work" }));
    await waitFor(() => expect(setWindowMode).toHaveBeenCalledWith("work"));
    expect(await screen.findByText("Work mode is coming soon.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Code" }));
    expect(await screen.findByText("Code content")).toBeTruthy();
  });
  it("reserves only unbound command-number shortcuts", () => {
    const event = new KeyboardEvent("keydown", { key: "2", metaKey: true });
    expect(modeShortcut(event, {} as AppSettings)).toBe("chat");
    expect(modeShortcut(event, { composerModelShortcut: "meta+2" } as AppSettings)).toBeUndefined();
    expect(modeShortcut(new KeyboardEvent("keydown", { key: "2", metaKey: true, shiftKey: true }), {} as AppSettings)).toBeUndefined();
  });
});
