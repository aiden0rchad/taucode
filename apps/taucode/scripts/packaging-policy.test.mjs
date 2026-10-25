import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { buildEnvironment } from "./release-build.mjs";
import { entitlementProfile, entitlementsFile, expectedEntitlements, matchEntitlements } from "./entitlements.mjs";

test("debug builds retain the original environment and Cargo configuration", () => {
  const env = { PATH: "/bin", CC: "custom-cc", CXX: "custom-cxx", RUSTFLAGS: "-C debuginfo=2" };
  assert.equal(buildEnvironment(true, env), env);
  assert.equal(readFileSync(new URL("../src-tauri/.cargo/config.toml", import.meta.url), "utf8"), "[net]\noffline = true\n");
});

test("release flags append without changing the caller or duplicating the remap", () => {
  const env = { RUSTFLAGS: "-C opt-level=2", CC: "cc", CXX: "c++" };
  const release = buildEnvironment(false, env);
  assert.deepEqual(env, { RUSTFLAGS: "-C opt-level=2", CC: "cc", CXX: "c++" });
  const flags = release.CARGO_ENCODED_RUSTFLAGS.split("\x1f");
  assert.deepEqual(flags.slice(0, -1), ["-C", "opt-level=2"]);
  assert.match(flags.at(-1), /^--remap-path-prefix=.+=\/taucode$/);
  assert.match(release.CC, /\/scripts\/remap-clang\.sh$/);
  assert.match(release.CXX, /\/scripts\/remap-clang\+\+\.sh$/);
  assert.deepEqual(buildEnvironment(false, release), release);
  const encoded = buildEnvironment(false, { RUSTFLAGS: "ignored", CARGO_ENCODED_RUSTFLAGS: "--cfg\x1fvalue=\"with spaces\"" });
  assert.deepEqual(encoded.CARGO_ENCODED_RUSTFLAGS.split("\x1f").slice(0, -1), ["--cfg", "value=\"with spaces\""]);
});

test("each executable has the exact least-privilege entitlement profile", () => {
  const paths = {
    "": "app",
    "Contents/MacOS/codex-monitor": "app",
    "Contents/MacOS/codex_monitor_daemon": "none",
    "Contents/MacOS/codex_monitor_daemonctl": "none",
    "Contents/MacOS/taucode-diag-mcp": "none",
    "Contents/Resources/node/bin/node": "runtime",
    "Contents/Resources/bun/bin/bun": "runtime",
    "Contents/MacOS/node": "runtime",
    "Contents/MacOS/bun": "runtime",
    "Contents/Resources/uv/uv": "none",
    "Contents/Resources/addon.node": "none",
  };
  assert.deepEqual(Object.keys(expectedEntitlements.app).sort(), ["com.apple.security.automation.apple-events", "com.apple.security.device.audio-input"]);
  assert.deepEqual(Object.keys(expectedEntitlements.runtime).sort(), ["com.apple.security.cs.allow-jit", "com.apple.security.cs.allow-unsigned-executable-memory", "com.apple.security.cs.disable-library-validation"]);
  for (const [path, profile] of Object.entries(paths)) {
    assert.equal(entitlementProfile(path), profile);
    matchEntitlements(path, expectedEntitlements[profile]);
    assert.throws(() => matchEntitlements(path, { ...expectedEntitlements[profile], extra: false }));
    const plist = entitlementsFile(path);
    if (profile === "none") { assert.equal(plist, null); assert.throws(() => matchEntitlements(path, expectedEntitlements.app)); }
    else {
      const actual = JSON.parse(execFileSync("/usr/bin/plutil", ["-convert", "json", "-o", "-", plist], { encoding: "utf8" }));
      matchEntitlements(path, actual);
      assert.throws(() => matchEntitlements(path, {}));
      const key = Object.keys(actual)[0];
      assert.throws(() => matchEntitlements(path, { ...actual, [key]: "true" }));
    }
  }
});
