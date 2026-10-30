import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// One replaceable snapshot per active delegation; never append full trees per token.
export function subagentCheckpoint(sessionFile: string | undefined, toolCallId: string) {
  const directory = sessionFile && `${sessionFile}.agents`;
  const file = directory && join(directory, `${createHash("sha256").update(toolCallId).digest("hex")}.json`);
  let latest: unknown, savedAt = 0, written = "", lastWrite = 0, warned = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const flush = () => {
    clearTimeout(timer); timer = undefined;
    if (!file || latest === undefined) return;
    const result = JSON.stringify(latest);
    if (result === written) return;
    const temp = `${file}.${randomUUID()}.tmp`;
    lastWrite = Date.now();
    try {
      mkdirSync(directory!, { recursive: true, mode: 0o700 });
      writeFileSync(temp, `{"version":1,"toolCallId":${JSON.stringify(toolCallId)},"savedAt":${savedAt},"result":${result}}`, { mode: 0o600, flag: "wx" });
      renameSync(temp, file);
      written = result; warned = false;
    } catch (error) {
      if (!warned) console.error("Subagent recovery checkpoint unavailable:", error);
      warned = true;
    } finally { try { rmSync(temp, { force: true }); } catch { /* Keep the original write error. */ } }
  };
  return {
    update(result: unknown) {
      if (!file) return;
      latest = result; savedAt = Date.now();
      if (Date.now() - lastWrite >= 1000) flush();
      else timer ??= setTimeout(flush, 1000 - (Date.now() - lastWrite)).unref();
    },
    flush,
    discard() {
      clearTimeout(timer);
      try { if (file) rmSync(file, { force: true }); }
      catch (error) { console.error("Subagent recovery checkpoint cleanup failed:", error); }
    },
  };
}
