import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { recoveryCheckpoint } from "./recovery.ts";

// Recovery is display-only. Never append unfinished messages to model context.
export function registerStreamRecovery(pi: ExtensionAPI) {
  if (Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0) > 0) return;
  const checkpoints = new Map<string, ReturnType<typeof recoveryCheckpoint>>();
  let assistant: ReturnType<typeof recoveryCheckpoint> | undefined;
  let interrupted = false;
  const checkpoint = (session: string, key: string, identity: Record<string, string>) => {
    const id = `${session}:${key}`;
    let saved = checkpoints.get(id);
    if (!saved) { saved = recoveryCheckpoint(`${session}.streams`, key, identity); checkpoints.set(id, saved); }
    return saved;
  };
  pi.on("agent_start", () => { interrupted = false; });
  pi.on("message_start", (event, ctx) => {
    if (event.message.role !== "assistant") return;
    const session = ctx.sessionManager.getSessionFile(), anchor = ctx.sessionManager.getLeafId();
    assistant = session && anchor ? checkpoint(session, `message:${anchor}`, { anchor }) : undefined;
  });
  pi.on("message_update", event => {
    if (event.message.role !== "assistant") return;
    // Tool-call arguments are not executable evidence until the engine saves them.
    assistant?.update({ ...event.message, content: event.message.content.filter(part => part.type === "text" || part.type === "thinking") });
  });
  pi.on("message_end", event => {
    if (event.message.role !== "assistant") return;
    interrupted ||= event.message.stopReason === "aborted" || event.message.stopReason === "error";
    assistant?.flush(); assistant = undefined;
  });
  pi.on("tool_execution_update", (event, ctx) => {
    if (event.toolName === "subagent" || event.parentToolCallId) return;
    const session = ctx.sessionManager.getSessionFile();
    if (session) checkpoint(session, `tool:${event.toolCallId}`, { toolCallId: event.toolCallId, toolName: event.toolName }).update(event.partialResult);
  });
  pi.on("tool_execution_end", (event, ctx) => {
    interrupted ||= event.isError === true;
    const session = ctx.sessionManager.getSessionFile();
    checkpoints.get(`${session}:tool:${event.toolCallId}`)?.flush();
  });
  pi.on("agent_end", () => {
    for (const saved of checkpoints.values()) { if (interrupted) saved.flush(); else saved.discard(); }
    checkpoints.clear(); assistant = undefined;
  });
  pi.on("session_shutdown", () => { for (const saved of checkpoints.values()) saved.flush(); });
}
