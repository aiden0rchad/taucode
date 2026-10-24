import { describe, expect, it } from "vitest";
import { ENGINE_CAPABILITIES, engineCapabilities, sideChatDisabledReason } from "./engineCapabilities";
import { extractEngineCapabilities } from "./threadSummary";

describe("engine capability matrix", () => {
  it("requires advertised Claude extensions and never permits item forks or rewind", () => {
    const advertised = extractEngineCapabilities({ engine: "claude", capabilities: { fork: true, rewind: true, steer: true } }).capabilities;
    expect(engineCapabilities("claude", advertised)).toEqual({ forkLatest: true, forkAtItem: false, conversationRewind: false, sideChats: true, agentsTree: true, checkpoints: true, steer: true, compact: false });
    expect(engineCapabilities("claude")).toEqual(ENGINE_CAPABILITIES.claude);
    expect(sideChatDisabledReason("claude")).toBe("Claude Code adapter doesn't support forking sessions");
    expect(sideChatDisabledReason("claude", advertised)).toBeUndefined();
    expect(sideChatDisabledReason("claude", advertised, true)).toBe("Message-level side chats aren't supported for Claude Code threads yet");
  });
  it("preserves Pi item forks and Codex latest-only forks", () => {
    expect(sideChatDisabledReason("pi", undefined, true)).toBeUndefined();
    expect(sideChatDisabledReason("codex")).toBeUndefined();
    expect(sideChatDisabledReason("codex", undefined, true)).toContain("latest point only");
  });
});
