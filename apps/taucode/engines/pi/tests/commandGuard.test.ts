import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { ownedCommand } from "../extensions/lib/commandGuard.ts";

const script = fileURLToPath(new URL("../extensions/lib/commandGuard.ts", import.meta.url));
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

for (const omp of [false, true]) test(`command supervision preserves shell input, environment and exit status: ${omp ? "OMP" : "Pi"}`, { skip: process.platform === "win32" }, async () => {
  const original = process.env.TAUCODE_OMP;
  if (omp) process.env.TAUCODE_OMP = "1"; else delete process.env.TAUCODE_OMP;
  const command = ownedCommand("printf '%s\\n' \"$TAU_TEST_LITERAL\"; exit 37");
  if (original === undefined) delete process.env.TAUCODE_OMP; else process.env.TAUCODE_OMP = original;
  if (omp) { assert.ok(!command.startsWith("exec ")); assert.ok(!command.includes('"$0"')); }
  const child = spawn("/bin/bash", ["-c", command], { env: { ...process.env, TAU_TEST_LITERAL: "quotes ' \" ` $()\nsecond line" } });
  let output = ""; child.stdout.on("data", data => { output += data; });
  const [code] = await once(child, "close");
  assert.equal(code, 37);
  assert.equal(output, "quotes ' \" ` $()\nsecond line\n");
});

for (const mode of ["cancel", "crash", "stubborn", "managed-stop", "managed-crash"]) test(`foreground process stops: ${mode}`, { skip: process.platform === "win32", timeout: 10000 }, async t => {
  const managed = mode.startsWith("managed-");
  const timer = `${quote(process.execPath)} -e ${quote(`${mode === "stubborn" || managed ? "process.on('SIGTERM',()=>{});" : ""}console.log('READY '+process.pid); setInterval(()=>{},1000)`)} | cat`;
  const parent = spawn(process.execPath, ["--input-type=module", "-e", `
    import {spawn} from 'node:child_process';
    const guard=spawn(process.execPath,[${JSON.stringify(script)},String(process.pid),'/bin/bash',${JSON.stringify(timer)},...${JSON.stringify(managed ? ["--shared-group"] : [])}],{detached:${managed},stdio:['ignore','inherit','inherit']});
    process.stdout.write('GUARD '+guard.pid+'\\n'); setInterval(()=>{},1000);
  `], { stdio: ["ignore", "pipe", "pipe"] });
  let output = "", guard = 0, worker = 0;
  t.after(() => { for (const pid of [parent.pid, guard, worker]) if (pid) try { process.kill(pid, "SIGKILL"); } catch { /* already exited */ } });
  for await (const data of parent.stdout) {
    output += data;
    guard = Number(/GUARD (\d+)/.exec(output)?.[1]);
    worker = Number(/READY (\d+)/.exec(output)?.[1]);
    if (guard && worker) break;
  }
  assert.ok(guard && worker, output);
  process.kill(mode === "managed-stop" ? -guard : mode === "cancel" ? guard : parent.pid!, mode === "cancel" ? "SIGTERM" : "SIGKILL");
  const deadline = Date.now() + 4000;
  while (Date.now() < deadline) {
    try { process.kill(worker, 0); } catch { return; }
    // Linux can retain an already-dead orphan as a zombie until init reaps it.
    if (process.platform === "linux") try {
      if (/\) Z /.test(readFileSync(`/proc/${worker}/stat`, "utf8"))) return;
    } catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return; throw error; }
    await new Promise(resolve => setTimeout(resolve, 30));
  }
  assert.fail("foreground command survived its owner");
});
