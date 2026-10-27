/** @vitest-environment jsdom */
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConversationItem, McpServerEntry, ThreadTokenUsage } from "@/types";
import { connectorRequest } from "@services/tauri";
import { contextUsage, reportedFiles, useCodeSessionData } from "./useCodeSessionData";
vi.mock("@services/tauri", () => ({ connectorRequest: vi.fn() }));
const breakdown = { totalTokens: 100, inputTokens: 50, cachedInputTokens: 20, outputTokens: 10, reasoningOutputTokens: 4 };
const usage: ThreadTokenUsage = { last: breakdown, total: { ...breakdown, totalTokens: 5000 }, modelContextWindow: 1000 };
beforeEach(() => vi.clearAllMocks());

describe("session data", () => {
  it("keeps reported totals, separates cache correctly, and avoids synthetic ACP categories", () => {
    expect(contextUsage(usage, "pi-rpc")).toMatchObject({ used: 100, input: 50, cached: 20, output: 10, other: 20, percent: 10 });
    expect(contextUsage(usage, "codex-app-server")).toMatchObject({ used: 100, input: 30, cached: 20 });
    expect(contextUsage(usage, "acp")).toMatchObject({ used: 100, detailed: false, percent: 10 });
    expect(contextUsage({ ...usage, modelContextWindow: null }, "pi-rpc")?.percent).toBeNull();
    expect(contextUsage(null, "pi-rpc")).toBeNull();
  });
  it("lists only reported files, excluding failed/deleted/virtual changes and user links", () => {
    const tool = (id: string, path: string, status = "completed", kind = "update"): ConversationItem => ({ id, kind: "tool", toolType: "fileChange", title: "Edit", detail: "", status, changes: [{ path, kind }] });
    const items: ConversationItem[] = [tool("a", "src/a.ts"), tool("b", "src/a.ts"), tool("c", "src/b.ts", "failed"), tool("d", "old.ts", "completed", "delete"), tool("e", "xd://mcp__browser"), tool("pending", "pending.ts", "inProgress"),
      { id: "m", kind: "message", role: "assistant", text: "[Report](reports/result.md) [website](https://example.com)" },
      { id: "u", kind: "message", role: "user", text: "[Unrelated](src/user.ts)" }];
    expect(reportedFiles(items, "/workspace")).toEqual(["src/a.ts", "reports/result.md"]);
  });
  it("reads configuration without probing, filters disabled entries, and ignores stale workspaces", async () => {
    let oldResolve!: (entries: McpServerEntry[]) => void;
    vi.mocked(connectorRequest).mockImplementationOnce(() => new Promise(resolve => { oldResolve = resolve as typeof oldResolve; })).mockResolvedValueOnce([{ id: "current", enabled: true }, { id: "off", enabled: false }]);
    const { result, rerender } = renderHook(({ id }) => useCodeSessionData(id, "config", []), { initialProps: { id: "old" } });
    rerender({ id: "new" });
    await waitFor(() => expect(result.current.connectors).toEqual([{ id: "current", enabled: true }]));
    await act(async () => oldResolve([{ id: "stale", enabled: true }]));
    expect(result.current.connectors.map(entry => entry.id)).toEqual(["current"]);
    expect(vi.mocked(connectorRequest).mock.calls.map(([params]) => params)).toEqual([{ action: "list", workspaceId: "old" }, { action: "list", workspaceId: "new" }]);
  });
  it("refreshes changed configuration and exposes fetch failure", async () => {
    vi.mocked(connectorRequest).mockResolvedValueOnce([]).mockRejectedValueOnce(new Error("Configuration unavailable"));
    const { result, rerender } = renderHook(({ revision }) => useCodeSessionData("w", revision, []), { initialProps: { revision: "one" } });
    await waitFor(() => expect(result.current.connectorsLoading).toBe(false));
    rerender({ revision: "two" });
    await waitFor(() => expect(result.current.connectorError).toContain("Configuration unavailable"));
  });
});
