import { describe, expect, it } from "vitest";

import { getThreadStatusClass, getWorkspaceHomeThreadState } from "./threadStatus";

describe("threadStatus", () => {
  it.each([["failed", "failed"], ["interrupted", "stopped"]])("keeps %s distinct from ready and unread", (turnStatus, expected) => {
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus }, hasUnread: true }, false)).toBe(expected);
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus }, isProcessing: true }, false)).toBe("processing");
    expect(getThreadStatusClass({ lastTurn: { status: turnStatus } }, true)).toBe("unread");
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
