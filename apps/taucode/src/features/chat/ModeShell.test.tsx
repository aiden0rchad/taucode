// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppSettings } from "@/types";
import { ModeShell, ModeSwitcher, modeShortcut, useModeSettings } from "./ModeShell";
import { getAppSettings, setWindowMode, windowSetMaterial } from "@services/tauri";
import { applyTheme } from '@/features/themes/applyTheme';
import { builtins } from '@/features/themes/builtins';
vi.mock("@services/tauri", () => ({ getAppSettings: vi.fn(), setWindowMode: vi.fn().mockResolvedValue(undefined), themesList: vi.fn(async () => []), windowSetMaterial: vi.fn(async () => {}), updateAppSettings: vi.fn(async (settings: unknown) => settings) }));
vi.mock("@/features/onboarding/SetupChecklist", () => ({ SetupChecklist: () => null }));
vi.mock("@/features/work/WorkView", () => ({ WorkView: () => <><ModeSwitcher /><p>Work content</p></> }));
vi.mock("./ChatView", () => ({ ChatView: () => <><ModeSwitcher /><p>Chat content</p></> }));
vi.mock("@/features/evals/EvalsView", () => ({ EvalsView: ({full,initialRunId,onClose}:{full:boolean;initialRunId?:string;onClose:()=>void}) => <section aria-label="Full eval results"><p>{full?initialRunId:"compact"}</p><button onClick={onClose}>Close results</button></section> }));
vi.mock("@/features/layout/hooks/useWindowLabel", () => ({ useWindowLabel: () => "second-window" }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it('keeps the cached theme while shell and code settings load', async () => {
  applyTheme(builtins.find(t => t.id === 'glass-dark')!);
  await act(async () => {});
  vi.mocked(windowSetMaterial).mockClear();
  let resolve!: (settings: AppSettings) => void;
  vi.mocked(getAppSettings).mockReturnValue(new Promise(done => { resolve = done; }));
  function LoadingCode() {
    useModeSettings(null);
    return <p>Code loading</p>;
  }
  render(<ModeShell><LoadingCode /></ModeShell>);
  await act(async () => {});
  expect(windowSetMaterial).not.toHaveBeenCalled();
  await act(async () => { resolve({ theme: 'glass-dark', windowModes: {} } as AppSettings); });
  expect(screen.getByText('Code loading')).toBeTruthy();
  expect(document.documentElement.dataset.themeId).toBe('glass-dark');
  expect(windowSetMaterial).not.toHaveBeenCalled();
});
it("opens the selected evaluation in the main area and returns to the previous mode",async()=>{
 vi.mocked(getAppSettings).mockResolvedValue({windowModes:{"second-window":"code"}} as unknown as AppSettings);
 render(<ModeShell><p>Code content</p></ModeShell>);
 await screen.findByText("Code content");
 act(()=>{window.dispatchEvent(new CustomEvent("taucode:evals-full",{detail:{runId:"eval-b"}}));});
 expect(screen.getByLabelText("Full eval results").textContent).toContain("eval-b");
 expect(screen.getByText("Code content").parentElement?.style.display).toBe("none");
 fireEvent.click(screen.getByRole("button",{name:"Close results"}));
 expect(screen.queryByLabelText("Full eval results")).toBeNull();
 expect(screen.getByText("Code content").parentElement?.style.display).toBe("contents");
});
describe("mode switching", () => {
  it("restores this window's mode and persists changes", async () => {
    vi.mocked(getAppSettings).mockResolvedValue({ theme: 'system', windowModes: { main: "code", "second-window": "chat" } } as unknown as AppSettings);
    render(<ModeShell><ModeSwitcher /><p>Code content</p></ModeShell>);
    expect(await screen.findByText("Chat content")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Work" }));
    await waitFor(() => expect(setWindowMode).toHaveBeenCalledWith("work"));
    expect(await screen.findByText("Work content")).toBeTruthy();
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

it("falls back to Code when a saved mode is disabled and hides both disabled segments", async () => {
  vi.mocked(getAppSettings).mockResolvedValue({ theme: 'system', windowModes: { "second-window": "chat" }, features: { chatMode: false, workMode: false } } as unknown as AppSettings);
  render(<ModeShell><ModeSwitcher /><p>Code content</p></ModeShell>);
  expect(await screen.findByText("Code content")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Chat" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Work" })).toBeNull();
  expect(screen.getByRole("button", { name: "Code" })).toBeTruthy();
  expect(setWindowMode).toHaveBeenCalledWith("code");
});
