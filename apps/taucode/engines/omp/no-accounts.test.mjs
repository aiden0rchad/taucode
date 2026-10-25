import assert from "node:assert/strict";
import { SqliteAuthCredentialStore } from "@oh-my-pi/pi-ai";
import "./no-accounts.mjs";
const store = Object.create(SqliteAuthCredentialStore.prototype);
for (const method of ["saveOAuth", "saveApiKey", "upsertAuthCredential", "replaceAuthCredentials", "updateAuthCredential", "tryUpdateAuthCredentialIfMatches"]) {
  assert.throws(() => store[method]("provider", { access: "secret" }), /disables accounts/);
}
assert.deepEqual(store.listAuthCredentials(), []);
assert.equal(store.getOAuth("provider"), null);
assert.equal(store.getApiKey("provider"), null);
await assert.rejects(fetch("https://example.invalid/telemetry"), /blocked unconfigured endpoint/);
console.log("omp auth and network guards passed");
