import { useFeatures } from "@settings/features";
import { useEffect, useRef, useState } from "react";
import { checkpointRequest } from "@services/tauri";
import { subscribeAppServerEvents } from "@services/events";

export function useCheckpointWarnings(workspaceId: string | null, threadId: string | null, latestUserId?: string) {
  if (!useFeatures().checkpoints) workspaceId = null;
  const [warnings, setWarnings] = useState<Record<string, string>>({});
  const scope = useRef(`${workspaceId}:${threadId}`);
  useEffect(() => {
    const nextScope = `${workspaceId}:${threadId}`;
    if (scope.current !== nextScope) { scope.current = nextScope; setWarnings((previous) => Object.keys(previous).length ? {} : previous); }
    if (!workspaceId || !threadId) return;
    let cancelled = false;
    const load = () => void checkpointRequest(workspaceId, { threadId, action: "list" }).then((result) => {
      if (cancelled) return;
      setWarnings((previous) => ({ ...previous, ...Object.fromEntries((result.checkpoints ?? []).filter((row) => row.metadata.kind === "before" && row.warning).map((row) => [row.metadata.userItemId, row.warning!])) }));
    }).catch(() => {});
    load();
    const unsubscribe = subscribeAppServerEvents((event) => {
      const params = event.message.params as { threadId?: string; warning?: string } | undefined;
      if (event.workspace_id !== workspaceId || params?.threadId !== threadId || event.message.method !== "checkpoint/updated") return;
      if (params.warning && latestUserId) setWarnings((previous) => ({ ...previous, [latestUserId]: params.warning! }));
      load();
    });
    return () => { cancelled = true; unsubscribe(); };
  }, [workspaceId, threadId, latestUserId]);
  return warnings;
}
