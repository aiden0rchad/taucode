import { buildEnvironment, prepareRelease } from "./release-build.mjs";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../src-tauri");
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (.+)$/m)[1];
const arch = process.env.TAURI_ENV_ARCH;
const target = process.platform === "darwin" && arch ? `${{ arm64: "aarch64", x64: "x86_64" }[arch] || arch}-apple-darwin` : host;
if (target !== host) throw new Error("Cross-compilation is unsupported: build on the target architecture");
const debug = process.env.TAURI_ENV_DEBUG === "true";
const env = process.platform === "darwin" ? buildEnvironment(debug) : process.env;
if (!debug && process.platform === "darwin") prepareRelease(env);
const helpers = ["codex_monitor_daemon", "codex_monitor_daemonctl", "taucode-diag-mcp"];
execFileSync("cargo", ["build", "--offline", "--features", "custom-protocol", ...(debug ? [] : ["--release"]), "--bins"], { cwd: root, env, stdio: "inherit" });
mkdirSync(join(root, "binaries"), { recursive: true });
const output = resolve(root, process.env.CARGO_TARGET_DIR || "target", debug ? "debug" : "release");
if (!debug && process.platform === "darwin") {
  for (const name of ["codex-monitor", ...helpers]) {
    const binary = join(output, name);
    // Swift static archives leave source paths in Mach-O debug symbols. Use the
    // platform stripper before signing; Rust's objcopy cannot always load LLVM.
    execFileSync("/usr/bin/strip", ["-S", binary]);
    if (readFileSync(binary).includes(Buffer.from(resolve(root, "../../..")))) {
      throw new Error(`${name} still contains checkout paths`);
    }
  }
}
const extension = process.platform === "win32" ? ".exe" : "";
for (const name of helpers) copyFileSync(join(output, name + extension), join(root, "binaries", `${name}-${target}${extension}`));
