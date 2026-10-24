import { describe, expect, it } from "vitest";
import { modelsForThread, parseModelListResponse } from "./modelListResponse";

it("limits Claude thread models to its registry provider", () => {
  const models = parseModelListResponse({ result: { data: [
    { id: "a/one", model: "a/one", providerId: "a", engines: ["pi", "claude"] },
    { id: "b/two", model: "b/two", providerId: "b", engines: ["pi", "claude"] },
    { id: "a/three", model: "a/three", providerId: "a", engines: ["codex"] },
  ] } });
  const thread = { id: "thread", name: "Thread", updatedAt: 1, engine: "claude" as const, modelProvider: "a" };
  expect(modelsForThread(models, thread).map(m => m.id)).toEqual(["a/one"]);
  expect(modelsForThread(models, { ...thread, engine: "pi" })).toBe(models);
  expect(modelsForThread(models, undefined)).toBe(models);
});

describe("parseModelListResponse", () => {
  it("uses displayName when present", () => {
    const response = {
      result: {
        data: [
          { id: "m1", model: "gpt-5.3-codex-spark", displayName: "GPT-5.3-Codex-Spark" },
        ],
      },
    };
    const [model] = parseModelListResponse(response);
    expect(model.displayName).toBe("GPT-5.3-Codex-Spark");
  });

  it("uses the raw model slug when displayName is missing", () => {
    const response = {
      result: {
        data: [{ id: "m1", model: "gpt-5.3-codex" }],
      },
    };
    const [model] = parseModelListResponse(response);
    expect(model.displayName).toBe("gpt-5.3-codex");
  });

  it("uses the raw model slug when displayName is an empty string", () => {
    const response = {
      result: {
        data: [{ id: "m1", model: "gpt-5.1-codex-mini", displayName: "" }],
      },
    };
    const [model] = parseModelListResponse(response);
    expect(model.displayName).toBe("gpt-5.1-codex-mini");
  });

  it("preserves displayName when it equals the model slug", () => {
    const response = {
      result: {
        data: [{ id: "m1", model: "gpt-5.3-codex", displayName: "gpt-5.3-codex" }],
      },
    };
    const [model] = parseModelListResponse(response);
    expect(model.displayName).toBe("gpt-5.3-codex");
  });

  it("preserves displayName when it differs from the slug", () => {
    const response = {
      result: {
        data: [
          { id: "m1", model: "gpt-5.3-codex-spark", displayName: "GPT-5.3-Codex-Spark" },
          { id: "m2", model: "gpt-5.2-codex", displayName: "gpt-5.2-codex" },
        ],
      },
    };
    const models = parseModelListResponse(response);
    expect(models[0].displayName).toBe("GPT-5.3-Codex-Spark");
    expect(models[1].displayName).toBe("gpt-5.2-codex");
  });
});
