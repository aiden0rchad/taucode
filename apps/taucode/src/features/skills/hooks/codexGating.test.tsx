// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { PropsWithChildren } from "react";
import type { AppSettings, EngineKind, WorkspaceInfo } from "@/types";
import { FeatureSettings } from "@settings/features";
import { getCollaborationModes, getSkillsList } from "@services/tauri";
import { useSkills } from "./useSkills";
import { useCollaborationModes } from "@/features/collaboration/hooks/useCollaborationModes";

vi.mock("@services/tauri", () => ({ getSkillsList: vi.fn(async () => ({ skills: [{ name: "skill", path: "/skill" }] })), getCollaborationModes: vi.fn(async () => ({ data: [{ mode: "plan" }] })) }));
vi.mock("@services/events", () => ({ subscribeAppServerEvents: () => () => {} }));
const settings = { providers: [{ id: "p", kind: "openai", enabled: true, models: [{ id: "m", enabled: true }] }] } as AppSettings;
const workspace = { id: "w", connected: true, settings: {} } as WorkspaceInfo;
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it.each(["pi", "omp", "claude"])("makes no Codex calls or errors for %s", async engine => {
  const debug = vi.fn();
  const wrapper = ({ children }: PropsWithChildren) => <FeatureSettings.Provider value={settings}>{children}</FeatureSettings.Provider>;
  const { result } = renderHook(() => ({ skills: useSkills({ activeWorkspace: workspace, engine, onDebug: debug }), modes: useCollaborationModes({ activeWorkspace: workspace, engine, enabled: true, onDebug: debug }) }), { wrapper });
  await act(async () => { await result.current.skills.refreshSkills(); await result.current.modes.refreshCollaborationModes(); });
  expect(getSkillsList).not.toHaveBeenCalled(); expect(getCollaborationModes).not.toHaveBeenCalled(); expect(debug).not.toHaveBeenCalled();
  expect(result.current.skills.skills).toEqual([]); expect(result.current.modes.collaborationModes).toEqual([]);
});
it("calls for ready Codex and clears results when switching engines", async () => {
  const wrapper = ({ children }: PropsWithChildren) => <FeatureSettings.Provider value={settings}>{children}</FeatureSettings.Provider>;
  const { result, rerender } = renderHook(({ engine }: { engine: EngineKind }) => ({ skills: useSkills({ activeWorkspace: workspace, engine }), modes: useCollaborationModes({ activeWorkspace: workspace, engine, enabled: true }) }), { wrapper, initialProps: { engine: "codex" } });
  await waitFor(() => expect(result.current.skills.skills).toHaveLength(1));
  expect(getCollaborationModes).toHaveBeenCalledOnce();
  rerender({ engine: "pi" });
  expect(result.current.skills.skills).toEqual([]); expect(result.current.modes.collaborationModes).toEqual([]);
  rerender({ engine: "codex" });
  await waitFor(() => expect(getSkillsList).toHaveBeenCalledTimes(2));
});
it("does not call an unconfigured Codex", () => {
  renderHook(() => { useSkills({ activeWorkspace: workspace, engine: "codex" }); useCollaborationModes({ activeWorkspace: workspace, engine: "codex", enabled: true }); });
  expect(getSkillsList).not.toHaveBeenCalled(); expect(getCollaborationModes).not.toHaveBeenCalled();
});
