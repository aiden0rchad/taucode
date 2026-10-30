import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "win32" || process.arch !== "x64") throw new Error("Windows x64 packaging requires a Windows x64 host");
if (!process.env.TAUCODE_BUNDLE_UV) throw new Error("Set TAUCODE_BUNDLE_UV to the directory containing uv.exe and its licenses");
const [major, minor] = process.versions.node.split(".").map(Number);
if (major < 22 || (major === 22 && minor < 19)) throw new Error("Node >=22.19 required");
const staging = resolve(dirname(fileURLToPath(import.meta.url)), "../src-tauri/target/engine-resources");
const prefix = dirname(process.execPath);
const node = join(staging, "node");
const uv = join(staging, "uv");
for (const dir of [node, uv]) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
}
mkdirSync(join(node, "bin"));
cpSync(process.execPath, join(node, "bin/node.exe"));
cpSync(join(prefix, "node_modules/npm"), join(node, "lib/node_modules/npm"), { recursive: true });
cpSync(join(prefix, "LICENSE"), join(node, "LICENSE"));
for (const name of ["npm", "npx"]) {
  writeFileSync(join(node, "bin", `${name}.cmd`), `@"%~dp0node.exe" "%~dp0..\\lib\\node_modules\\npm\\bin\\${name}-cli.js" %*\r\n`);
}
writeFileSync(join(node, "VERSION"), process.version + "\n");
for (const name of ["uv.exe", "LICENSE-APACHE", "LICENSE-MIT"]) cpSync(join(process.env.TAUCODE_BUNDLE_UV, name), join(uv, name));
writeFileSync(join(uv, "VERSION"), execFileSync(join(uv, "uv.exe"), ["--version"]));
// Reject an accidentally supplied ARM or 32-bit runtime before packaging it.
for (const binary of [join(node, "bin/node.exe"), join(uv, "uv.exe")]) {
  const bytes = readFileSync(binary);
  const pe = bytes.readUInt32LE(0x3c);
  if (bytes.toString("ascii", 0, 2) !== "MZ" || bytes.readUInt32LE(pe) !== 0x4550 || bytes.readUInt16LE(pe + 4) !== 0x8664) {
    throw new Error(`Expected a Windows x64 executable: ${binary}`);
  }
}
console.log(`Staged Node ${process.version}, npm and uv (Windows x64)`);
