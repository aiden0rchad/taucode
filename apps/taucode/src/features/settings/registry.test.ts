import { expect, it } from "vitest";
import type { ProviderEntry } from "@/types";
import { codexReady, defaultAgentPolicy, formatModelRef, policyError, providerSlug, setAgentMode } from "./registry";

const provider: ProviderEntry = { id: "setup-old-id", name: "LiteLLM", kind: "openai-compatible", apiStyle: "responses", enabled: true, auth: { type: "none" }, models: [{ id: "org/opus", displayName: "Claude Opus 5.5", enabled: true }] };
it("switches command modes without discarding custom roles or model choices", () => {
  const chain = setAgentMode(defaultAgentPolicy, "chain-of-command");
  expect(chain.subagents.maxDepth).toBe(2);
  expect(policyError(chain, [])).toBeNull();
  const colonel = chain.subagents.roles.find(role => role.name === "colonel")!;
  colonel.model = { providerId: provider.id, modelId: "org/opus" };
  const single = setAgentMode(chain, "single");
  expect(single.subagents.enabled).toBe(false);
  const restored = setAgentMode(single, "chain-of-command");
  expect(restored.subagents.roles).toEqual(chain.subagents.roles);
  expect(restored.subagents.roles.filter(role => role.name === "colonel")).toHaveLength(1);
  expect(policyError(restored, [provider])).toBeNull();
  expect(policyError({ ...restored, subagents: { ...restored.subagents, maxDepth: 1 } }, [provider])).toMatch(/depth at least 2/);
  expect(policyError({ ...restored, subagents: { ...restored.subagents, roles: [] } }, [provider])).toMatch(/colonel, major and enlisted/);
  expect(defaultAgentPolicy.subagents.roles.map(role => role.name)).toEqual(["scout", "planner", "worker"]);
});
it("formats existing IDs, nested model IDs, missing models and removed providers", () => {
  const settings = { providers: [provider] };
  expect(formatModelRef(settings, "setup-old-id/org/opus")).toEqual({ name: "Claude Opus 5.5", title: "LiteLLM · Claude Opus 5.5" });
  expect(formatModelRef(settings, { providerId: provider.id, modelId: "org/opus" }).name).toBe("Claude Opus 5.5");
  expect(formatModelRef(settings, "setup-old-id/new").title).toBe("LiteLLM · new");
  expect(formatModelRef(settings, "removed/org/model").name).toBe("org/model");
  expect(formatModelRef(settings, "legacy-model").name).toBe("legacy-model");
});
it("makes readable, unique provider slugs without changing existing IDs", () => {
  expect(providerSlug(" LiteLLM ", [provider])).toBe("litellm");
  expect(providerSlug("LiteLLM", [{ ...provider, id: "litellm" }, { ...provider, id: "litellm-2" }])).toBe("litellm-3");
  expect(providerSlug("My Local Proxy!", [])).toBe("my-local-proxy");
  expect(providerSlug("✨", [])).toBe("provider");
  const long = "a".repeat(40);
  expect(providerSlug(long, [{ ...provider, id: "a".repeat(32) }])).toBe("a".repeat(30) + "-2");
  expect(providerSlug("LiteLLM", [{ ...provider, id: "litellm" }, { ...provider, id: "litellm_2" }])).toBe("litellm-3");
});
it("requires an enabled Codex-compatible provider and model", () => {
  expect(codexReady({ providers: [provider] })).toBe(true);
  for (const p of [{ ...provider, enabled: false }, { ...provider, models: [] }, { ...provider, apiStyle: "chat-completions" as const }]) expect(codexReady({ providers: [p] })).toBe(false);
});
