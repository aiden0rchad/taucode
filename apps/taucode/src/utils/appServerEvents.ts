import type { AppServerEvent } from "../types";
import type { ServerNotification } from "@protocol/ServerNotification";

// These features have no UI in taucode. Keep their traffic out of error handling.
export const IGNORED_APP_SERVER_METHODS = [
  "turn/moderationMetadata", "thread/goal/updated", "thread/goal/cleared",
  "remoteControl/status/changed", "mcpServer/startupStatus/updated",
] as const satisfies readonly ServerNotification["method"][];

export const THREAD_METADATA_METHODS = [
  "serverRequest/resolved", "thread/readState/changed", "deprecationNotice",
] as const satisfies readonly ServerNotification["method"][];

export const SUPPORTED_APP_SERVER_METHODS = [
  ...IGNORED_APP_SERVER_METHODS,
  ...THREAD_METADATA_METHODS,
  "app/list/updated",
  "account/login/completed",
  "account/rateLimits/updated",
  "account/updated",
  "codex/backgroundThread",
  "codex/connected",
  "codex/event/skills_update_available",
  "error",
  "hook/completed",
  "hook/started",
  "item/agentMessage/delta",
  "item/commandExecution/outputDelta",
  "item/commandExecution/terminalInteraction",
  "item/completed",
  "item/fileChange/outputDelta",
  "item/plan/delta",
  "item/reasoning/summaryPartAdded",
  "item/reasoning/summaryTextDelta",
  "item/reasoning/textDelta",
  "item/started",
  "item/tool/requestUserInput",
  "thread/archived",
  "thread/closed",
  "thread/name/updated",
  "thread/status/changed",
  "thread/started",
  "thread/tokenUsage/updated",
  "thread/unarchived",
  "turn/completed",
  "turn/diff/updated",
  "turn/plan/updated",
  "turn/started",
] as const;

export type SupportedAppServerMethod = (typeof SUPPORTED_APP_SERVER_METHODS)[number];

export const METHODS_HANDLED_OUTSIDE_USE_APP_SERVER_EVENTS = [
  ...IGNORED_APP_SERVER_METHODS,
  ...THREAD_METADATA_METHODS,
  "app/list/updated",
  "codex/event/skills_update_available",
] as const satisfies readonly SupportedAppServerMethod[];

const SUPPORTED_METHOD_SET = new Set<string>(SUPPORTED_APP_SERVER_METHODS);

function getAppServerMessageObject(
  event: AppServerEvent,
): Record<string, unknown> | null {
  if (!event || typeof event !== "object") {
    return null;
  }
  const message = (event as { message?: unknown }).message;
  if (!message || typeof message !== "object" || Array.isArray(message)) {
    return null;
  }
  return message as Record<string, unknown>;
}

export function getAppServerRawMethod(event: AppServerEvent): string | null {
  const message = getAppServerMessageObject(event);
  if (!message) {
    return null;
  }
  const method = message.method;
  if (typeof method !== "string") {
    return null;
  }
  const trimmed = method.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export function isSupportedAppServerMethod(
  method: string,
): method is SupportedAppServerMethod {
  return SUPPORTED_METHOD_SET.has(method);
}

export function getAppServerParams(event: AppServerEvent): Record<string, unknown> {
  const message = getAppServerMessageObject(event);
  if (!message) {
    return {};
  }
  const params = message.params;
  if (!params || typeof params !== "object" || Array.isArray(params)) {
    return {};
  }
  return params as Record<string, unknown>;
}

export function getAppServerRequestId(event: AppServerEvent): string | number | null {
  const message = getAppServerMessageObject(event);
  if (!message) {
    return null;
  }
  const requestId = message.id;
  if (typeof requestId === "number" || typeof requestId === "string") {
    return requestId;
  }
  return null;
}

export function isApprovalRequestMethod(method: string): boolean {
  return method.endsWith("requestApproval");
}

export function isSkillsUpdateAvailableEvent(event: AppServerEvent): boolean {
  return getAppServerRawMethod(event) === "codex/event/skills_update_available";
}

export function isAppListUpdatedEvent(event: AppServerEvent): boolean {
  return getAppServerRawMethod(event) === "app/list/updated";
}
