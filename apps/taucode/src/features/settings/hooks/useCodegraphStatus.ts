import { useEffect, useState } from "react";
import { codegraphStatus, rebuildCodegraph, type CodegraphStatus } from "@services/tauri";
import { subscribeCodegraphStatus } from "@services/events";

export function useCodegraphStatus(workspaceId: string, enabled: boolean, override: boolean | null | undefined) {
  const [status, setStatus] = useState<CodegraphStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    let receivedEvent = false;
    const unsubscribe = subscribeCodegraphStatus((id, next) => {
      if (id === workspaceId) { receivedEvent = true; setStatus(next); }
    });
    codegraphStatus(workspaceId).then((next) => {
      if (active && !receivedEvent) setStatus(next);
    }).catch((reason: unknown) => { if (active) setError(String(reason)); });
    return () => { active = false; unsubscribe(); };
  }, [workspaceId, enabled, override]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try { await action(); } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }

  return {
    status, error, busy, run,
    rebuild: () => run(async () => { setStatus(await rebuildCodegraph(workspaceId)); }),
  };
}
