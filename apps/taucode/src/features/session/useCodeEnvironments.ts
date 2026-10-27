import { useCallback, useRef } from "react";
import type { AppSettings, CodeEnvironment, WorkspaceInfo } from "@/types";

export function useCodeEnvironments({ settings, saveSettings, workspaces, engine, modelId, selectWorkspace, setActiveThreadId, startDraft, restore }: {
  settings: AppSettings;
  saveSettings: (settings: AppSettings) => Promise<unknown>;
  workspaces: WorkspaceInfo[];
  engine: string;
  modelId: string | null;
  selectWorkspace: (id: string) => void;
  setActiveThreadId: (thread: string | null, workspace: string) => void;
  startDraft: (workspace: string) => void;
  restore: (profile: CodeEnvironment, workspaceId: string) => void;
}) {
  const latestSettings = useRef(settings);
  latestSettings.current = settings;
  const queue = useRef(Promise.resolve());
  const persist = useCallback((update: (profiles: CodeEnvironment[]) => CodeEnvironment[]) => {
    const task = async () => {
      const next = { ...latestSettings.current, codeEnvironments: update(latestSettings.current.codeEnvironments ?? []) };
      await saveSettings(next);
      latestSettings.current = next;
    };
    const result = queue.current.then(task, task);
    queue.current = result.catch(() => {});
    return result;
  }, [saveSettings]);
  const save = useCallback(async (name: string, ids: string[]) => {
    const workspaceIds = [...new Set(ids)].filter(id => workspaces.some(workspace => workspace.id === id));
    if (!name.trim() || name.trim().length > 80 || !workspaceIds.length) throw new Error("Enter a name and select an existing folder.");
    const profile = { id: crypto.randomUUID(), name: name.trim(), workspaceIds, engine, modelId };
    await persist(profiles => [...profiles, profile]);
  }, [workspaces, engine, modelId, persist]);
  const select = useCallback((profile: CodeEnvironment) => {
    const target = profile.workspaceIds.find(id => workspaces.some(workspace => workspace.id === id));
    if (!target) return;
    selectWorkspace(target);
    setActiveThreadId(null, target);
    startDraft(target);
    restore(profile, target);
  }, [workspaces, selectWorkspace, setActiveThreadId, startDraft, restore]);
  return { profiles: settings.codeEnvironments ?? [], save, select, remove: useCallback((id: string) => persist(profiles => profiles.filter(profile => profile.id !== id)), [persist]) };
}
