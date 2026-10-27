import { describe, expect, it } from "vitest";
import type { ConversationItem } from "../../../types";
import { buildToolSummary, statusToneFromText } from "./messageRenderUtils";

function makeToolItem(
  overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>,
): Extract<ConversationItem, { kind: "tool" }> {
  return {
    id: "tool-1",
    kind: "tool",
    toolType: "webSearch",
    title: "Web search",
    detail: "codex monitor",
    status: "completed",
    output: "",
    ...overrides,
  };
}

describe("messageRenderUtils", () => {
  it("renders web search as searching while in progress", () => {
    const summary = buildToolSummary(makeToolItem({ status: "inProgress" }), "");
    expect(summary.label).toBe("searching");
    expect(summary.value).toBe("codex monitor");
  });

  it("renders mcp search calls as searching while in progress", () => {
    const summary = buildToolSummary(
      makeToolItem({
        toolType: "mcpToolCall",
        title: "Tool: web / search_query",
        detail: '{\n  "query": "codex monitor"\n}',
        status: "inProgress",
      }),
      "",
    );
    expect(summary.label).toBe("searching");
    expect(summary.value).toBe("codex monitor");
  });

  it("classifies camelCase inProgress as processing", () => {
    expect(statusToneFromText("inProgress")).toBe("processing");
  });

  it("renders collab tool calls with nickname and role", () => {
    const summary = buildToolSummary(
      makeToolItem({
        toolType: "collabToolCall",
        title: "Collab: wait",
        detail: "From thread-parent → thread-child",
        status: "completed",
        output: "Robie [explorer]: completed",
        collabReceivers: [
          {
            threadId: "thread-child",
            nickname: "Robie",
            role: "explorer",
          },
        ],
      }),
      "",
    );
    expect(summary.label).toBe("waited for");
    expect(summary.value).toBe("Robie [explorer]");
    expect(summary.output).toContain("Robie [explorer]: completed");
  });

  it.each([
    { name: "nickname", nickname: " Scout ", role: "reviewer", expected: "Scout" },
    { name: "role", role: " reviewer ", expected: "reviewer" },
    { name: "no metadata", expected: "Sub-agent" },
  ])("labels one synthetic receiver using $name", ({ expected, nickname, role }) => {
    const summary = buildToolSummary(makeToolItem({
      toolType: "collabToolCall",
      title: "Collab: spawnAgent",
      collabReceiver: { threadId: "tool-1:agent:0", nickname, role },
    }), "");
    expect(summary.value).toBe(expected);
  });

  it("labels multiple synthetic receivers from the matching agent tree node", () => {
    const summary = buildToolSummary(makeToolItem({
      toolType: "collabToolCall",
      title: "Collab: spawnAgent",
      status: "failed",
      collabReceivers: [{ threadId: "tool-1:agent:0" }, { threadId: "tool-1:agent:1" }],
      agentsTree: [
        { id: "tool-1:agent:1", agent: "writer", status: "failed", children: [] },
        { id: "tool-1:agent:0", agent: "scout", status: "failed", children: [] },
      ],
    }), "");
    expect(summary.value).toBe("scout +1");
    expect(summary.label).toBe("spawn failed");
  });

  it.each([
    { role: undefined, expected: "thread-child" },
    { role: "reviewer", expected: "thread-child [reviewer]" },
  ])("preserves real receiver identifiers ($expected)", ({ role, expected }) => {
    const summary = buildToolSummary(makeToolItem({
      toolType: "collabToolCall",
      title: "Collab: spawnAgent",
      collabReceivers: [{ threadId: "thread-child", role }],
    }), "");
    expect(summary.value).toBe(expected);
    expect(summary.label).toBe("spawned");
  });

  it("keeps the in-progress spawn label", () => {
    expect(buildToolSummary(makeToolItem({
      toolType: "collabToolCall", title: "Collab: spawnAgent", status: "inProgress",
    }), "").label).toBe("spawning");
  });
});
