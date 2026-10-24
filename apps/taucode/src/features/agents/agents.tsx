import { createContext, useContext } from "react";
import type { AgentNode, AgentPolicy, ConversationItem, CollabAgentStatus, ThreadSummary, ThreadTokenUsage } from "@/types";

export type AgentGroup = { id: string; label: string; nodes: AgentNode[] };

type ThreadDetails = Record<string, Partial<AgentNode>>;
export function agentThreadDetails(threads: ThreadSummary[], statuses: Record<string, { isProcessing: boolean; processingStartedAt: number | null; lastDurationMs: number | null; lastTurn?: { status: string; startedAt: number | null; completedAt: number | null; error: { message: string } | null } }>, usage: Record<string, ThreadTokenUsage>): ThreadDetails {
  return Object.fromEntries(threads.map(thread => {
    const state = statuses[thread.id];
    const tokens = usage[thread.id]?.total;
    const startedAt = state?.lastTurn?.startedAt ? state.lastTurn.startedAt * 1000 : state?.processingStartedAt ?? thread.createdAt;
    return [thread.id, { agent: thread.subagentNickname ?? thread.subagentRole ?? undefined, model: thread.modelId,
      startedAt, endedAt: state?.lastTurn?.completedAt ? state.lastTurn.completedAt * 1000 : state && !state.isProcessing && state.lastDurationMs != null && startedAt ? startedAt + state.lastDurationMs : null,
      ...(state?.isProcessing ? { status: "running" as const } : state?.lastTurn ? { status: (state.lastTurn.status === "failed" ? "failed" : state.lastTurn.status === "interrupted" ? "aborted" : "completed") as AgentNode["status"] } : {}),
      errorMessage: state?.lastTurn?.error?.message,
      ...(tokens ? { usage: { input: tokens.inputTokens, output: tokens.outputTokens, cacheRead: tokens.cachedInputTokens } } : {}),
    }];
  }));
}

export function agentGroups(items: ConversationItem[], byThread: Record<string, ConversationItem[]> = {}, seen = new Set<string>(), details: ThreadDetails = {}): AgentGroup[] {
  const groups: AgentGroup[] = [];
  let turn = 0;
  for (const item of items) {
    if (item.kind === "message" && item.role === "user") turn++;
    if (item.kind !== "tool") continue;
    const statuses: CollabAgentStatus[] = item.collabStatuses?.length ? item.collabStatuses : (item.collabReceivers ?? []).map(ref => ({ ...ref, status: "running" }));
    const nodes = item.agentsTree ?? statuses.filter(s => !seen.has(s.threadId)).map(s => {
      const visited = new Set(seen).add(s.threadId);
      const info = details[s.threadId];
      return {
        id: s.threadId, agent: info?.agent ?? s.nickname ?? s.role ?? "Agent", model: s.model ?? info?.model,
        status: info?.status ?? (["errored", "notFound"].includes(s.status) ? "failed" : ["shutdown", "interrupted"].includes(s.status) ? "aborted" : s.status === "pendingInit" ? "pending" : ["completed", "failed", "aborted", "pending"].includes(s.status) ? s.status : "running") as AgentNode["status"],
        startedAt: info?.startedAt, endedAt: info?.endedAt,
        usage: s.usage ?? info?.usage, lastMessage: s.message, errorMessage: info?.errorMessage ?? (s.status === "errored" ? s.message : undefined),
        children: agentGroups(byThread[s.threadId] ?? [], byThread, visited, details).flatMap(g => g.nodes),
      };
    });
    if (!nodes.length) continue;
    const id = item.turnId || String(turn);
    const group = groups.find(g => g.id === id);
    if (group) group.nodes.push(...nodes);
    else groups.push({ id, label: `Turn ${Math.max(1, turn)}`, nodes });
  }
  // Codex wait/send calls repeat receiver IDs. Keep the latest observed state.
  const ids = new Set<string>();
  return groups.reverse().map(g => ({ ...g, nodes: g.nodes.slice().reverse().filter(n => {
    if (ids.has(n.id)) return false;
    ids.add(n.id); return true;
  }).reverse() })).filter(g => g.nodes.length);
}

export function runningAgents(nodes: AgentNode[]): number {
  return nodes.reduce((n, node) => n + Number(node.status === "running") + runningAgents(node.children), 0);
}

export const AgentsContext = createContext<{
  workspaceId: string | null; threadId: string | null;
  itemsByThread: Record<string, ConversationItem[]>; policy?: AgentPolicy["subagents"];
  details?: ThreadDetails;
}>({ workspaceId: null, threadId: null, itemsByThread: {} });

export function useRunningAgents(threadId?: string) {
  const context = useContext(AgentsContext);
  const id = threadId ?? context.threadId ?? "";
  return runningAgents(agentGroups(context.itemsByThread[id] ?? [], context.itemsByThread, new Set([id]), context.details).flatMap(g => g.nodes));
}
