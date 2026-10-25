import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const expectedEntitlements = {
  app: { "com.apple.security.device.audio-input": true, "com.apple.security.automation.apple-events": true },
  runtime: { "com.apple.security.cs.allow-jit": true, "com.apple.security.cs.allow-unsigned-executable-memory": true, "com.apple.security.cs.disable-library-validation": true },
  none: {},
};

export function entitlementProfile(path) {
  if (path === "" || path === "Contents/MacOS/codex-monitor") return "app";
  if ((path.startsWith("Contents/Resources/") || path.startsWith("Contents/MacOS/")) && ["node", "bun"].includes(basename(path))) return "runtime";
  return "none";
}

export function entitlementsFile(path) {
  const profile = entitlementProfile(path);
  return profile === "none" ? null : join(dirname(fileURLToPath(import.meta.url)), "../src-tauri",
    profile === "app" ? "Entitlements.plist" : "Entitlements.runtime.plist");
}

export function matchEntitlements(path, actual) {
  assert.deepEqual(actual, expectedEntitlements[entitlementProfile(path)], `Unexpected entitlements: ${path || "app bundle"}`);
}

export function verifyEntitlements(app, binary) {
  const xml = execFileSync("/usr/bin/codesign", ["-d", "--entitlements", ":-", binary], { encoding: "utf8", stdio: "pipe" }).trim();
  const actual = xml ? JSON.parse(execFileSync("/usr/bin/plutil", ["-convert", "json", "-o", "-", "--", "-"], { input: xml, encoding: "utf8" })) : {};
  matchEntitlements(relative(app, binary), actual);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [app, binary] = process.argv.slice(2);
  if (!app || !binary) throw new Error("Usage: entitlements.mjs <app> <binary>");
  console.log(entitlementsFile(relative(resolve(app), resolve(binary))) || "");
}
