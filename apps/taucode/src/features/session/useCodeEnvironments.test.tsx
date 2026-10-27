/** @vitest-environment jsdom */
import { useLayoutEffect, useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, CodeEnvironment, WorkspaceInfo } from "@/types";
import { useCodeEnvironments } from "./useCodeEnvironments";
import { useModels } from "@/features/models/hooks/useModels";
import { getConfigModel, getModelList } from "@services/tauri";
vi.mock("@services/tauri", () => ({ getConfigModel: vi.fn(), getModelList: vi.fn() }));
beforeEach(() => vi.clearAllMocks());
const workspaces = [{ id: "w1" }, { id: "w2" }] as WorkspaceInfo[];
const profile: CodeEnvironment = { id: "saved", name: "Project", workspaceIds: ["removed", "w2", "w1"], engine: "omp", modelId: "provider/model" };
function options() {
  return { settings: { codeEnvironments: [] } as unknown as AppSettings, saveSettings: vi.fn().mockResolvedValue(undefined), workspaces, engine: "pi", modelId: "current/model", selectWorkspace: vi.fn(), setActiveThreadId: vi.fn(), startDraft: vi.fn(), restore: vi.fn() };
}
describe("saved environments", () => {
  it("persists only named workspace references and engine/model metadata", async () => {
    const props = options(); const { result } = renderHook(() => useCodeEnvironments(props));
    await act(async () => result.current.save(" Project ", ["w2", "w2", "missing", "w1"]));
    expect(props.saveSettings.mock.calls[0][0].codeEnvironments).toEqual([{ id: expect.any(String), name: "Project", workspaceIds: ["w2", "w1"], engine: "pi", modelId: "current/model" }]);
    await expect(result.current.save(" ", ["w1"])).rejects.toThrow("Enter a name");
    await expect(result.current.save("Project", ["missing"])).rejects.toThrow("existing folder");
    expect(props.saveSettings).toHaveBeenCalledTimes(1);
  });
  it("serializes profile changes and retains previous profiles", async () => {
    const props = options(); props.settings.codeEnvironments = [profile];
    const { result } = renderHook(() => useCodeEnvironments(props));
    await act(async () => { await Promise.all([result.current.save("Second", ["w1"]), result.current.remove("saved")]); });
    expect(props.saveSettings.mock.calls[1][0].codeEnvironments).toEqual([{ id: expect.any(String), name: "Second", workspaceIds: ["w1"], engine: "pi", modelId: "current/model" }]);
  });
  it("selects the first existing folder and supplies declarative draft preferences", () => {
    const props = options();
    const { result } = renderHook(() => useCodeEnvironments(props));
    act(() => result.current.select(profile));
    expect(props.selectWorkspace).toHaveBeenCalledWith("w2");
    expect(props.setActiveThreadId).toHaveBeenCalledWith(null, "w2");
    expect(props.startDraft).toHaveBeenCalledWith("w2");
    expect(props.restore).toHaveBeenCalledWith(profile, "w2");
    act(() => result.current.select({ ...profile, workspaceIds: ["missing"] }));
    expect(props.selectWorkspace).toHaveBeenCalledTimes(1);
  });
  it("keeps the saved model through late model-list responses and draft defaults", async () => {
    const response = { result: { data: [
      { id: "sol", model: "sol", displayName: "Sol", isDefault: true, supportedReasoningEfforts: [], defaultReasoningEffort: null },
      { id: "astra", model: "astra", displayName: "Astra", isDefault: false, supportedReasoningEfforts: [], defaultReasoningEffort: null },
    ] } };
    vi.mocked(getModelList).mockResolvedValue(response);
    vi.mocked(getConfigModel).mockResolvedValue("sol");
    const base = options();
    const workspace = { id: "w1", name: "Project", path: "/project", connected: true, settings: { sidebarCollapsed: false } };
    const { result } = renderHook(() => {
      const [threadId, setThreadId] = useState<string | null>("existing");
      const [engine, setEngine] = useState("omp");
      const [scope, setScope] = useState("existing");
      const [preferred, setPreferred] = useState("sol");
      const [draft, setDraft] = useState<{ profile: CodeEnvironment; revision: number } | null>(null);
      const models = useModels({ activeWorkspace: workspace, threadId, preferredModelId: !threadId && draft ? draft.profile.modelId : preferred, selectionKey: !threadId && draft ? `${scope}:environment:${draft.revision}` : scope });
      // MainApp's thread sync runs in a layout effect after the draft becomes active.
      useLayoutEffect(() => { setScope(threadId ?? "new-draft"); setPreferred("sol"); }, [threadId]);
      const environments = useCodeEnvironments({ ...base, workspaces: [workspace], engine, modelId: models.selectedModelId,
        selectWorkspace: vi.fn(), setActiveThreadId: setThreadId, startDraft: vi.fn(),
        restore: profile => { setEngine(profile.engine); setDraft(previous => ({ profile, revision: (previous?.revision ?? 0) + 1 })); },
      });
      return { environments, models, engine };
    });
    await waitFor(() => expect(result.current.models.selectedModelId).toBe("sol"));
    let finish!: (value: typeof response) => void;
    vi.mocked(getModelList).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    let refresh!: Promise<void>;
    act(() => { refresh = result.current.models.refreshModels(); }); // Captures the old Sol selection.
    act(() => result.current.models.setSelectedModelId("astra"));
    await act(async () => result.current.environments.save("Saved Astra", ["w1"]));
    const saved = base.saveSettings.mock.calls[0][0].codeEnvironments[0] as CodeEnvironment;
    expect(saved.modelId).toBe("astra");
    act(() => result.current.environments.select(saved));
    await act(async () => { finish(response); await refresh; });
    await waitFor(() => expect(result.current.models.selectedModelId).toBe("astra"));
    expect(result.current.engine).toBe("omp");
    vi.mocked(getModelList).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    act(() => { refresh = result.current.models.refreshModels(); }); // Captures Astra before the manual change.
    act(() => result.current.models.setSelectedModelId("sol")); // A manual choice still wins, even over late data.
    await act(async () => { finish(response); await refresh; });
    expect(result.current.models.selectedModelId).toBe("sol");
    act(() => result.current.environments.select(saved)); // Selecting the same profile restores it again.
    await waitFor(() => expect(result.current.models.selectedModelId).toBe("astra"));
  });
  it("retains failed-save errors and recovers on the next save", async () => {
    const props = options(); props.saveSettings.mockRejectedValueOnce(new Error("Disk full"));
    const { result } = renderHook(() => useCodeEnvironments(props));
    await expect(result.current.save("First", ["w1"])).rejects.toThrow("Disk full");
    await act(async () => result.current.save("Retry", ["w1"]));
    expect(props.saveSettings.mock.calls[1][0].codeEnvironments).toHaveLength(1);
  });
});
