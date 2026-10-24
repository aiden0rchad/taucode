import type { ThreadSummary } from "@/types";
import {
  getThreadCreatedTimestamp,
  getThreadTimestamp,
} from "@utils/threadItems";
import { extractThreadCodexMetadata } from "@threads/utils/threadCodexMetadata";
import { asString } from "@threads/utils/threadNormalize";
import {
  getParentThreadIdFromThread,
  getSubagentMetadataFromThread,
  isSubagentThreadSource,
  shouldHideSubagentThreadFromSidebar,
} from "@threads/utils/threadRpc";
import { clampThreadName } from "@threads/utils/threadNaming";

type BuildThreadSummaryFromThreadOptions = {
  workspaceId: string;
  thread: Record<string, unknown>;
  fallbackIndex: number;
  getCustomName?: (workspaceId: string, threadId: string) => string | undefined;
};

export function extractEngineCapabilities(thread: Record<string, unknown> | null | undefined): Pick<ThreadSummary, "capabilities"> {
  if (thread?.engine !== "claude") return {};
  const capabilities = thread.capabilities as Record<string, unknown> | undefined;
  return { capabilities: { fork: capabilities?.fork === true, rewind: false, steer: capabilities?.steer === true } };
}

export function extractThreadFromResponse(
  response: Record<string, unknown> | null | undefined,
): Record<string, unknown> | null {
  if (!response || typeof response !== "object" || Array.isArray(response)) {
    return null;
  }
  const result =
    response.result && typeof response.result === "object" && !Array.isArray(response.result)
      ? (response.result as Record<string, unknown>)
      : null;
  const thread =
    (result?.thread as Record<string, unknown> | undefined) ??
    (response.thread as Record<string, unknown> | undefined);
  return thread ?? null;
}

export function buildThreadSummaryFromThread({
  workspaceId,
  thread,
  fallbackIndex,
  getCustomName,
}: BuildThreadSummaryFromThreadOptions): ThreadSummary | null {
  const id = String(thread?.id ?? "");
  if (!id) {
    return null;
  }
  const preview = asString(thread.preview ?? "").trim();
  const customName = getCustomName?.(workspaceId, id);
  const fallbackName = `Agent ${fallbackIndex + 1}`;
  const name = customName
    ? customName
    : clampThreadName(preview) ?? fallbackName;
  const metadata = extractThreadCodexMetadata(thread);
  if (shouldHideSubagentThreadFromSidebar(thread.source)) {
    return null;
  }
  const subagentMetadata = getSubagentMetadataFromThread(thread);
  const isSubagent = !thread.sideOf && (
    isSubagentThreadSource(thread.source) ||
    Boolean(getParentThreadIdFromThread(thread)) ||
    Boolean(subagentMetadata.nickname || subagentMetadata.role));
  return {
    id,
    name,
    ...(thread.sideOf ? { sideOf: thread.sideOf as ThreadSummary["sideOf"] } : {}),
    ...extractEngineCapabilities(thread),
    ...(thread.engine === "pi" || thread.engine === "codex" || thread.engine === "claude" ? { engine: thread.engine } : {}),
    ...(typeof thread.modelProvider === "string" ? { modelProvider: thread.modelProvider } : {}),
    updatedAt: getThreadTimestamp(thread),
    createdAt: getThreadCreatedTimestamp(thread),
    ...(metadata.modelId ? { modelId: metadata.modelId } : {}),
    ...(metadata.effort ? { effort: metadata.effort } : {}),
    ...(isSubagent ? { isSubagent: true } : {}),
    ...(isSubagent && subagentMetadata.nickname
      ? { subagentNickname: subagentMetadata.nickname }
      : {}),
    ...(isSubagent && subagentMetadata.role
      ? { subagentRole: subagentMetadata.role }
      : {}),
  };
}
