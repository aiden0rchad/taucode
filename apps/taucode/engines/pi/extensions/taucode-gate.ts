import { readFileSync, writeFileSync, renameSync } from "node:fs";
import { join } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { decide, normalizeRule, safeReadOnlyCommand, type Access } from "./lib/policy.ts";
import { connectorDecision, type Connector } from "./lib/connectors.ts";

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (_event, ctx) => {
    if (process.env.TAUCODE_OMP === "1") {
      const features = JSON.parse(process.env.TAUCODE_FEATURES || "{}");
      await pi.setActiveTools(pi.getActiveTools().filter(name => ["read", "bash", "edit", "write", "grep", "glob", "lsp", "debug", "eval"].includes(name) || (name === "subagent" && features.subagents !== false) || (name.startsWith("mcp__") && features.connectors !== false)));
    }
    ctx.ui.setStatus("taucode-gate", "ready");
  });
  pi.on("tool_call", async (event, ctx) => {
    if (process.env.TAUCODE_OMP === "1") {
      if (["task", "browser", "computer", "login"].includes(event.toolName)) return { block: true, reason: "Use taucode-managed tools and subagents" };
      if (event.toolName.startsWith("mcp__") && !event.toolName.slice(5).includes("__")) {
        const connectors = JSON.parse(readFileSync(join(process.env.PI_CODING_AGENT_DIR!, "taucode/connectors.json"), "utf8")) as Connector[];
        const connector = connectors.sort((a, b) => b.id.length - a.id.length).find(c => event.toolName.startsWith(`mcp__${c.id.replace(/-/g, "_")}_`));
        if (!connector) return { block: true, reason: "Unknown connector" };
        const prefix = `mcp__${connector.id.replace(/-/g, "_")}_`;
        event = { ...event, toolName: prefix.slice(0, -1) + "__" + event.toolName.slice(prefix.length) };
      }
      if (!["read", "grep", "find", "glob", "bash", "write", "edit", "subagent"].includes(event.toolName) && !event.toolName.startsWith("mcp__")) {
        const home = process.env.PI_CODING_AGENT_DIR!, session = process.env.TAUCODE_SESSION_KEY!;
        const access = JSON.parse(readFileSync(join(home, "taucode/threads", `${session}.json`), "utf8")) as Access;
        if (access.accessMode === "read-only" || process.env.TAUCODE_SUBAGENT_DEPTH || !ctx.hasUI) return { block: true, reason: "Native tool requires interactive approval" };
        const title = "taucode:approval:" + JSON.stringify({ toolCallId: event.toolCallId, tool: event.toolName, command: event.toolName, cwd: ctx.cwd });
        if (await ctx.ui.select(title, ["Allow", "Block"]) !== "Allow") return { block: true, reason: "Native tool denied" };
        return;
      }
    }
    if (process.env.TAUCODE_CHAT === "1" && !(process.env.TAUCODE_CHAT_WEB === "1" && ["mcp__taucode_browser__browser_navigate", "mcp__taucode_browser__browser_snapshot", "mcp__taucode_browser__browser_take_screenshot"].includes(event.toolName))) return { block: true, reason: "Tools are disabled in Chat mode" };
    // OMP dispatches this write through the inner MCP tool's tool_call hook.
    // Apply connector policy there once, rather than prompting for a filesystem write first.
    if (process.env.TAUCODE_OMP === "1" && event.toolName === "write" && typeof event.input?.path === "string" && event.input.path.startsWith("xd://mcp__")) return;
    if (!["bash", "write", "edit", ...(process.env.TAUCODE_WORK === "1" ? ["read", "ls", "find", "grep"] : [])].includes(event.toolName) && !event.toolName.startsWith("mcp__")) return;
    const home = process.env.PI_CODING_AGENT_DIR!;
    const session = process.env.TAUCODE_SESSION_KEY;
    if (!home || !session || !/^[\w-]+$/.test(session)) return { block: true, reason: "Missing taucode thread access policy" };
    const file = join(home, "taucode", "threads", `${session}.json`);
    const read = () => JSON.parse(readFileSync(file, "utf8")) as Access;
    if (event.toolName.startsWith("mcp__")) {
      const decideMcp = () => {
        try {
          return connectorDecision(JSON.parse(readFileSync(join(home, "taucode/connectors.json"), "utf8")) as Connector[], event.toolName, process.env.TAUCODE_SUBAGENT_DEPTH && process.env.TAUCODE_TOOL_ACCESS === "read-only" ? "read-only" : read().accessMode, ctx.hasUI && !process.env.TAUCODE_SUBAGENT_DEPTH);
        } catch { return "block"; }
      };
      const decision = decideMcp();
      if (decision === "allow") return;
      if (decision === "block") return { block: true, reason: "Connector tool blocked by taucode policy" };
      const title = "taucode:approval:" + JSON.stringify({ toolCallId: event.toolCallId, tool: event.toolName, command: event.toolName, cwd: ctx.cwd });
      const answer = await ctx.ui.select(title, ["Allow", "Block"]);
      if (answer !== "Allow" || decideMcp() === "block") return { block: true, reason: "Connector call denied" };
      return;
    }
    const input = event.input as Record<string, unknown>;
    const call = { tool: event.toolName, cwd: ctx.cwd, command: input.command as string, path: input.path as string };
    const child = process.env.TAUCODE_SUBAGENT_DEPTH ? { workspace: process.env.TAUCODE_WORKSPACE!, toolAccess: process.env.TAUCODE_TOOL_ACCESS ?? "read-only" } : undefined;
    const decision = decide(read(), call, home, ctx.hasUI, child);
    if (decision === "allow") {
      if (call.tool === "bash") input.command = safeReadOnlyCommand(call.command);
      return;
    }
    if (decision === "block") return { block: true, reason: "Blocked by taucode access policy (read-only, protected path, or no interactive approval)" };
    const title = "taucode:approval:" + JSON.stringify({ toolCallId: event.toolCallId, ...call, diffPreview: call.tool === "edit" ? JSON.stringify(event.input).slice(0, 4000) : undefined });
    const answer = await ctx.ui.select(title, read().kind === "work" ? ["Allow", "Block"] : ["Allow", "Allow for session", "Block"]);
    // A mode change while the dialog was open must take effect before execution.
    if (decide(read(), call, home, ctx.hasUI, child) === "block") return { block: true, reason: "Thread access changed while awaiting approval" };
    if (answer === "Allow for session") {
      const access = read();
      const rule = normalizeRule(call);
      if (!access.sessionAllow.some(r => JSON.stringify(r) === JSON.stringify(rule))) access.sessionAllow.push(rule);
      const temp = `${file}.${process.pid}.tmp`;
      writeFileSync(temp, JSON.stringify(access));
      renameSync(temp, file);
    }
    if (answer !== "Allow" && answer !== "Allow for session") return { block: true, reason: "Blocked by user" };
  });
}
