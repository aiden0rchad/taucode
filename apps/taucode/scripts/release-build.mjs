import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const app = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export function buildEnvironment(debug, env = process.env) {
  if (debug) return env;
  const flag = `--remap-path-prefix=${resolve(app, "../..")}=/taucode`;
  // Cargo gives encoded flags precedence; preserve their argument boundaries.
  const flags = env.CARGO_ENCODED_RUSTFLAGS !== undefined
    ? env.CARGO_ENCODED_RUSTFLAGS.split("\x1f").filter(Boolean)
    : (env.RUSTFLAGS || "").split(/\s+/).filter(Boolean);
  if (!flags.includes(flag)) flags.push(flag);
  return { ...env, CARGO_ENCODED_RUSTFLAGS: flags.join("\x1f"),
    CC: join(app, "scripts/remap-clang.sh"), CXX: join(app, "scripts/remap-clang++.sh") };
}

export function prepareRelease(env) {
  // Whisper does not track compiler environment changes. Only invalidate its
  // release cache; debug/test compiler selection and artifacts are untouched.
  const root = join(app, "src-tauri");
  const target = resolve(root, env.CARGO_TARGET_DIR || "target");
  const stamp = join(target, "engine-resources/.release-native-environment");
  const hash = createHash("sha256").update(env.CARGO_ENCODED_RUSTFLAGS)
    .update(readFileSync(env.CC)).digest("hex");
  if (!existsSync(stamp) || readFileSync(stamp, "utf8") !== hash) {
    execFileSync("cargo", ["clean", "--package", "whisper-rs-sys", "--profile", "release"], { cwd: root, env, stdio: "inherit" });
    mkdirSync(dirname(stamp), { recursive: true });
    writeFileSync(stamp, hash);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (!command) throw new Error("Usage: release-build.mjs <command> [args...]");
  const env = buildEnvironment(false);
  prepareRelease(env);
  const result = spawnSync(command, args, { env, stdio: "inherit" });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}
