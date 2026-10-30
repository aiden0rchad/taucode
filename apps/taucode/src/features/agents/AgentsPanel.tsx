import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import { ArrowLeft } from "lucide-react";
import { useContext, useEffect, useState } from "react";
import type { AgentNode, ConversationItem } from "@/types";
import { getSubagentTranscript } from "@services/tauri";
import { buildConversationItem } from "@utils/threadItems";
import { Messages } from "@/features/messages/components/Messages";
import { PanelShell } from "@/features/layout/components/PanelShell";
import type { PanelTabId } from "@/features/layout/components/PanelTabs";
import { agentGroups, AgentsContext, runningAgents } from "./agents";
import "./agents.css";

export function transcriptItems(messages: Record<string, unknown>[]): ConversationItem[] {
  return messages.flatMap((message, index): ConversationItem[] => {
    if (message.type) {
      const item = buildConversationItem(message);
      return item ? [item] : [];
    }
    const parts = Array.isArray(message.content) ? message.content as Record<string, unknown>[] : [];
    const text = typeof message.content === "string" ? message.content : parts.filter(p => p.type === "text").map(p => String(p.text ?? "")).join("\n");
    const id = String(message.id ?? index);
    if (message.role === "toolResult") return [{ id, kind: "tool", toolType: "dynamicToolCall", title: String(message.toolName ?? "Tool result"), detail: "", output: text, status: message.isError ? "failed" : "completed" }];
    return [
      ...(text ? [{ id, kind: "message" as const, role: message.role === "user" ? "user" as const : "assistant" as const, text }] : []),
      ...parts.filter(p => p.type === "toolCall").map(p => ({ id: String(p.id), kind: "tool" as const, toolType: "dynamicToolCall", title: String(p.name), detail: JSON.stringify(p.arguments, null, 2) })),
    ];
  });
}

export function AgentsPanel({ onSelect }: { onSelect: (tab: PanelTabId) => void }) {
  return <PanelShell filePanelMode="agents" onFilePanelModeChange={onSelect}><AgentActivity /></PanelShell>;
}

export function AgentActivity({ loadTranscript = getSubagentTranscript }: { loadTranscript?: typeof getSubagentTranscript }) {
  const settings = useFeatureSettings();
  const { workspaceId, threadId, itemsByThread, policy, details } = useContext(AgentsContext);
  const groups = agentGroups(itemsByThread[threadId ?? ""] ?? [], itemsByThread, new Set([threadId ?? ""]), details);
  const [selection, setSelection] = useState<{ node: AgentNode; workspaceId: string | null; threadId: string | null } | null>(null);
  const selected = selection?.workspaceId === workspaceId && selection?.threadId === threadId ? selection.node : null;
  const [transcript, setTranscript] = useState<{ items: ConversationItem[]; truncated: boolean; note?: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [now, setNow] = useState(Date.now);
  const running = runningAgents(groups.flatMap(g => g.nodes));
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running]);
  const findSelected = (nodes: AgentNode[]): AgentNode | undefined => {
    for (const node of nodes) { if (node.id === selected?.id) return node; const child = findSelected(node.children); if (child) return child; }
  };
  const selectedStatus = findSelected(groups.flatMap(g => g.nodes))?.status;
  useEffect(() => {
    if (!selected || !workspaceId || !threadId) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    setTranscript(null); setError(null);
    const refresh = async () => {
      try {
        const result = await loadTranscript(workspaceId, threadId, selected.id);
        const note = "note" in result && typeof result.note === "string" ? result.note : undefined;
        if (!cancelled) { setTranscript({ items: transcriptItems(result.messages), truncated: result.truncated, note }); setError(null); }
      } catch (e) { if (!cancelled) setError(String(e)); }
      if (!cancelled && (selectedStatus === "running" || selectedStatus === "pending")) timer = setTimeout(() => { void refresh(); }, document.hidden ? 10000 : 2000);
    };
    void refresh();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [selected, workspaceId, threadId, selectedStatus, loadTranscript, retry]);
  const renderNode = (node: AgentNode) => <li key={node.id}>
    <button className="agent-node" aria-label={`View ${node.agent || "agent"} transcript, ${node.status}`} onClick={() => setSelection({ node, workspaceId, threadId })}>
      <span className={`agent-status ${node.status}`} aria-label={node.status} />
      <span><strong>{node.agent || "Agent"}</strong> <small title={formatModelRef(settings, node.model).title}>{node.model ? formatModelRef(settings, node.model).name : ""}</small>
        <span className="agent-detail">{node.status} · {node.startedAt && (node.endedAt != null || node.status === "running") ? `${Math.max(0, Math.floor(((node.endedAt ?? now) - node.startedAt) / 1000))}s` : "elapsed unavailable"}</span>
        <span className="agent-detail">{node.usage ? `${(node.usage.input ?? 0) + (node.usage.output ?? 0) + (node.usage.cacheRead ?? 0) + (node.usage.cacheWrite ?? 0)} tokens · ${node.usage.cost == null ? "cost unavailable" : `$${node.usage.cost.toFixed(4)}`}` : "Usage unavailable"}</span>
        {node.task && <span className="agent-detail agent-preview" title={node.task}>{node.task}</span>}
        {node.lastMessage && <span className="agent-detail agent-preview" title={node.lastMessage}>{node.lastMessage}</span>}
        {node.status === "failed" && node.errorMessage && <span role="alert">{node.errorMessage}</span>}
      </span>
    </button>
    {!!node.children.length && <ul>{node.children.map(renderNode)}</ul>}
  </li>;
  return <div className="agents-panel" aria-label="Agent activity">
      {selected ? <>
        <button className="ghost" onClick={() => { setSelection(null); setError(null); }}><ArrowLeft aria-hidden />Back to agents</button>
        <h3>{selected.agent || "Agent"}</h3>
        {error && <div><p role="alert">{error}</p><button type="button" className="ghost" onClick={() => setRetry(value => value + 1)}>Retry transcript</button></div>}
        {!transcript && !error && <p role="status">Loading transcript…</p>}
        {transcript?.truncated && <p role="status">{transcript.note ?? "Showing the retained transcript (last 500 messages / 2 MB)."}</p>}
        {transcript && <Messages items={transcript.items} threadId={selected.id} isThinking={false} openTargets={[]} selectedOpenAppId="" />}
      </> : groups.length ? groups.map(g => <section key={g.id}><h3>{g.label}</h3><ul>{g.nodes.map(renderNode)}</ul></section>) : <>
        <p>No subagents in this thread yet</p>
        {policy && <p className="agent-detail">{policy.enabled ? "Enabled" : "Disabled"} · {policy.maxConcurrent} concurrent · {policy.maxPerTurn} per turn · depth {policy.maxDepth}<br />Allowed models: {policy.allowedModels.map(m => formatModelRef(settings, m).title).join(", ") || "All enabled models"}</p>}
      </>}
    </div>;
}
