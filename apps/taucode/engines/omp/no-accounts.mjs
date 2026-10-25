// Omp stores auth_credentials in agent.db. Block reads, mutations and OAuth at
// the credential-store boundary, before the CLI imports its auth implementation.
import { SqliteAuthCredentialStore } from "@oh-my-pi/pi-ai";
import { readFileSync } from "node:fs";
import { join } from "node:path";
if (!process.env.PI_CODING_AGENT_DIR) throw new Error("Taucode requires an isolated config dir");
const denied = () => { throw new Error("Taucode disables accounts and credential storage. Use a provider instead."); };
for (const method of ["replaceAuthCredentials", "upsertAuthCredential", "updateAuthCredential", "tryUpdateAuthCredentialIfMatches", "saveOAuth", "saveApiKey"]) SqliteAuthCredentialStore.prototype[method] = denied;
SqliteAuthCredentialStore.prototype.listAuthCredentials = () => [];
SqliteAuthCredentialStore.prototype.getOAuth = () => null;
SqliteAuthCredentialStore.prototype.getApiKey = () => null;
// No marketplace, release, remote model discovery or telemetry requests. Model
// traffic is confined to the endpoints generated from taucode's registry.
const origins = new Set(JSON.parse(readFileSync(join(process.env.PI_CODING_AGENT_DIR, "taucode/network.json"), "utf8")));
const originalFetch = globalThis.fetch;
globalThis.fetch = (input, init) => {
  const url = new URL(typeof input === "string" || input instanceof URL ? input : input.url);
  if (!origins.has(url.origin)) return Promise.reject(new Error(`Taucode blocked unconfigured endpoint: ${url.origin}`));
  return originalFetch(input, init);
};
