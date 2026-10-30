import { mkdirSync, chmodSync } from "node:fs";
import { dirname } from "node:path";
import { DatabaseSync } from "node:sqlite";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import type { Policy } from "./policy.ts";

const day = (timestamp = Date.now()) => new Date(timestamp).toISOString().slice(0, 10);

// One project database is shared by Pi/OMP roots and descendants. SQLite owns
// cross-process locking; replayed response frames update rather than double count.
export class UsageLedger {
  private db: DatabaseSync;
  constructor(path: string) {
    mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
    this.db = new DatabaseSync(path);
    chmodSync(path, 0o600);
    this.db.exec("PRAGMA busy_timeout=5000; PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS usage (id TEXT PRIMARY KEY, day TEXT NOT NULL, tokens INTEGER NOT NULL, unknown INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS usage_day ON usage(day)");
  }
  record(id: string, usage: { input?: unknown; output?: unknown; cacheRead?: unknown; cacheWrite?: unknown } | undefined, timestamp = Date.now(), failed = false) {
    const values = [usage?.input, usage?.output, usage?.cacheRead ?? 0, usage?.cacheWrite ?? 0];
    const valid = values.every(value => Number.isSafeInteger(value) && (value as number) >= 0);
    const tokens = valid ? (values as number[]).reduce((sum, value) => sum + value, 0) : 0;
    const known = valid && Number.isSafeInteger(tokens) && (tokens > 0 || failed);
    this.db.prepare("INSERT INTO usage VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET tokens=max(tokens,excluded.tokens), unknown=min(unknown,excluded.unknown)").run(id, day(timestamp), known ? tokens : 0, known ? 0 : 1);
  }
  totals(timestamp = Date.now()): { tokens: number; unknown: number } {
    return { ...this.db.prepare("SELECT coalesce(sum(tokens),0) AS tokens, coalesce(sum(unknown),0) AS unknown FROM usage WHERE day=?").get(day(timestamp)) } as { tokens: number; unknown: number };
  }
  close() { this.db.close(); }
}

export function registerExecutionLimits(pi: ExtensionAPI, policy: () => Policy) {
  const root = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0) === 0;
  let ledger: UsageLedger | undefined, timer: ReturnType<typeof setInterval> | undefined;
  let active = false, deadline = 0, cap = 0, stopped = "", storageError = "";
  let reported = false;
  const stop = (ctx: Pick<ExtensionContext, "abort" | "ui">, reason: string, beforeUser = false) => {
    stopped = reason;
    if (root && !reported) {
      reported = true;
      const record = { message: stopped, beforeUser };
      pi.appendEntry("taucode-execution-limit", record);
      ctx.ui.setStatus("taucode-execution-limit", JSON.stringify(record));
    }
    ctx.abort();
  };
  const enabled = () => !process.env.TAUCODE_CHAT && !process.env.TAUCODE_WORK && policy().enabled !== false && policy().mode === "chain-of-command";
  const problem = () => {
    if (deadline && Date.now() >= deadline) return "The run reached its time limit. Its active agents and tools were stopped.";
    if (!cap) return "";
    if (storageError || !ledger) return "Usage tracking is unavailable. This capped run cannot continue until tracking is restored or the daily allowance is disabled.";
    const usage = ledger.totals();
    if (usage.unknown) return "A provider did not report token usage. This capped run was stopped because its remaining daily allowance cannot be established.";
    if (!Number.isSafeInteger(usage.tokens) || usage.tokens >= cap) return `The project's daily token allowance was reached (${usage.tokens.toLocaleString()} / ${cap.toLocaleString()}). It resets at 00:00 UTC; increase the allowance or wait to continue.`;
    return "";
  };
  const check = (ctx: Pick<ExtensionContext, "abort" | "ui">, beforeUser = false) => {
    if (!active || stopped) return stopped;
    try { stopped = problem(); }
    catch { storageError = "Cannot read usage"; stopped = cap ? problem() : ""; }
    if (stopped) stop(ctx, stopped, beforeUser);
    return stopped;
  };
  pi.on("agent_start", (_event, ctx) => {
    clearInterval(timer); active = enabled(); stopped = ""; reported = false; storageError = "";
    if (!active) return;
    const current = policy(), minutes = current.maxRunMinutes ?? 0;
    cap = current.dailyTokenLimit ?? 0;
    if (!Number.isInteger(minutes) || minutes < 0 || minutes > 1440 || !Number.isSafeInteger(cap) || cap < 0 || cap > 1_000_000_000) {
      stop(ctx, "Invalid execution limits. Check the project's Chain of Command settings.", true); return;
    }
    deadline = root ? (minutes ? Date.now() + minutes * 60_000 : 0) : Number(process.env.TAUCODE_RUN_DEADLINE ?? 0);
    if (root) process.env.TAUCODE_RUN_DEADLINE = String(deadline);
    if (!Number.isSafeInteger(deadline) || deadline < 0) { stop(ctx, "Invalid inherited run deadline", true); return; }
    try {
      if (!ledger && process.env.TAUCODE_COMMAND_USAGE_DB) ledger = new UsageLedger(process.env.TAUCODE_COMMAND_USAGE_DB);
    } catch { storageError = "Cannot open usage"; }
    if (deadline || cap) timer = setInterval(() => check(ctx), 500).unref();
    check(ctx, true);
  });
  pi.on("before_provider_request", (_event, ctx) => { check(ctx); });
  pi.on("tool_call", (_event, ctx) => {
    const reason = check(ctx);
    if (reason) return { block: true, reason };
  });
  pi.on("message_end", (event, ctx) => {
    if (!active || event.message.role !== "assistant" || !ledger) return;
    const message = event.message;
    try {
      ledger.record(`${ctx.sessionManager.getSessionId()}:${message.timestamp}:${message.responseId ?? ""}`, message.usage, message.timestamp, ["error", "aborted"].includes(message.stopReason) && !message.content.length);
    } catch { storageError = "Cannot save usage"; }
    check(ctx);
  });
  pi.on("agent_end", () => {
    clearInterval(timer); active = false;
  });
  pi.on("session_shutdown", () => { clearInterval(timer); active = false; ledger?.close(); ledger = undefined; });
}
