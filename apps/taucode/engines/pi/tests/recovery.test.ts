import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import test from "node:test";
import { subagentCheckpoint } from "../extensions/lib/recovery.ts";

test("recovery coalesces updates, atomically replaces one private file and flushes final evidence", async t => {
  const dir = mkdtempSync(join(tmpdir(), "taucode-recovery-"));
  const folder = join(dir, "session.jsonl.agents");
  const checkpoint = subagentCheckpoint(join(dir, "session.jsonl"), "call/../../unsafe");
  t.after(() => { checkpoint.discard(); rmSync(dir, { recursive: true, force: true }); });
  checkpoint.update({ partial: 0 });
  const file = join(folder, readdirSync(folder)[0]);
  const first = readFileSync(file, "utf8");
  for (let partial = 1; partial <= 100; partial++) checkpoint.update({ partial });
  assert.equal(readFileSync(file, "utf8"), first, "streaming does not rewrite on each token");
  await delay(1100);
  assert.deepEqual(JSON.parse(readFileSync(file, "utf8")).result, { partial: 100 });
  assert.equal(readdirSync(folder).length, 1, "snapshots do not accumulate");
  if (process.platform !== "win32") assert.equal(statSync(file).mode & 0o777, 0o600);
  checkpoint.update({ completed: true }); checkpoint.flush();
  assert.deepEqual(JSON.parse(readFileSync(file, "utf8")).result, { completed: true });
  checkpoint.discard();
  await delay(1100);
  assert.equal(readdirSync(folder).length, 0, "discard cancels any pending rewrite");
});

test("a killed writer leaves its last complete checkpoint readable", { skip: process.platform === "win32" }, async t => {
  const dir = mkdtempSync(join(tmpdir(), "taucode-recovery-kill-"));
  const session = join(dir, "session.jsonl");
  const child = spawn(process.execPath, ["--input-type=module", "-e", `
    import { subagentCheckpoint } from ${JSON.stringify(new URL("../extensions/lib/recovery.ts", import.meta.url).href)};
    const checkpoint = subagentCheckpoint(${JSON.stringify(session)}, 'call');
    checkpoint.update({details:{results:[{agent:'enlisted',status:'running',startedAt:123,messages:[{role:'assistant',content:[{type:'text',text:'Saved partial evidence'}]}]}]}});
    process.stdout.write('ready'); setInterval(() => {}, 1000);
  `], { stdio: ["ignore", "pipe", "pipe"] });
  t.after(() => { child.kill("SIGKILL"); rmSync(dir, { recursive: true, force: true }); });
  const ready = await Promise.race([once(child.stdout, "data"), once(child, "exit").then(() => { throw new Error("checkpoint writer exited before ready"); })]);
  assert.equal(String(ready[0]), "ready");
  const exited = once(child, "exit"); child.kill("SIGKILL"); await exited;
  const folder = `${session}.agents`;
  const saved = JSON.parse(readFileSync(join(folder, readdirSync(folder)[0]), "utf8"));
  assert.equal(saved.result.details.results[0].status, "running");
  assert.match(JSON.stringify(saved), /Saved partial evidence/);
});
