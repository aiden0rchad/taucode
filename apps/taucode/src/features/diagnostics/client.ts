import type { AppSettings } from "@/types";
import fixtures from "../../../../../docs/fixtures/diagnostics-redaction.json";

export type DiagnosticEvent = { kind: "error" | "warning" | "perf" | "usage" | "ux" | "feedback"; area: string; code: string; message: string; severity?: string; context?: Record<string, unknown>; metrics?: Record<string, number>; detail?: Record<string, unknown> };
export const defaultDiagnostics = { level: "standard" as const, retention: 7, includeContentSnippets: false, exposeToAgents: false };
let enabled = false;
let options: NonNullable<AppSettings["diagnostics"]> = defaultDiagnostics;
let queue: DiagnosticEvent[] = [];
let timer: ReturnType<typeof setInterval> | undefined;
let cleanup: (() => void) | undefined;
let context: Record<string, unknown> = {};
let send: (events: DiagnosticEvent[]) => Promise<unknown> = async () => {};
export function diagnosticsEnabled() { return enabled; }
export function redact(text: string, home = "", keys: string[] = [], identifier = false) {
  for (const key of keys.filter(Boolean)) text = text.split(key).join("[REDACTED]");
  if (home) text = text.split(home).join("~");
  for (const [pattern, replacement] of fixtures.rules) text = text.replace(new RegExp(pattern.replace("(?i)", ""), pattern.startsWith("(?i)") ? "gi" : "g"), replacement);
  if (!identifier) text = text.replace(new RegExp(fixtures.longToken, "g"), token => new RegExp(fixtures.uuid).test(token) ? token : "[REDACTED]");
  return text;
}
export function diagnosticContext(value: Record<string, unknown>) { if (enabled) context = { ...context, ...value }; }
export function record(event: DiagnosticEvent) {
  if (!enabled) return;
  if (options.level === "essential" && event.kind !== "error" && event.kind !== "feedback") return;
  if (options.level !== "verbose" && event.kind === "ux") return;
  if (queue.length < 100) queue.push({ ...event, message: redact(event.message).slice(0, 300), severity: event.severity ?? (event.kind === "error" ? "high" : "low"), context: { ...context, ...event.context } });
}
export function configureDiagnostics(settings: Pick<AppSettings, "features" | "diagnostics">, transport: typeof send) {
  const next = !!settings.features?.diagnostics;
  options = settings.diagnostics ?? defaultDiagnostics;
  send = transport;
  if (next === enabled) return;
  enabled = next;
  if (!next) { clearInterval(timer); timer = undefined; cleanup?.(); cleanup = undefined; queue = []; context = {}; return; }
  timer = setInterval(() => { if (!queue.length) return; const batch = queue; queue = []; void send(batch).catch(() => {}); }, 2000);
  const onError = (e: ErrorEvent) => record({ kind: "error", area: "ui", code: "ui.uncaught_error", message: "Unhandled JavaScript error", detail: { errorClass: e.error instanceof Error ? e.error.name : "Error", stack: e.error instanceof Error ? e.error.stack?.split("\n").slice(1, 4).join("\n") : undefined } });
  const onRejection = (e: PromiseRejectionEvent) => record({ kind: "error", area: "ui", code: "ui.unhandled_rejection", message: "Unhandled promise rejection", detail: { errorClass: e.reason instanceof Error ? e.reason.name : "Error" } });
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  let observer: PerformanceObserver | undefined;
  if (typeof PerformanceObserver !== "undefined" && PerformanceObserver.supportedEntryTypes?.includes("longtask")) {
    observer = new PerformanceObserver(list => { for (const entry of list.getEntries()) if (entry.duration > 150) record({ kind: "perf", area: "ui", code: "ui.long_task", message: "UI long task", metrics: { durationMs: entry.duration } }); });
    observer.observe({ type: "longtask", buffered: false });
  }
  record({ kind: "perf", area: "startup", code: "startup.interactive", message: "Frontend interactive", detail: { timeOriginMs: performance.timeOrigin }, metrics: { durationMs: performance.now() } });
  for (const entry of performance.getEntriesByType("paint")) record({ kind: "perf", area: "startup", code: "startup.first_paint", message: "First paint", detail: { timeOriginMs: performance.timeOrigin }, metrics: { durationMs: entry.startTime } });
  cleanup = () => { observer?.disconnect(); window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection); };
}
export async function instrumentInvoke<T>(command: string, call: () => Promise<T>): Promise<T> {
  if (!enabled || command === "diagnostics_request") return call();
  const start = performance.now();
  try { return await call(); }
  catch (error) {
    const area = command.includes("git") ? "git" : command.startsWith("browser") ? "browser" : command.startsWith("computer") ? "computer" : command.startsWith("checkpoint") ? "checkpoints" : "ipc";
    record({ kind: "error", area, code: area + ".invoke_failed", message: "Command failed: " + command, detail: { command, errorClass: error instanceof Error ? error.name : "CommandError" } });
    throw error;
  } finally {
    const durationMs = performance.now() - start;
    if (durationMs > 300) record({ kind: "perf", area: "ipc", code: "ipc.slow", message: "Slow command: " + command, detail: { command }, metrics: { durationMs } });
    if (command.includes("git") && durationMs > 500) record({ kind: "perf", area: "git", code: "git.slow", message: "Slow git operation", detail: { command }, metrics: { durationMs } });
  }
}
export function openFeedback(detail: Record<string, unknown> = {}) { if (enabled) window.dispatchEvent(new CustomEvent("taucode:diagnostic-feedback", { detail })); }
