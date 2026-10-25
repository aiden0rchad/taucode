import { verifyEntitlements } from "./entitlements.mjs";
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";
import { setTimeout as delay } from "node:timers/promises";

const app = process.argv[2] && resolve(process.argv[2]);
if (!app?.endsWith(".app")) throw new Error("Usage: check-bundle.mjs <release.app>");
const resources = join(app, "Contents/Resources");
const macos = join(app, "Contents/MacOS");
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const helpers = ["codex_monitor_daemon", "codex_monitor_daemonctl", "taucode-diag-mcp"];
for (const path of ["node/bin/node", "node/bin/npm", "node/bin/npx", "node/lib/node_modules/npm/bin/npm-cli.js", "node/VERSION", "node/LICENSE", "uv/uv"]) {
  assert(existsSync(join(resources, path)), `Missing ${path}`);
}
const nodeVersion = execFileSync(join(resources, "node/bin/node"), ["--version"], { env: { PATH: "" }, encoding: "utf8" }).trim();
assert.equal(nodeVersion, readFileSync(join(resources, "node/VERSION"), "utf8").trim());
execFileSync(join(resources, "node/bin/node"), [join(resources, "node/lib/node_modules/npm/bin/npm-cli.js"), "--version"], { env: { PATH: "" } });
execFileSync(join(resources, "uv/uv"), ["--version"], { env: { PATH: "" } });
for (const engine of ["pi", "omp", "claude-acp", "browser", "computer", "work"]) {
  for (const name of ["computer", "work"].includes(engine) ? ["pyproject.toml", "uv.lock"] : ["package.json", "package-lock.json"]) {
    assert(existsSync(join(resources, "engines", engine, name)), `Missing ${engine}/${name}`);
  }
}
assert(existsSync(join(resources, "engines/omp/no-accounts.mjs")), "Missing omp auth guard");
for (const name of ["codex-monitor", ...helpers]) {
  const binary = join(macos, name);
  assert(existsSync(binary), `Missing helper ${name}`);
  assert(!readFileSync(binary).includes(Buffer.from(repo)), `${name} contains checkout path ${repo}`);
}
function* files(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path);
    else if (entry.isFile()) yield path;
  }
}
for (const path of files(app)) {
  if (execFileSync("/usr/bin/file", ["-b", path], { encoding: "utf8" }).includes("Mach-O")) {
    execFileSync("/usr/bin/codesign", ["--verify", "--strict", path], { stdio: "pipe" });
    verifyEntitlements(app, path);
  }
}
execFileSync("/usr/bin/codesign", ["--verify", "--deep", "--strict", app]);
verifyEntitlements(app, app);
console.log(`Contents, checkout-path, signing and entitlement checks passed (${nodeVersion})`);

async function freePort() {
  const server = net.createServer();
  try {
    await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
    return server.address().port;
  } finally { if (server.listening) await new Promise(resolve => server.close(resolve)); }
}
function rpc(port, id, method, params = {}) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: "127.0.0.1", port });
    let buffer = "";
    socket.setTimeout(15000, () => socket.destroy(new Error(`RPC ${method} timed out`)));
    socket.on("error", reject);
    socket.on("connect", () => socket.write(JSON.stringify({ id, method, params }) + "\n"));
    socket.on("data", chunk => {
      buffer += chunk;
      while (buffer.includes("\n")) {
        const end = buffer.indexOf("\n");
        const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
        try {
          const message = JSON.parse(line);
          if (message.id !== id) continue;
          socket.end();
          if (message.error) reject(new Error(JSON.stringify(message.error))); else resolve(message.result);
        } catch (error) { socket.destroy(); reject(error); }
      }
    });
    socket.on("end", () => reject(new Error(`RPC ${method} closed without a response`)));
  });
}
async function smoke() {
  let port;
  try { port = await freePort(); }
  catch (error) {
    if (!["EPERM", "EACCES"].includes(error.code)) throw error;
    console.log(`SKIP daemon smoke: sandbox denies socket binding (${error.code})`); return;
  }
  const data = mkdtempSync(join(tmpdir(), "taucode-bundle-"));
  const child = spawn(join(macos, helpers[0]), ["--data-dir", data, "--listen", `127.0.0.1:${port}`, "--insecure-no-auth"], {
    env: { PATH: "/usr/bin:/bin" }, cwd: data, stdio: ["ignore", "ignore", "pipe"],
  });
  let stderr = "", spawnError;
  child.stderr.on("data", chunk => { stderr += chunk; });
  child.on("error", error => { spawnError = error; });
  try {
    for (let attempt = 0; attempt < 100; attempt++) {
      if (spawnError) throw spawnError;
      if (child.exitCode !== null) {
        if (/failed to bind.*(Operation not permitted|Permission denied)/.test(stderr)) {
          console.log(`SKIP daemon smoke: sandbox denies binding: ${stderr.trim()}`); return;
        }
        throw new Error(`Daemon exited: ${stderr}`);
      }
      try {
        const settings = await rpc(port, 1, "get_app_settings");
        assert(settings && typeof settings === "object", "Invalid app settings");
        console.log("Daemon get_app_settings passed; runtimes:", JSON.stringify(await rpc(port, 2, "engine_runtime", { action: "status" })));
        return;
      } catch (error) { if (error.code !== "ECONNREFUSED") throw error; }
      await delay(100);
    }
    throw new Error(`Daemon startup timed out: ${stderr}`);
  } finally {
    if (child.exitCode === null && !spawnError) {
      child.kill("SIGTERM");
      const kill = setTimeout(() => child.kill("SIGKILL"), 3000);
      await new Promise(resolve => child.once("exit", resolve)); clearTimeout(kill);
    }
    rmSync(data, { recursive: true, force: true });
  }
}
await smoke();
