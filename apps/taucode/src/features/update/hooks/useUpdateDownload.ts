import { useSyncExternalStore } from "react";
import { downloadAppUpdate, openAppUpdate, type AppUpdateProgress } from "@services/tauri";

type Download = { progress: AppUpdateProgress | null; path: string | null; error: string | null };
const idle: Download = { progress: null, path: null, error: null };
const downloads = new Map<string, Download>();
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };

// About and the startup toast share a transfer; closing either view doesn't lose it.
export function useUpdateDownload(version: string) {
  const state = useSyncExternalStore(subscribe, () => downloads.get(version) ?? idle);
  const update = (patch: Partial<Download>) => {
    downloads.set(version, { ...(downloads.get(version) ?? idle), ...patch });
    listeners.forEach(listener => listener());
  };
  const start = async () => {
    if (downloads.get(version)?.progress) return;
    update({ path: null, error: null, progress: { downloaded: 0, total: 0 } });
    try { update({ path: await downloadAppUpdate(version, progress => update({ progress })) }); }
    catch (e) { update({ error: String(e) }); }
    finally { update({ progress: null }); }
  };
  const open = async () => {
    if (!state.path) return;
    update({ error: null });
    try { await openAppUpdate(state.path); } catch (e) { update({ error: `Couldn't open installer: ${String(e)}. Saved at ${state.path}` }); }
  };
  return { ...state, start, open };
}
