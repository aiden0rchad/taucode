import { spawn } from "node:child_process";
import { constants } from "node:os";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(import.meta.url);
export function ownedCommand(command: string) {
  if (process.platform === "win32") return command;
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
  // OMP's native command runner has no `exec` builtin or shell `$0`.
  if (process.env.TAUCODE_OMP === "1") return [process.execPath, script, String(process.pid), "/bin/bash", command].map(quote).join(" ");
  return `exec ${[process.execPath, script, String(process.pid)].map(quote).join(" ")} "$0" ${quote(command)}`;
}

// A separate supervisor survives SIGKILL of the engine. Its child owns a process
// group, so foreground pipelines stop together even after the engine disappears.
if (process.argv[1] === script) {
  const owner = Number(process.argv[2]), shell = process.argv[3], command = process.argv[4];
  if (!Number.isSafeInteger(owner) || owner <= 1 || !shell || command === undefined || process.platform === "win32") process.exit(2);
  const child = spawn(shell, ["-c", command], { detached: true, stdio: "inherit" });
  const parent = process.ppid;
  let stopping = false;
  const kill = (signal: NodeJS.Signals) => {
    if (child.pid) try { process.kill(-child.pid, signal); } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ESRCH") console.error("Command cleanup failed:", error);
    }
  };
  const stop = (code: number) => {
    if (stopping) return;
    stopping = true;
    clearInterval(watch);
    kill("SIGTERM");
    setTimeout(() => { kill("SIGKILL"); process.exit(code); }, 1000);
  };
  const watch = setInterval(() => {
    if (process.ppid !== parent) { stop(143); return; }
    try { process.kill(owner, 0); } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ESRCH") stop(143);
    }
  }, 200);
  for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"] as const) process.on(signal, () => stop(128 + constants.signals[signal]));
  child.on("error", error => { console.error(error.message); clearInterval(watch); process.exit(127); });
  child.on("exit", (code, signal) => {
    if (!stopping) { clearInterval(watch); process.exit(code ?? 128 + (signal ? constants.signals[signal] : 1)); }
  });
}
