import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import { IconButton } from "@/features/design-system/components/IconButton";
import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import { Bot, GitFork, Ellipsis } from "lucide-react";
import { useContext } from "react";
import { SidebarEngine, SidebarProjects } from "./SidebarTree";
import { useRunningAgents } from "@/features/agents/agents";
import type { CSSProperties, MouseEvent } from "react";

import type { ThreadSummary } from "../../../types";
import { getThreadStatusClass, type ThreadStatusById } from "../../../utils/threadStatus";

function hashString(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }
  return hash;
}

function getSubagentPillToneStyle(
  workspaceId: string,
  nickname: string | null | undefined,
  role: string | null | undefined,
  threadId: string,
) {
  const identity = [workspaceId, nickname ?? role ?? threadId].join(":");
  const hash = hashString(identity);
  const hue = hash % 360;
  const saturation = 68 + (hash % 12);
  const accent = 52 + ((hash >> 3) % 10);
  return {
    "--thread-subagent-pill-hue": `${hue}`,
    "--thread-subagent-pill-saturation": `${saturation}%`,
    "--thread-subagent-pill-accent": `${accent}%`,
  } as CSSProperties;
}

function formatSubagentRoleLabel(role: string | null | undefined) {
  const normalized = (role ?? "").trim();
  if (!normalized) {
    return null;
  }
  return normalized
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .replace(/\b\w/g, (match) => match.toUpperCase());
}

type ThreadRowProps = {
  thread: ThreadSummary;
  depth: number;
  treeDepth?: number;
  workspaceId: string;
  indentUnit: number;
  activeWorkspaceId: string | null;
  activeThreadId: string | null;
  threadStatusById: ThreadStatusById;
  pendingUserInputKeys?: Set<string>;
  workspaceLabel?: string | null;
  getThreadTime: (thread: ThreadSummary) => string | null;
  getThreadArgsBadge?: (workspaceId: string, threadId: string) => string | null;
  isThreadPinned: (workspaceId: string, threadId: string) => boolean;
  onSelectThread: (workspaceId: string, threadId: string) => void;
  onShowThreadMenu: (
    event: MouseEvent,
    workspaceId: string,
    threadId: string,
    canPin: boolean,
  ) => void;
  hasSubagentChildren?: boolean;
  subagentsExpanded?: boolean;
  onToggleSubagents?: (workspaceId: string, threadId: string) => void;
  showPinnedLabel?: boolean;
};

export function ThreadRow({
  thread,
  depth,
  treeDepth = depth,
  workspaceId,
  indentUnit,
  activeWorkspaceId,
  activeThreadId,
  threadStatusById,
  pendingUserInputKeys,
  workspaceLabel,
  getThreadTime,
  getThreadArgsBadge,
  isThreadPinned,
  onSelectThread,
  onShowThreadMenu,
  hasSubagentChildren = false,
  subagentsExpanded = true,
  onToggleSubagents,
  showPinnedLabel = true,
}: ThreadRowProps) {
  const settings = useFeatureSettings();
  const inheritedEngine = useContext(SidebarEngine);
  const defaultEngine = useContext(SidebarProjects).find(workspace => workspace.id === workspaceId)?.settings.agentPolicy?.defaultEngine ?? inheritedEngine;
  const runningAgents = useRunningAgents(thread.id);
  const relativeTime = getThreadTime(thread);
  const badge = getThreadArgsBadge?.(workspaceId, thread.id) ?? null;
  const modelBadge =
    thread.modelId && thread.modelId.trim().length > 0
      ? thread.effort && thread.effort.trim().length > 0
        ? `${formatModelRef(settings, thread.modelId).name} · ${thread.effort}`
        : formatModelRef(settings, thread.modelId).name
      : null;
  const indentStyle =
    depth > 0
      ? ({ "--thread-indent": `${depth * indentUnit}px` } as CSSProperties)
      : undefined;
  const hasPendingUserInput = Boolean(
    pendingUserInputKeys?.has(`${workspaceId}:${thread.id}`),
  );
  const needsInput = hasPendingUserInput || Boolean(threadStatusById[thread.id]?.activeFlags?.some(flag => flag === "waitingOnApproval" || flag === "waitingOnUserInput"));
  const statusClass = getThreadStatusClass(
    threadStatusById[thread.id],
    hasPendingUserInput,
  );
  const statusLabel =
    threadStatusById[thread.id]?.activeFlags?.includes("waitingOnApproval")
      ? "Waiting for approval"
      : threadStatusById[thread.id]?.activeFlags?.includes("waitingOnUserInput")
        ? "Waiting for input"
        : statusClass === "reviewing"
      ? "Reviewing"
      : hasPendingUserInput
        ? "Waiting"
        : null;
  const subagentLabel =
    thread.isSubagent && (thread.subagentNickname || thread.subagentRole)
      ? thread.subagentNickname ?? thread.subagentRole ?? null
      : null;
  const subagentTitle =
    thread.subagentNickname && thread.subagentRole
      ? `${thread.subagentNickname} · ${thread.subagentRole}`
      : subagentLabel;
  const subagentRoleLabel =
    thread.subagentNickname && thread.subagentRole
      ? formatSubagentRoleLabel(thread.subagentRole)
      : null;
  const subagentPillStyle = subagentLabel
    ? getSubagentPillToneStyle(
        workspaceId,
        thread.subagentNickname,
        thread.subagentRole,
        thread.id,
      )
    : undefined;
  const effectiveWorkspaceLabel = depth > 0 ? null : workspaceLabel;
  const contextLabel = modelBadge ?? badge;
  const canPin = depth === 0;
  const isPinned = canPin && isThreadPinned(workspaceId, thread.id);
  const canToggleSubagents = hasSubagentChildren && Boolean(onToggleSubagents);
  const hasDetails = Boolean(
    effectiveWorkspaceLabel || subagentLabel || contextLabel || statusLabel || isPinned,
  );

  return (
    <div
      className={`thread-row ${
        workspaceId === activeWorkspaceId && thread.id === activeThreadId
          ? "active"
          : ""
      }${hasDetails ? " has-details" : ""}${
        hasDetails ? " has-secondary-line" : ""
      }${canToggleSubagents ? " has-subagent-children" : ""}${
        depth > 0 ? " is-nested" : ""
      }${isPinned ? " is-pinned" : ""}`}
      style={indentStyle}
      data-depth={treeDepth}
      data-needs-input={needsInput || undefined}
      title={[thread.name, thread.engine ?? "codex", modelBadge, badge, statusLabel, effectiveWorkspaceLabel].filter(Boolean).join(" · ")}
      onClick={() => onSelectThread(workspaceId, thread.id)}
      onContextMenu={(event) => onShowThreadMenu(event, workspaceId, thread.id, canPin)}
      role="button"
      aria-label={thread.name}
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) {
          event.preventDefault();
          onSelectThread(workspaceId, thread.id);
        }
      }}
    >
      <span className="thread-kind">{canToggleSubagents && <IconButton size="sm" type="button" className="thread-tree-toggle" aria-label={subagentsExpanded ? "Hide sub-agents" : "Show sub-agents"} aria-expanded={subagentsExpanded} onClick={event => { event.stopPropagation(); onToggleSubagents?.(workspaceId, thread.id); }}><ChevronRight size={14} aria-hidden /></IconButton>}{!needsInput && statusClass !== "processing" && thread.isSubagent ? <Bot size={14} aria-hidden /> : !needsInput && statusClass !== "processing" && thread.sideOf ? <GitFork size={14} aria-label="Side chat" /> : <span className={`thread-status ${statusClass}`} aria-hidden />}</span>
      <div className="thread-content">
        <div className="thread-headline">
          <span className="thread-name">{thread.name}</span>{runningAgents > 0 && <span className="agents-count" aria-label={`${runningAgents} running subagents`}>{runningAgents}</span>}
        </div>
        {hasDetails && (
          <div className="thread-details">
            {effectiveWorkspaceLabel && (
              <span className="thread-workspace-label" title={effectiveWorkspaceLabel}>
                {effectiveWorkspaceLabel}
              </span>
            )}
            {subagentLabel && (
              <span
                className="thread-subagent-pill"
                title={subagentTitle ?? undefined}
                style={subagentPillStyle}
              >
                {subagentLabel}
              </span>
            )}
            {subagentRoleLabel && (
              <span className="thread-subagent-role" title={thread.subagentRole ?? undefined}>
                {subagentRoleLabel}
              </span>
            )}
            {statusLabel && (
              <span className={`thread-state-chip ${statusClass}`}>{statusLabel}</span>
            )}
            {contextLabel && (
              <span className="thread-context-label" title={thread.modelId ? formatModelRef(settings, thread.modelId).title : contextLabel}>
                {contextLabel}
              </span>
            )}
            {showPinnedLabel && isPinned && <span className="thread-pinned-label">Pinned</span>}
          </div>
        )}
      </div>
      <div className="thread-meta">
        {(thread.engine ?? "codex") !== defaultEngine && <span className={`thread-engine-dot engine-${thread.engine ?? "codex"}`} title={thread.engine ?? "codex"} />}
        <span className="thread-time">{relativeTime ?? "Now"}</span>
        <IconButton size="sm" type="button" className="ghost thread-menu-trigger" aria-label={`Actions for ${thread.name}`} onClick={event => { event.stopPropagation(); onShowThreadMenu(event, workspaceId, thread.id, canPin); }}><Ellipsis size={14} aria-hidden /></IconButton>
      </div>
    </div>
  );
}
