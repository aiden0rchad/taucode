import { useEffect, useMemo, useState } from "react";
import type { ConversationItem, McpServerEntry, ThreadTokenUsage } from "@/types";
import { connectorRequest } from "@services/tauri";
import { parseInlineFileTarget, resolveMessageFileHref } from "@/features/messages/utils/messageFileLinks";

export function reportedFiles(items: ConversationItem[], workspacePath?: string) {
  return [...new Set(items.flatMap(item => {
    if (item.kind === "tool" && !/failed|error|denied|declined|in.?progress|running|pending/i.test(item.status ?? "")) {
      return (item.changes ?? []).filter(change => !/^deleted?$/i.test(change.kind ?? "")).map(change => change.path);
    }
    if (item.kind === "message" && item.role === "assistant") {
      return [...item.text.matchAll(/\[[^\]]*\]\((?:<([^>]+)>|([^\s)]+))(?:\s+"[^"]*")?\)/g)]
        .map(match => {
          const href = match[1] ?? match[2];
          return (resolveMessageFileHref(href, workspacePath) ?? (!/^[a-z][a-z\d+.-]*:|[?#]/i.test(href) ? parseInlineFileTarget(href.includes("/") ? href : `./${href}`) : null))?.path.replace(/^\.\//, "");
        }).filter((path): path is string => !!path);
    }
    return [];
  }).filter(path => path.trim() && !/^[a-z][a-z\d+.-]*:\/\//i.test(path)))];
}

export function contextUsage(usage: ThreadTokenUsage | null, protocol: string) {
  if (!usage) return null;
  const last = usage.last;
  const detailed = protocol !== "acp";
  // PI reports cache reads separately; Codex includes them in inputTokens.
  const cached = Math.max(0, last.cachedInputTokens);
  const input = Math.max(0, last.inputTokens - (protocol === "codex-app-server" ? cached : 0));
  const output = Math.max(0, last.outputTokens);
  const used = Math.max(0, last.totalTokens || (detailed ? input + cached + output : 0));
  const capacity = usage.modelContextWindow && usage.modelContextWindow > 0 ? usage.modelContextWindow : null;
  return { detailed, input, cached, output, other: Math.max(0, used - input - cached - output), reasoning: Math.max(0, last.reasoningOutputTokens), used, capacity,
    percent: capacity ? Math.min(100, Math.round(used / capacity * 100)) : null };
}

export function useCodeSessionData(workspaceId: string | null, connectorRevision: string, items: ConversationItem[], workspacePath?: string) {
  const [state, setState] = useState<{ workspaceId: string | null; entries: McpServerEntry[]; error: string | null; loading: boolean }>({ workspaceId: null, entries: [], error: null, loading: false });
  useEffect(() => {
    let active = true;
    setState({ workspaceId, entries: [], error: null, loading: !!workspaceId });
    if (workspaceId) void connectorRequest<McpServerEntry[]>({ action: "list", workspaceId }).then(entries => {
      if (active) setState({ workspaceId, entries, error: null, loading: false });
    }).catch(error => {
      if (active) setState({ workspaceId, entries: [], error: String(error), loading: false });
    });
    return () => { active = false; };
  }, [workspaceId, connectorRevision]);
  return {
    connectors: state.workspaceId === workspaceId ? state.entries.filter(entry => entry.enabled) : [],
    connectorError: state.workspaceId === workspaceId ? state.error : null,
    connectorsLoading: state.workspaceId !== workspaceId || state.loading,
    outputs: useMemo(() => reportedFiles(items, workspacePath), [items, workspacePath]),
  };
}
