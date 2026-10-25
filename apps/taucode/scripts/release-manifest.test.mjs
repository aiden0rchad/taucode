import { test } from "node:test";
import assert from "node:assert/strict";
import { buildManifest } from "./release-manifest.mjs";

test("manifest records exact pins, artifact hash, versions and identity type", () => {
  const input = { version: "1.2.3", nodeVersion: "v26.10.0\n", engines: { pi: { agent: "1.4.0" } }, dmg: Buffer.from("abc"), identity: "taucode Dev" };
  assert.deepEqual(buildManifest(input), {
    version: "1.2.3", nodeVersion: "v26.10.0", engines: input.engines, codegraphVersion: null,
    dmgSha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    signingIdentityType: "local",
  });
  const release = buildManifest({ ...input, identity: "Developer ID Application: Example (TEAM)", codegraphVersion: "2.0" });
  assert.equal(release.signingIdentityType, "developer-id");
  assert.equal(release.codegraphVersion, "2.0");
});
