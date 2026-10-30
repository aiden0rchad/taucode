import { useEffect, useMemo, useRef, useState } from "react";
import type { AgentPolicy } from "@/types";
import { threadAgentPolicy, type ConversationPolicy } from "@services/tauri";

export function useConversationPolicy(workspaceId: string | null, threadId: string | null, connected: boolean, base: AgentPolicy) {
  const key = useMemo(() => workspaceId && threadId ? JSON.stringify([workspaceId, threadId, base]) : "", [workspaceId, threadId, base]);
  const current = useRef(key);
  current.current = key;
  const [state, setState] = useState<{ key: string; value?: ConversationPolicy; error?: string }>({ key: "" });
  useEffect(() => {
    if (!workspaceId || !threadId || !connected) return;
    let cancelled = false;
    setState({ key });
    void threadAgentPolicy(workspaceId, threadId, "get").then(
      value => { if (!cancelled) setState({ key, value }); },
      error => { if (!cancelled) setState({ key, error: String(error) }); },
    );
    return () => { cancelled = true; };
  }, [workspaceId, threadId, connected, key]);
  const value = state.key === key ? state.value : undefined;
  const error = state.key === key ? state.error : undefined;
  const change = async (policy?: AgentPolicy) => {
    if (!workspaceId || !threadId || !connected) throw new Error("Connect this project to edit its conversation team.");
    const value = await threadAgentPolicy(workspaceId, threadId, policy ? "set" : "reset", policy?.subagents);
    if (current.current === key) setState({ key, value });
  };
  return { policy: value?.policy ?? base, overridden: value?.overridden ?? false,
    loading: !!key && (!connected || !value && !error), error, change };
}
