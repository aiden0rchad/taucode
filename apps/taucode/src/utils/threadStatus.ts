export type ThreadStatusFlags = {
  lastTurn?: { status: string };
  activeFlags?: import("@protocol/v2/ThreadActiveFlag").ThreadActiveFlag[];
  isProcessing?: boolean;
  hasUnread?: boolean;
  isReviewing?: boolean;
};

export type ThreadStatusById = Record<string, ThreadStatusFlags>;

export type ThreadStatusClass = "processing" | "reviewing" | "unread" | "ready" | "failed" | "stopped";

export function getThreadStatusClass(
  status: ThreadStatusFlags | undefined,
  hasPendingUserInput: boolean,
): ThreadStatusClass {
  if (hasPendingUserInput || status?.activeFlags?.length) {
    return "unread";
  }
  if (!status?.isProcessing && status?.lastTurn?.status === "failed") return "failed";
  if (!status?.isProcessing && status?.lastTurn?.status === "interrupted") return "stopped";
  if (status?.isReviewing) {
    return "reviewing";
  }
  if (status?.isProcessing) {
    return "processing";
  }
  if (status?.hasUnread) {
    return "unread";
  }
  return "ready";
}

type WorkspaceHomeThreadState = {
  statusLabel: "Running" | "Reviewing" | "Idle" | "Waiting for you" | "Stopped" | "Failed";
  stateClass: "is-running" | "is-reviewing" | "is-idle" | "is-waiting" | "is-stopped" | "is-failed";
  isRunning: boolean;
};

export function getWorkspaceHomeThreadState(
  status: ThreadStatusFlags | undefined,
): WorkspaceHomeThreadState {
  if (status?.activeFlags?.length) return { statusLabel: "Waiting for you", stateClass: "is-waiting", isRunning: false };
  if (status?.isProcessing) {
    return { statusLabel: "Running", stateClass: "is-running", isRunning: true };
  }
  if (status?.lastTurn?.status === "failed") return { statusLabel: "Failed", stateClass: "is-failed", isRunning: false };
  if (status?.lastTurn?.status === "interrupted") return { statusLabel: "Stopped", stateClass: "is-stopped", isRunning: false };
  if (status?.isReviewing) {
    return { statusLabel: "Reviewing", stateClass: "is-reviewing", isRunning: false };
  }
  return { statusLabel: "Idle", stateClass: "is-idle", isRunning: false };
}
