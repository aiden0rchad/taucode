// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { AppSettings, WorkspaceInfo } from "@/types";
import { defaultFeatures, features, FeatureSettings } from "./features";
import { engineRegistry, resolveEngine } from "./engines";
import { SettingsTaucodeFeaturesSection, featureGroups } from "./components/sections/SettingsTaucodeFeaturesSection";
import { SettingsNav } from "./components/SettingsNav";
import { ComposerEngineControl } from "@/features/composer/components/ComposerMetaBar";
import { useGitStatus } from "@/features/git/hooks/useGitStatus";
import { getGitStatus } from "@services/tauri";
import { RewindAction } from "@/features/checkpoints/Checkpoints";
import { ComposerDictation } from "@/features/composer/components/ComposerDictation";
import { ReasoningControl } from "@/features/composer/components/ReasoningControl";
import { modeShortcut } from "@/features/chat/ModeShell";

vi.mock("@services/tauri", () => ({ getGitStatus: vi.fn().mockResolvedValue({ branchName: "main", files: [], stagedFiles: [], unstagedFiles: [], totalAdditions: 0, totalDeletions: 0 }), engineRuntime: vi.fn().mockResolvedValue([]) }));
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.useRealTimers(); });
const settings = (value: Partial<AppSettings> = {}) => value as AppSettings;
it("migrates all feature defaults and keeps checkpoints independent of Git", () => {
  expect(features({}).diagnostics).toBe(false);
  expect(Object.entries(features({})).filter(([key]) => key !== "diagnostics").every(([, value]) => value)).toBe(true);
  expect(features({ features: { git: false } })).toMatchObject({ git: false, worktrees: false, checkpoints: true });
});
it.each(featureGroups.flatMap(([, rows]) => rows))("persists the %s toggle", async (key, title) => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<SettingsTaucodeFeaturesSection appSettings={settings()} onUpdateAppSettings={save} />);
  fireEvent.click(screen.getByRole("button", { name: title }));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ features: expect.objectContaining({ [key]: !defaultFeatures[key] }) }));
});
it("hides disabled settings entry points and rewind", () => {
  const allOff = Object.fromEntries(Object.keys(defaultFeatures).map(k => [k, false]));
  render(<FeatureSettings.Provider value={settings({ features: allOff })}><SettingsNav activeSection="features" onSelectSection={() => {}} /><RewindAction workspaceId="w" threadId="t" running={false} /></FeatureSettings.Provider>);
  for (const name of ["Git", "Browser", "Computer Use", "Connectors", "Import", "Dictation", "Rewind…"]) expect(screen.queryByRole("button", { name })).toBeNull();
  expect(screen.getByRole("button", { name: "Features" })).toBeTruthy();
});
it("stops Git polling immediately when disabled", async () => {
  vi.useFakeTimers();
  let enabled = true;
  const wrapper = ({ children }: { children: ReactNode }) => <FeatureSettings.Provider value={settings({ features: { git: enabled } })}>{children}</FeatureSettings.Provider>;
  const { rerender } = renderHook(() => useGitStatus({ id: "w", connected: true } as WorkspaceInfo), { wrapper });
  await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
  expect(getGitStatus).toHaveBeenCalled();
  enabled = false; rerender(); vi.mocked(getGitStatus).mockClear();
  await act(async () => { await vi.advanceTimersByTimeAsync(12000); });
  expect(getGitStatus).not.toHaveBeenCalled();
});
it("keeps Code available when both optional modes are disabled", () => {
  const app = settings({ features: { chatMode: false, workMode: false } });
  expect(modeShortcut(new KeyboardEvent("keydown", { metaKey: true, key: "1" }), app)).toBe("code");
  for (const key of ["2", "3"]) expect(modeShortcut(new KeyboardEvent("keydown", { metaKey: true, key }), app)).toBeUndefined();
});
it("preserves IDs, resolves unknown IDs with notice and filters engine pickers", () => {
  const engines = engineRegistry().map(e => ({ ...e, enabled: e.id !== "codex" }));
  const app = settings({ engines });
  expect(resolveEngine("missing", app)).toMatchObject({ engine: { id: "pi" }, notice: expect.stringContaining("Unknown engine missing") });
  const props = { engine: "pi", models: [], selectedModelId: null, onSelectModel: vi.fn(), disabled: false };
  const view = render(<FeatureSettings.Provider value={app}><ComposerEngineControl {...props} onSelectEngine={() => {}} /></FeatureSettings.Provider>);
  expect(screen.queryByRole("option", { name: "Codex" })).toBeNull();
  expect(screen.getByRole("option", { name: "oh-my-pi" })).toBeTruthy();
  view.rerender(<FeatureSettings.Provider value={app}><ComposerEngineControl {...props} engine="codex" /></FeatureSettings.Provider>);
  expect(screen.getByRole("status").textContent).toBe("Enable Codex to continue");
});

it("keeps reasoning selection working with every visual effect disabled", () => {
  const change = vi.fn();
  render(<FeatureSettings.Provider value={settings({ features: { reasoningEffects: false } })}><ReasoningControl capability={{ supported: true, levels: ["low", "high"], default: "low", control: "effort", source: "api", detectedAt: 1 }} value="high" onChange={change} /></FeatureSettings.Provider>);
  fireEvent.click(screen.getByRole("button", { name: "Model · High" }));
  const dialog = screen.getByRole("dialog");
  expect(dialog.classList.contains("effects-off")).toBe(true);
  expect(dialog.style.getPropertyValue("--reasoning-position")).toBe("100%");
  expect(dialog.querySelector(".reasoning-ember, .reasoning-particle, .reasoning-flame")).toBeNull();
  fireEvent.keyDown(screen.getByRole("slider"), { key: "ArrowLeft" });
  expect(change).toHaveBeenCalledWith("low");
});

it("hides the dictation mic, including its settings entry point", () => {
  render(<FeatureSettings.Provider value={settings({ features: { dictation: false } })}><ComposerDictation settings={settings({ dictationEnabled: false })} disabled={false} onTranscript={vi.fn()} onBusyChange={vi.fn()} onError={vi.fn()} onOpenSettings={vi.fn()} /></FeatureSettings.Provider>);
  expect(screen.queryByRole("button")).toBeNull();
});
