import { useCallback, useEffect, useRef, useState } from "react";
import { checkAppUpdate, type AppUpdateCheck } from "../../../services/tauri";
import type { DebugEntry } from "../../../types";

// taucode only checks GitHub Releases and links to the download; it never installs anything.
type UpdateStage = "idle" | "checking" | AppUpdateCheck["status"];

export type UpdateState = {
  stage: UpdateStage;
  version?: string;
  result?: AppUpdateCheck;
  error?: string;
};

export type PostUpdateNoticeState = null;

type UseUpdaterOptions = {
  enabled?: boolean;
  autoCheckOnMount?: boolean;
  onDebug?: (entry: DebugEntry) => void;
};

export const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const lastCheckKey = "taucode.updates.lastCheck";
const dismissedKey = "taucode.updates.dismissedVersion";

type StoredCheck = { at: number; result: AppUpdateCheck };

function readStorage(key: string) {
  try { return localStorage.getItem(key); } catch { return null; }
}
function writeStorage(key: string, value: string) {
  try { localStorage.setItem(key, value); } catch { /* storage unavailable */ }
}

export function readLastUpdateCheck(): StoredCheck | null {
  try {
    const value = JSON.parse(readStorage(lastCheckKey) ?? "null") as StoredCheck | null;
    // A check made by an older build says nothing about this one.
    return value && typeof value.at === "number" && value.result?.currentVersion === __APP_VERSION__ ? value : null;
  } catch { return null; }
}

export function recordUpdateCheck(result: AppUpdateCheck, at = Date.now()) {
  // Failed checks are retried on the next launch instead of waiting a day.
  if (result.status !== "unavailable") writeStorage(lastCheckKey, JSON.stringify({ at, result }));
}

/** Run a check unless one finished within the last day; return the cached result in that case. */
export async function throttledUpdateCheck(now = Date.now()): Promise<AppUpdateCheck> {
  const last = readLastUpdateCheck();
  if (last && now - last.at < UPDATE_CHECK_INTERVAL_MS) return last.result;
  const result = await checkAppUpdate();
  recordUpdateCheck(result, now);
  return result;
}

const idleState: UpdateState = { stage: "idle" };

export function useUpdater({ enabled = true, autoCheckOnMount = false, onDebug }: UseUpdaterOptions) {
  const [state, setState] = useState<UpdateState>(idleState);
  const autoChecked = useRef(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const show = useCallback((result: AppUpdateCheck, announce: boolean) => {
    clearTimeout(hideTimer.current);
    if (result.status === "available") {
      if (!announce && readStorage(dismissedKey) === result.latestVersion) return setState(idleState);
      return setState({ stage: "available", version: result.latestVersion, result });
    }
    if (!announce) return setState(idleState);
    setState({ stage: result.status, version: result.latestVersion, result, error: result.status === "unavailable" ? result.message : undefined });
    hideTimer.current = setTimeout(() => setState(idleState), 6000);
  }, []);

  const checkForUpdates = useCallback(async (options?: { announceNoUpdate?: boolean }) => {
    if (!enabled) return;
    const announce = options?.announceNoUpdate ?? false;
    if (announce) setState({ stage: "checking" });
    try {
      const result = announce ? await checkAppUpdate() : await throttledUpdateCheck();
      if (announce) recordUpdateCheck(result);
      show(result, announce);
    } catch (error) {
      onDebug?.({ id: `${Date.now()}-client-updater-error`, timestamp: Date.now(), source: "error", label: "updater/check-error", payload: error instanceof Error ? error.message : String(error) });
      show({ status: "unavailable", currentVersion: __APP_VERSION__, message: "Couldn't check for updates. taucode will try again later." }, announce);
    }
  }, [enabled, onDebug, show]);

  useEffect(() => {
    if (!enabled || !autoCheckOnMount || autoChecked.current) return;
    autoChecked.current = true;
    void checkForUpdates();
  }, [autoCheckOnMount, checkForUpdates, enabled]);

  useEffect(() => () => clearTimeout(hideTimer.current), []);

  const startUpdate = useCallback(() => {
    const url = state.result?.downloadUrl ?? state.result?.htmlUrl;
    if (!url) return;
    void import("@tauri-apps/plugin-opener").then(({ openUrl }) => openUrl(url)).catch(() => {});
  }, [state.result]);

  const dismiss = useCallback(() => {
    if (state.stage === "available" && state.version) writeStorage(dismissedKey, state.version);
    clearTimeout(hideTimer.current);
    setState(idleState);
  }, [state.stage, state.version]);

  return {
    state,
    checkForUpdates,
    startUpdate,
    dismiss,
    postUpdateNotice: null as PostUpdateNoticeState,
    dismissPostUpdateNotice: () => {},
  };
}
