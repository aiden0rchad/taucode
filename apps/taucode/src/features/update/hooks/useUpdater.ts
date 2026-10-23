import type { DebugEntry } from "../../../types";

type UpdateStage =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "restarting"
  | "latest"
  | "error";

type UpdateProgress = {
  totalBytes?: number;
  downloadedBytes: number;
};

export type UpdateState = {
  stage: UpdateStage;
  version?: string;
  progress?: UpdateProgress;
  error?: string;
};

type PostUpdateNotice =
  | {
      stage: "loading";
      version: string;
      htmlUrl: string;
    }
  | {
      stage: "ready";
      version: string;
      body: string;
      htmlUrl: string;
    }
  | {
      stage: "fallback";
      version: string;
      htmlUrl: string;
    };

export type PostUpdateNoticeState = PostUpdateNotice | null;

type UseUpdaterOptions = {
  enabled?: boolean;
  autoCheckOnMount?: boolean;
  onDebug?: (entry: DebugEntry) => void;
};

const noop = () => {};
const idleState: UpdateState = { stage: "idle" };
const checkForUpdates = async (_options?: { announceNoUpdate?: boolean }) => {};

// No release channel is configured for taucode. Never invoke the updater plugin.
export function useUpdater(_options: UseUpdaterOptions) {
  return {
    state: idleState,
    checkForUpdates,
    startUpdate: noop,
    dismiss: noop,
    postUpdateNotice: null as PostUpdateNoticeState,
    dismissPostUpdateNotice: noop,
  };
}
