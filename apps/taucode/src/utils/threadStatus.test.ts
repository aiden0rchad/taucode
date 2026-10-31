import { describe, expect, it } from "vitest";

import { getThreadStatusClass, getWorkspaceHomeThreadState } from "./threadStatus";

describe("threadStatus", () => {
  it.each([["failed", "failed"], ["interrupted", "stopped"]])("keeps %s distinct from ready and unread", (turnStatus, expected) => {
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus }, hasUnread: true }, false)).toBe(expected);
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus }, isReviewing: true }, false)).toBe(expected);
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus }, isProcessing: true }, false)).toBe("processing");
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus } }, true)).toBe("unread");
  });
  it("keeps waiting and terminal states visible on the workspace home", () => {
    expect(getWorkspaceHomeThreadState({ isProcessing: true, activeFlags: ["waitingOnUserInput"] })).toEqual({ statusLabel: "Waiting for you", stateClass: "is-waiting", isRunning: false });
    expect(getWorkspaceHomeThreadState({ lastTurn: { status: "failed" }, isReviewing: true })).toEqual({ statusLabel: "Failed", stateClass: "is-failed", isRunning: false });
    expect(getWorkspaceHomeThreadState({ lastTurn: { status: "interrupted" }, isReviewing: true })).toEqual({ statusLabel: "Stopped", stateClass: "is-stopped", isRunning: false });
  });
  it("prioritizes pending user input over processing state", () => {
    expect(
      getThreadStatusClass(
        { isProcessing: true, hasUnread: false, isReviewing: false },
        true,
      ),
    ).toBe("unread");
  });

  it("maps thread status to workspace home labels and classes", () => {
    expect(
      getWorkspaceHomeThreadState({
        isProcessing: true,
        hasUnread: false,
        isReviewing: false,
      }),
    ).toEqual({
      statusLabel: "Running",
      stateClass: "is-running",
      isRunning: true,
    });

    expect(
      getWorkspaceHomeThreadState({
        isProcessing: false,
        hasUnread: false,
        isReviewing: true,
      }),
    ).toEqual({
      statusLabel: "Reviewing",
      stateClass: "is-reviewing",
      isRunning: false,
    });

    expect(
      getWorkspaceHomeThreadState({
        isProcessing: false,
        hasUnread: false,
        isReviewing: false,
      }),
    ).toEqual({
      statusLabel: "Idle",
      stateClass: "is-idle",
      isRunning: false,
    });
  });

  it("keeps running state when processing and reviewing are both true", () => {
    expect(
      getWorkspaceHomeThreadState({
        isProcessing: true,
        hasUnread: false,
        isReviewing: true,
      }),
    ).toEqual({
      statusLabel: "Running",
      stateClass: "is-running",
      isRunning: true,
    });
  });
});
