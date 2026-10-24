import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { decide, normalizeRule, type Access } from "../extensions/lib/policy.ts";

test("Work overwrites need fresh approval in every mode; Code behavior stays unchanged", () => {
  const root = mkdtempSync(join(tmpdir(), "work-policy-"));
  const cwd = join(root, "space"), extra = join(root, "reference"), home = join(root, "agent");
  mkdirSync(join(cwd, "outputs"), { recursive: true });
  mkdirSync(extra);
  writeFileSync(join(cwd, "notes.md"), "user file");
  writeFileSync(join(cwd, "outputs/result.md"), "output");
  symlinkSync(extra, join(cwd, "escape"));
  try {
    for (const accessMode of ["ask", "read-only", "full-access"] as const) {
      const call = { tool: "write", cwd, path: "notes.md" };
      const access: Access = { accessMode, kind: "work", workspace: cwd, readOnlyFolders: [extra], sessionAllow: [normalizeRule(call)] };
      assert.equal(decide(access, call, home, true), "ask");
      assert.equal(decide(access, { ...call, tool: "edit" }, home, true), "ask");
      assert.equal(decide(access, call, home, false, { workspace: cwd, toolAccess: "full" }), "block");
      for (const path of ["../outside", "escape/new.md", extra + "/new.md"]) assert.equal(decide(access, { ...call, path }, home, true), "block");
      assert.equal(decide(access, { tool: "read", cwd, path: extra }, home, true), "allow");
      assert.equal(decide(access, { tool: "read", cwd, path: root }, home, true), "block");
      const deletion = decide(access, { tool: "bash", cwd, command: "rm notes.md" }, home, true);
      assert.equal(deletion, "ask");
      if (accessMode === "read-only") assert.equal(decide({ accessMode, sessionAllow: [] }, { tool: "bash", cwd, command: "rm notes.md" }, home, true), "block");
      if (accessMode === "full-access") {
        assert.equal(decide(access, { ...call, path: "outputs/result.md" }, home, true), "allow");
        assert.equal(decide({ accessMode, sessionAllow: [] }, call, home, true), "allow");
        assert.equal(decide(access, { tool: "bash", cwd, command: "python make_report.py" }, home, true), "ask");
      }
    }
    rmSync(join(cwd, "outputs"), { recursive: true });
    symlinkSync(cwd, join(cwd, "outputs"));
    assert.equal(decide({ kind: "work", workspace: cwd, accessMode: "full-access", sessionAllow: [] }, { tool: "write", cwd, path: "notes.md" }, home, true), "ask");
    assert.equal(decide({ kind: "work", workspace: cwd, accessMode: "full-access", sessionAllow: [] }, { tool: "write", cwd, path: "outputs/notes.md" }, home, true), "ask");
  } finally { rmSync(root, { recursive: true, force: true }); }
});


test("bundled Work skills pass the installed pi 1.4.0 parser", async () => {
  const { loadSkillsFromDir } = await import("../node_modules/@earendil-works/pi-coding-agent/dist/core/skills.js");
  const { fileURLToPath } = await import("node:url");
  const result = loadSkillsFromDir({ dir: fileURLToPath(new URL("../../work/skills", import.meta.url)), source: "test" });
  assert.deepEqual(result.skills.map(s => s.name).sort(), ["charts", "docx", "pdf", "pptx", "xlsx"]);
  assert.deepEqual(result.diagnostics, []);
});

test("Work questions use pi UI and cannot run in non-interactive children", async () => {
  const work = (await import("../extensions/taucode-work.ts")).default;
  let tool: any;
  work({ on: () => {}, registerTool: (registered: any) => { tool = registered; } } as any);
  const ctx = { hasUI: true, ui: { input: async (question: string) => { assert.equal(question, "Which month?"); return "January"; }, select: async () => "January" } };
  const result = await tool.execute("question", { question: "Which month?" }, undefined, undefined, ctx);
  assert.equal(result.content[0].text, "January");
  await assert.rejects(tool.execute("question", { question: "Which month?" }, undefined, undefined, { ...ctx, hasUI: false }), /parent task/);
});
