import { useEffect, useMemo } from "react";
import type { ApprovalRequest, WorkspaceInfo } from "../../../types";
import { getApprovalCommandInfo } from "../../../utils/approvalRules";
import {
  ToastActions,
  ToastBody,
  ToastCard,
  ToastError,
  ToastHeader,
  ToastTitle,
  ToastViewport,
} from "../../design-system/components/toast/ToastPrimitives";

type ApprovalToastsProps = {
  approvals: ApprovalRequest[];
  workspaces: WorkspaceInfo[];
  onDecision: (request: ApprovalRequest, decision: "accept" | "acceptForSession" | "decline") => void;
  onRemember?: (request: ApprovalRequest, command: string[]) => void;
};

export function ApprovalToasts({
  approvals,
  workspaces,
  onDecision,
  onRemember,
}: ApprovalToastsProps) {
  const workspaceLabels = useMemo(
    () => new Map(workspaces.map((workspace) => [workspace.id, workspace.name])),
    [workspaces],
  );

  const primaryRequest = approvals[approvals.length - 1];

  useEffect(() => {
    if (!primaryRequest) {
      return;
    }

    const handler = (event: KeyboardEvent) => {
      if (event.key !== "Enter") {
        return;
      }
      const active = document.activeElement;
      if (
        active instanceof HTMLElement &&
        (active.isContentEditable ||
          active.tagName === "INPUT" ||
          active.tagName === "TEXTAREA" ||
          active.tagName === "SELECT")
      ) {
        return;
      }
      event.preventDefault();
      onDecision(primaryRequest, "accept");
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onDecision, primaryRequest]);

  if (!approvals.length) {
    return null;
  }

  const formatLabel = (value: string) =>
    value
      .replace(/([a-z])([A-Z])/g, "$1 $2")
      .replace(/_/g, " ")
      .trim();

  const hasValue = (value: unknown) =>
    value !== null && value !== undefined && value !== "None" && value !== "";

  const renderParamValue = (value: unknown) => {
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return { text: String(value), isCode: false };
    }
    if (Array.isArray(value)) {
      if (value.every((entry) => ["string", "number", "boolean"].includes(typeof entry))) {
        return { text: value.map(String).join(", "), isCode: false };
      }
      return { text: JSON.stringify(value, null, 2), isCode: true };
    }
    return { text: JSON.stringify(value, null, 2), isCode: true };
  };

  return (
    <ToastViewport className="approval-toasts" role="region" ariaLive="assertive">
      {approvals.map((request) => {
        const workspaceName = workspaceLabels.get(request.workspace_id);
        const params = request.params ?? {};
        const commandInfo = getApprovalCommandInfo(params);
        const reason = typeof params.reason === "string" && hasValue(params.reason) ? params.reason : null;
        const command = typeof params.command === "string" && hasValue(params.command)
          ? params.command : params.command === "None" ? null : commandInfo?.preview;
        const path = typeof params.path === "string" && hasValue(params.path) ? params.path : null;
        const tool = typeof params.tool === "string" && hasValue(params.tool) ? params.tool : null;
        const connector = reason?.startsWith("Connector requests permission") ||
          tool?.startsWith("mcp__") || command?.startsWith("mcp__") || path?.startsWith("xd://mcp__");
        const title = connector ? "Use a connector tool"
          : request.method.includes("fileChange") || path ? "Change a file" : "Run a command";
        const entries = Object.entries(params).filter(([key, value]) =>
          !["approvalId", "itemId", "threadId", "turnId", "availableDecisions", "grantRoot",
            "reason", "command", "path", "tool", "cwd", "diffPreview"].includes(key) && hasValue(value),
        );
        return (
          <ToastCard
            key={`${request.workspace_id}-${request.request_id}`}
            className="approval-toast"
            role="alert"
          >
            <ToastHeader className="approval-toast-header">
              <ToastTitle className="approval-toast-title">{title}</ToastTitle>
              {workspaceName ? (
                <div className="approval-toast-workspace">{workspaceName}</div>
              ) : null}
            </ToastHeader>
            <div className="approval-toast-details">
              {reason && <ToastBody>{reason}</ToastBody>}
              {[...new Set([command, path, tool].filter(Boolean))].map((value) => (
                <code key={value} className="approval-toast-chip">{value}</code>
              ))}
              {typeof params.cwd === "string" && hasValue(params.cwd) && (
                <div className="approval-toast-cwd">{params.cwd}</div>
              )}
              {hasValue(params.diffPreview) && (
                <ToastError className="approval-toast-detail-code">
                  {renderParamValue(params.diffPreview).text}
                </ToastError>
              )}
              {entries.length > 0 && (
                <details className="approval-toast-more">
                  <summary>More details</summary>
                  {entries.map(([key, value]) => {
                    const rendered = renderParamValue(value);
                    return (
                      <div key={key} className="approval-toast-detail">
                        <div className="approval-toast-detail-label">
                          {formatLabel(key)}
                        </div>
                        {rendered.isCode ? (
                          <ToastError className="approval-toast-detail-code">
                            {rendered.text}
                          </ToastError>
                        ) : (
                          <ToastBody className="approval-toast-detail-value">
                            {rendered.text}
                          </ToastBody>
                        )}
                      </div>
                    );
                  })}
                </details>
              )}
            </div>
            <ToastActions className="approval-toast-actions">
              <button
                className="secondary"
                onClick={() => onDecision(request, "decline")}
              >
                Decline
              </button>
              {Array.isArray(params.availableDecisions) && params.availableDecisions.includes("acceptForSession") && (
                <button className="secondary" onClick={() => onDecision(request, "acceptForSession")}>Allow for session</button>
              )}
              {commandInfo && onRemember && !String(request.request_id).startsWith("pi:") ? (
                <button
                  className="ghost approval-toast-remember"
                  onClick={() => onRemember(request, commandInfo.tokens)}
                  title={`Allow commands that start with ${commandInfo.preview}`}
                >
                  Always allow
                </button>
              ) : null}
              <button
                className="primary"
                onClick={() => onDecision(request, "accept")}
              >
                Approve (Enter)
              </button>
            </ToastActions>
          </ToastCard>
        );
      })}
    </ToastViewport>
  );
}
