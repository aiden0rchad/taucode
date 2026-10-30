import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { commandRank, inside, type Policy } from "./policy.ts";

type Evidence = { id: string; toolCallId: string; tool: string; input?: string; passed: boolean; output: string; artifact?: { path: string; hash: string } };
type Check = { criterion: string; passed: boolean; evidence: string[]; note: string };
export type Verification = { version: 1; checks: Check[]; evidence: Evidence[]; artifacts: { path: string; hash: string }[] };
export function acceptanceCriteria(task: { task: string; acceptanceCriteria?: string[] | null }): string[] {
  const criteria = task.acceptanceCriteria ?? [task.task];
  if (!Array.isArray(criteria) || !criteria.length || criteria.length > 20 || criteria.some(value => typeof value !== "string" || !value.trim() || value.length > 8000) || new Set(criteria).size !== criteria.length) throw new Error("Provide 1–20 unique, non-empty acceptance criteria (up to 8000 characters each)");
  return criteria;
}
const digest = (path: string) => {
  const stat = statSync(path);
  if (!stat.isFile() || stat.size > 16 * 1024 * 1024) throw new Error("Verification artifacts must be files no larger than 16 MiB");
  return createHash("sha256").update(readFileSync(path)).digest("hex");
};

export function verifyReport(checks: Check[], paths: string[], evidence: Map<string, Evidence>, criteria: string[], cwd: string): Verification {
  if (!Array.isArray(checks) || !checks.length || checks.length > 20 || checks.length !== criteria.length || !Array.isArray(paths) || paths.length > 40 || paths.some(path => typeof path !== "string" || !path || path.length > 4096)) throw new Error("Report every assigned criterion exactly once (up to 20 criteria and 40 artifacts)");
  const covered = new Set<string>();
  for (const check of checks) {
    if (!criteria.includes(check.criterion) || covered.has(check.criterion)) throw new Error("Unknown or repeated acceptance criterion");
    covered.add(check.criterion);
    if (typeof check.passed !== "boolean" || typeof check.note !== "string" || !check.note.trim() || check.note.length > 4000 || !Array.isArray(check.evidence) || !check.evidence.length || check.evidence.length > 40 || check.evidence.some(id => !evidence.has(id))) throw new Error("Each check needs a verdict, note and observed evidence IDs from this QA run");
    for (const id of check.evidence) {
      const item = evidence.get(id)!;
      if (item.id !== id || typeof item.toolCallId !== "string" || !item.toolCallId || typeof item.tool !== "string" || !item.tool || typeof item.passed !== "boolean" || typeof item.output !== "string" || item.output.length > 4000) throw new Error("Invalid observed tool evidence");
    }
    if (check.passed && check.evidence.some(id => !evidence.get(id)!.passed)) throw new Error("Failed tool results cannot support a passing check");
  }
  const used = [...new Set(checks.flatMap(check => check.evidence))].map(id => evidence.get(id)!);
  const observed = used.flatMap(item => item.artifact ? [item.artifact] : []);
  const reviewedPaths = [...new Set([...paths.map(path => resolve(cwd, path)), ...observed.map(item => item.path)])];
  if (reviewedPaths.length > 40) throw new Error("A QA report can review up to 40 artifacts");
  const artifacts = reviewedPaths.map(path => {
    if (!inside(path, cwd)) throw new Error("Verification artifacts must stay inside the task folder");
    const hash = digest(path);
    if (observed.some(item => item.path === path && item.hash !== hash)) throw new Error("Artifact changed since QA read it; read and check it again");
    return { path, hash };
  });
  return {
    version: 1, checks,
    evidence: used, artifacts,
  };
}

export function verificationProblem(reports: Verification[]): string | undefined {
  if (!reports.length) return "No QA major submitted checks backed by observed tool results.";
  for (const report of reports) {
    const failed = report.checks.find(check => !check.passed);
    if (failed) return `QA did not verify “${failed.criterion}”: ${failed.note}`;
    for (const artifact of report.artifacts) {
      try { if (digest(artifact.path) !== artifact.hash) return `Artifact changed after QA: ${artifact.path}`; }
      catch { return `Cannot recheck the reviewed artifact: ${artifact.path}`; }
    }
  }
}

export function registerVerification(pi: ExtensionAPI, policy: () => Policy) {
  const role = process.env.TAUCODE_SUBAGENT_ROLE ?? "commander";
  const root = Number(process.env.TAUCODE_SUBAGENT_DEPTH ?? 0) === 0;
  const enabled = () => !process.env.TAUCODE_CHAT && !process.env.TAUCODE_WORK && policy().enabled !== false && policy().mode === "chain-of-command";
  const major = () => !root && commandRank(role, policy().roles) === "major";
  const criteria: string[] = root || !enabled() || !major() ? [] : acceptanceCriteria({ task: "Verify the assigned task", acceptanceCriteria: JSON.parse(process.env.TAUCODE_VERIFY_CRITERIA || "null") });
  let worked = false, interrupted = false, lastChangeAt = 0;
  const evidence = new Map<string, Evidence>();
  const reports = new Map<string, Verification[]>();
  pi.on("agent_start", () => { worked = false; interrupted = false; lastChangeAt = 0; evidence.clear(); reports.clear(); });
  pi.on("message_end", event => {
    if (event.message.role === "assistant") interrupted = ["aborted", "error"].includes(event.message.stopReason);
  });
  pi.on("before_agent_start", () => {
    if (enabled() && major()) return { message: { customType: "taucode-verification-instructions", display: false, content: `Acceptance criteria: ${JSON.stringify(criteria)}. Independently check every criterion with tools. Tool results supply evidence IDs. Before finishing, call report_verification with every criterion verbatim, its verdict and evidence IDs, and the project artifact paths you checked. Report failed checks honestly; text alone is not verification.` } };
  });
  pi.on("tool_result", (event, ctx) => {
    if (!enabled() || !major() || ["report_verification", "subagent", "write", "edit"].includes(event.toolName)) return;
    const output = event.content.filter(part => part.type === "text").map(part => part.text).join("\n");
    const id = `E${evidence.size + 1}`;
    let artifact: Evidence["artifact"];
    if (!event.isError && event.toolName === "read" && typeof event.input?.path === "string") {
      const path = resolve(ctx.cwd, event.input.path);
      if (inside(path, ctx.cwd)) try { artifact = { path, hash: digest(path) }; } catch { /* Explicit artifact validation will report unreadable/oversized files. */ }
    }
    evidence.set(id, { id, toolCallId: event.toolCallId, tool: event.toolName, input: JSON.stringify(event.input)?.slice(0, 4000), artifact, passed: !event.isError && (output.trim().length > 0 || event.content.some(part => part.type === "image")), output: output.slice(0, 4000) });
    return { content: [...event.content, { type: "text" as const, text: `Verification evidence: ${id}${event.isError ? " (failed)" : ""}` }] };
  });
  if (!root && enabled() && major()) pi.registerTool({
    name: "report_verification", label: "Report verification",
    description: "QA majors only: record every assigned acceptance criterion with observed evidence IDs and reviewed artifact paths. Passing checks require successful tool evidence. Changed artifacts invalidate verification.",
    parameters: Type.Object({ checks: Type.Array(Type.Object({ criterion: Type.String({ maxLength: 8000 }), passed: Type.Boolean(), evidence: Type.Array(Type.String(), { minItems: 1, maxItems: 40 }), note: Type.String({ minLength: 1, maxLength: 4000 }) }), { minItems: 1, maxItems: 20 }), artifacts: Type.Array(Type.String(), { maxItems: 40 }) }),
    async execute(_id, input, _signal, _update, ctx) {
      if (!enabled() || !major()) throw new Error("Only a Chain of Command QA major can report verification");
      const verification = verifyReport(input.checks, input.artifacts, evidence, criteria, ctx.cwd);
      return { content: [{ type: "text", text: verification.checks.map(check => `${check.passed ? "PASS" : "FAIL"}: ${check.criterion} — ${check.note} (${check.evidence.join(", ")})`).join("\n") }], details: { taucodeVerification: verification } };
    },
  });
  pi.on("tool_execution_end", (event, ctx) => {
    if (!root || !enabled()) return;
    worked = true;
    // A later command or edit requires a fresh review, even if it touched a file
    // omitted from the earlier artifact list. Read-only inspection is harmless.
    const results = (event.result.details as any)?.results ?? (event.result.details as any)?.xdev?.inner?.results;
    if (!Array.isArray(results)) {
      if (!["read", "grep", "glob", "find", "ls"].includes(event.toolName)) { reports.clear(); lastChangeAt = Date.now(); }
      return;
    }
    const found: Verification[] = [];
    let implementationEnded = 0;
    const reviewed: number[] = [];
    const visit = (results: any[], depth = 0) => {
      if (depth > 10) return;
      for (const result of results) {
        const messages = result.messages ?? [];
        if (commandRank(result.agent, policy().roles) === "enlisted") implementationEnded = Math.max(implementationEnded, result.endedAt ?? Date.now());
        if (commandRank(result.agent, policy().roles) === "major") {
          const message = messages.filter((message: any) => message.role === "toolResult" && (message.toolName === "report_verification" || message.details?.xdev?.tool === "report_verification") && !message.isError).at(-1);
          const report = message?.details?.taucodeVerification ?? message?.details?.xdev?.inner?.taucodeVerification;
          try {
            if (result.status !== "completed" || report?.version !== 1) throw new Error("Missing completed QA report");
            // Revalidate child details before consuming them as a completion gate.
            const checked = verifyReport(report.checks, report.artifacts.map((artifact: any) => artifact.path), new Map(report.evidence.map((item: Evidence) => [item.id, item])), acceptanceCriteria(result), ctx.cwd);
            if (checked.artifacts.some((artifact, index) => artifact.hash !== report.artifacts[index].hash)) throw new Error("Artifacts changed after QA");
            found.push(report);
            reviewed.push(result.startedAt ?? 0);
          } catch {
            const criteria = Array.isArray(result.acceptanceCriteria) ? result.acceptanceCriteria : [result.task];
            found.push({ version: 1, checks: criteria.map((criterion: string) => ({ criterion, passed: false, evidence: [], note: "QA report missing, invalid, interrupted, or stale" })), evidence: [], artifacts: [] });
          }
        }
        for (const message of messages) {
          const children = message.details?.results ?? message.details?.xdev?.inner?.results;
          if (Array.isArray(children)) visit(children, depth + 1);
          else if (commandRank(result.agent, policy().roles) === "colonel" && message.role === "toolResult" && !["read", "grep", "glob", "find", "ls"].includes(message.toolName)) implementationEnded = Math.max(implementationEnded, message.timestamp ?? result.endedAt ?? Date.now());
        }
      }
    };
    visit(results);
    if (implementationEnded) {
      reports.clear();
      lastChangeAt = Math.max(lastChangeAt, implementationEnded);
    }
    if (reviewed.some(started => started < lastChangeAt)) found.length = 0;
    reports.set(event.toolCallId, found);
  });
  pi.on("agent_end", (_event, ctx) => {
    if (!root || !enabled() || !worked || interrupted) return;
    // A retry may repair a failed criterion; retain the latest verdict for each.
    const all = [...reports.values()].flat(), latest = new Map<string, Verification>();
    for (const report of all) for (const check of report.checks) latest.set(check.criterion, report);
    const checked = all.map(report => ({ ...report, checks: report.checks.filter(check => latest.get(check.criterion) === report) })).filter(report => report.checks.length);
    const problem = verificationProblem(checked);
    const record = { status: problem ? "unverified" : "verified", message: problem ?? `QA reported ${checked.reduce((sum, report) => sum + report.checks.length, 0)} passing acceptance criteria with recorded tool evidence. Reviewed artifacts are unchanged.`, reports: checked };
    pi.appendEntry("taucode-verification", record);
    ctx.ui.setStatus("taucode-verification", JSON.stringify(record));
  });
}
