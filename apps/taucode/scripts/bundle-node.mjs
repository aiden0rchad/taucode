import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const staging = join(app, "src-tauri/target/engine-resources");
const arch = process.env.TAURI_ENV_ARCH || process.arch;
const expected = { arm64: "arm64", aarch64: "arm64", x64: "x86_64", x86_64: "x86_64" }[arch];
if (process.platform !== "darwin" || !expected) throw new Error(`Unsupported packaging target: ${process.platform}/${arch}`);
function verify(binary) {
  const arches = execFileSync("/usr/bin/lipo", ["-archs", binary], { encoding: "utf8" }).trim().split(/\s+/);
  if (!arches.includes(expected)) throw new Error(`${binary}: expected ${expected}, found ${arches.join(", ")}`);
  const links = execFileSync("/usr/bin/otool", ["-L", binary], { encoding: "utf8" });
  if (links.split("\n").slice(1).some(line => /^\s*\/(?!usr\/lib\/|System\/Library\/)/.test(line))) {
    throw new Error(`${binary} links to non-system libraries; use a standalone distribution`);
  }
}
const node = realpathSync(process.env.TAUCODE_BUNDLE_NODE || process.execPath);
verify(node);
const version = execFileSync(node, ["--version"], { encoding: "utf8" }).trim();
const [major, minor] = version.slice(1).split(".").map(Number);
if (!(major > 22 || (major === 22 && minor >= 19))) throw new Error(`Node >=22.19 required, found ${version}`);
const prefix = resolve(dirname(node), "..");
const npm = join(prefix, "lib/node_modules/npm");
for (const path of [join(prefix, "LICENSE"), join(npm, "bin/npm-cli.js"), join(npm, "bin/npx-cli.js")]) {
  if (!existsSync(path)) throw new Error(`Missing Node distribution file: ${path}`);
}
const dest = join(staging, "node");
rmSync(dest, { recursive: true, force: true });
mkdirSync(join(dest, "bin"), { recursive: true });
cpSync(node, join(dest, "bin/node"));
chmodSync(join(dest, "bin/node"), 0o755);
cpSync(npm, join(dest, "lib/node_modules/npm"), { recursive: true, dereference: true });
cpSync(join(prefix, "LICENSE"), join(dest, "LICENSE"));
for (const name of ["npm", "npx"]) symlinkSync(`../lib/node_modules/npm/bin/${name}-cli.js`, join(dest, "bin", name));
writeFileSync(join(dest, "VERSION"), version + "\n");

// uv downloads managed Python on first setup; no system Python is needed.
const uv = realpathSync(process.env.TAUCODE_BUNDLE_UV || execFileSync("/usr/bin/which", ["uv"], { encoding: "utf8" }).trim());
verify(uv);
const uvDest = join(staging, "uv");
rmSync(uvDest, { recursive: true, force: true });
mkdirSync(uvDest, { recursive: true });
cpSync(uv, join(uvDest, "uv"));
chmodSync(join(uvDest, "uv"), 0o755);
for (const name of ["LICENSE-APACHE", "LICENSE-MIT"]) {
  const license = join(process.env.TAUCODE_UV_LICENSE_DIR || resolve(dirname(uv), ".."), name);
  cpSync(license, join(uvDest, name));
}
writeFileSync(join(uvDest, "VERSION"), execFileSync(uv, ["--version"]));

// Tauri signs sidecars but not Mach-O resources. Give plain `tauri build` valid
// ad-hoc runtime signatures; release-macos.sh replaces them with the chosen ID.
for (const binary of [join(dest, "bin/node"), join(uvDest, "uv")]) {
  execFileSync("/usr/bin/codesign", ["--force", "--sign", "-", "--options", "runtime", "--timestamp=none",
    "--entitlements", join(app, "src-tauri/Entitlements.plist"), binary], { stdio: "inherit" });
}

// whisper-rs-sys does not track CC/CXX changes. Invalidate its cached CMake
// compiler once when adopting/updating these wrappers, before Cargo starts.
const profile = process.env.TAURI_ENV_DEBUG === "true" ? "dev" : "release";
const stamp = join(staging, `.native-remap-${profile}`);
const nativeHash = createHash("sha256").update(app).update(readFileSync(join(app, "scripts/remap-clang.sh"))).digest("hex");
if (!existsSync(stamp) || readFileSync(stamp, "utf8") !== nativeHash) {
  execFileSync("cargo", ["clean", "--package", "whisper-rs-sys", "--profile", profile], { cwd: join(app, "src-tauri"), stdio: "inherit" });
  mkdirSync(staging, { recursive: true });
  writeFileSync(stamp, nativeHash);
}

console.log(`Staged Node ${readFileSync(join(dest, "VERSION"), "utf8").trim()}, npm and uv (${expected})`);
