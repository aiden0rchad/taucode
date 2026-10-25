import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function buildManifest({ version, nodeVersion, engines, codegraphVersion = null, dmg, identity }) {
  return {
    version, nodeVersion: nodeVersion.trim(), engines, codegraphVersion,
    dmgSha256: createHash("sha256").update(dmg).digest("hex"),
    signingIdentityType: identity.startsWith("Developer ID Application:") ? "developer-id" : "local",
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [app, dmg] = process.argv.slice(2);
  if (!app || !dmg) throw new Error("Usage: release-manifest.mjs app dmg");
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const resources = join(app, "Contents/Resources");
  const config = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json")));
  const engines = Object.fromEntries(["pi", "omp", "claude-acp", "browser"].map(name => {
    const lock = JSON.parse(readFileSync(join(resources, "engines", name, "package-lock.json")));
    const pins = Object.fromEntries(Object.keys(lock.packages[""].dependencies).map(dep => [dep, lock.packages[`node_modules/${dep}`].version]));
    return [name, pins];
  }));
  const codegraph = join(resources, "codegraph/VERSION");
  const manifest = buildManifest({ version: config.version, nodeVersion: readFileSync(join(resources, "node/VERSION"), "utf8"), engines,
    codegraphVersion: existsSync(codegraph) ? readFileSync(codegraph, "utf8").trim() : null,
    dmg: readFileSync(dmg), identity: process.env.TAUCODE_SIGNING_IDENTITY || "taucode Dev" });
  writeFileSync(join(dirname(dmg), "release-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
}
