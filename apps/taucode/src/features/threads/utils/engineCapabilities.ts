import type { EngineKind, ThreadSummary } from "@/types";

type EngineCapabilities = {
  forkLatest: boolean; forkAtItem: boolean; conversationRewind: boolean;
  sideChats: boolean; agentsTree: boolean; checkpoints: boolean; steer: boolean; compact: boolean;
};

// Keep the matrix documented in shared/engines.rs in sync. Codex rollback is
// additionally probed by the checkpoint service; Claude extensions are opt-in.
export const ENGINE_CAPABILITIES: Record<EngineKind, EngineCapabilities> = {
  pi: { forkLatest: true, forkAtItem: true, conversationRewind: true, sideChats: true, agentsTree: true, checkpoints: true, steer: true, compact: true },
  codex: { forkLatest: true, forkAtItem: false, conversationRewind: true, sideChats: true, agentsTree: true, checkpoints: true, steer: true, compact: true },
  claude: { forkLatest: false, forkAtItem: false, conversationRewind: false, sideChats: false, agentsTree: true, checkpoints: true, steer: false, compact: false },
};

export function engineCapabilities(engine: EngineKind, advertised?: ThreadSummary["capabilities"]): EngineCapabilities {
  const defaults = ENGINE_CAPABILITIES[engine];
  return engine === "claude" ? { ...defaults, forkLatest: advertised?.fork === true, sideChats: advertised?.fork === true, steer: advertised?.steer === true } : defaults;
}

export const CLAUDE_REWIND_REASON = "Conversation rewind isn't supported for Claude Code threads yet";

export function sideChatDisabledReason(engine: EngineKind, advertised?: ThreadSummary["capabilities"], atItem = false): string | undefined {
  const capabilities = engineCapabilities(engine, advertised);
  if (atItem && !capabilities.forkAtItem) return engine === "claude"
    ? "Message-level side chats aren't supported for Claude Code threads yet"
    : "Codex supports side chats at the latest point only. Use Side chat in the header.";
  if (!capabilities.sideChats) return "Claude Code adapter doesn't support forking sessions";
}
