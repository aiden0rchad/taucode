import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ServerNotification } from "@protocol/ServerNotification";
import type { ServerRequest } from "@protocol/ServerRequest";
import type { CommandExecutionRequestApprovalResponse } from "@protocol/v2/CommandExecutionRequestApprovalResponse";
import type { FileChangeRequestApprovalResponse } from "@protocol/v2/FileChangeRequestApprovalResponse";
import type { ThreadForkParams } from "@protocol/v2/ThreadForkParams";
import type { ThreadForkResponse } from "@protocol/v2/ThreadForkResponse";
import type { ThreadStartParams } from "@protocol/v2/ThreadStartParams";
import type { ThreadStartResponse } from "@protocol/v2/ThreadStartResponse";
import type { TurnStartResponse } from "@protocol/v2/TurnStartResponse";

export type { ServerNotification, ServerRequest };

export interface EngineStartInfo {
  running: boolean;
  home: string;
}

export interface EngineExit {
  code: number | null;
}

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

export type ApprovalResponse =
  | CommandExecutionRequestApprovalResponse
  | FileChangeRequestApprovalResponse;

export function engineStart(): Promise<EngineStartInfo> {
  return invoke("engine_start");
}

export function engineRequest<T>(method: string, params: unknown): Promise<T> {
  return invoke("engine_request", { method, params });
}

export function threadStart(
  params: Pick<ThreadStartParams, "cwd" | "model" | "modelProvider">,
): Promise<ThreadStartResponse> {
  return invoke("thread_start", params);
}

export function turnStart(
  threadId: string,
  text: string,
): Promise<TurnStartResponse> {
  return invoke("turn_start", { threadId, text });
}

export function turnInterrupt(
  threadId: string,
  turnId: string,
): Promise<unknown> {
  return invoke("turn_interrupt", { threadId, turnId });
}

export function threadFork(
  params: Pick<ThreadForkParams, "threadId" | "model" | "modelProvider">,
): Promise<ThreadForkResponse> {
  return invoke("thread_fork", params);
}

export function serverRequestRespond(
  id: ServerRequest["id"],
  result?: ApprovalResponse,
  error?: RpcError,
): Promise<null> {
  return invoke("server_request_respond", {
    id,
    result: result ?? null,
    error: error ?? null,
  });
}

export function onNotification(
  handler: (notification: ServerNotification) => void,
): Promise<UnlistenFn> {
  return listen<ServerNotification>("engine://notification", ({ payload }) =>
    handler(payload),
  );
}

export function onServerRequest(
  handler: (request: ServerRequest) => void,
): Promise<UnlistenFn> {
  return listen<ServerRequest>("engine://server-request", ({ payload }) =>
    handler(payload),
  );
}

export function onStderr(
  handler: (line: string) => void,
): Promise<UnlistenFn> {
  return listen<string>("engine://stderr", ({ payload }) => handler(payload));
}

export function onExit(
  handler: (exit: EngineExit) => void,
): Promise<UnlistenFn> {
  return listen<EngineExit>("engine://exit", ({ payload }) => handler(payload));
}
