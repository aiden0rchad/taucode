// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { defaultAgentPolicy } from "@settings/registry";
import { threadAgentPolicy } from "@services/tauri";
import { useConversationPolicy } from "./useConversationPolicy";

vi.mock("@services/tauri", () => ({ threadAgentPolicy: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const base = defaultAgentPolicy;
const custom = { ...base, subagents: { ...base.subagents, maxRunMinutes: 7 } };

it("loads, saves only this conversation, and restores project inheritance", async () => {
  vi.mocked(threadAgentPolicy).mockResolvedValue({ policy: base, overridden: false });
  const { result } = renderHook(() => useConversationPolicy("project", "one", true, base));
  expect(result.current.loading).toBe(true);
  await waitFor(() => expect(result.current.loading).toBe(false));
  vi.mocked(threadAgentPolicy).mockResolvedValue({ policy: custom, overridden: true });
  await act(() => result.current.change(custom));
  expect(threadAgentPolicy).toHaveBeenLastCalledWith("project", "one", "set", custom.subagents);
  expect(result.current.overridden).toBe(true);
  expect(result.current.policy).toEqual(custom);
  vi.mocked(threadAgentPolicy).mockResolvedValue({ policy: base, overridden: false });
  await act(() => result.current.change());
  expect(threadAgentPolicy).toHaveBeenLastCalledWith("project", "one", "reset", undefined);
  expect(result.current.overridden).toBe(false);
});

it("discards late reads and saves after switching conversations", async () => {
  let resolveRead!: (value: { policy: typeof base; overridden: boolean }) => void;
  vi.mocked(threadAgentPolicy).mockImplementationOnce(() => new Promise(resolve => { resolveRead = resolve; })).mockResolvedValue({ policy: base, overridden: false });
  const { result, rerender } = renderHook(({ id }) => useConversationPolicy("project", id, true, base), { initialProps: { id: "one" } });
  rerender({ id: "two" });
  await waitFor(() => expect(result.current.loading).toBe(false));
  await act(async () => resolveRead({ policy: custom, overridden: true }));
  expect(result.current.policy).toEqual(base);
  let resolveSave!: typeof resolveRead;
  vi.mocked(threadAgentPolicy).mockImplementationOnce(() => new Promise(resolve => { resolveSave = resolve; }));
  let save!: Promise<void>;
  act(() => { save = result.current.change(custom); });
  rerender({ id: "three" });
  await waitFor(() => expect(result.current.loading).toBe(false));
  await act(async () => { resolveSave({ policy: custom, overridden: true }); await save; });
  expect(result.current.overridden).toBe(false);
  expect(result.current.policy).toEqual(base);
});

it("waits for a connection, reports load errors, and refreshes changed project defaults", async () => {
  vi.mocked(threadAgentPolicy).mockRejectedValueOnce("Cannot read conversation team").mockResolvedValue({ policy: custom, overridden: false });
  const { result, rerender } = renderHook(({ connected, policy }) => useConversationPolicy("project", "one", connected, policy), { initialProps: { connected: false, policy: base } });
  expect(threadAgentPolicy).not.toHaveBeenCalled();
  rerender({ connected: true, policy: base });
  await waitFor(() => expect(result.current.error).toContain("Cannot read"));
  rerender({ connected: true, policy: custom });
  await waitFor(() => expect(result.current.policy).toEqual(custom));
  expect(threadAgentPolicy).toHaveBeenCalledTimes(2);
});
