import { useSyncExternalStore, type SetStateAction } from "react";
import type { QueuedMessage } from "@/types";

/** Follow-ups queued per conversation. They outlive the Chat and Work views, which unmount when
 *  you switch modes, and send once you are back and the reply or task has completed. */
export function createQueueStore<T extends QueuedMessage>() {
  let state: Record<string, T[]> = {};
  const listeners = new Set<() => void>();
  const set = (update: SetStateAction<Record<string, T[]>>) => {
    state = typeof update === "function" ? update(state) : update;
    listeners.forEach(listener => listener());
  };
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  };
  const useQueue = () => [useSyncExternalStore(subscribe, () => state), set] as const;
  return { useQueue, clear: () => set({}) };
}
