// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { AgentActivity, AgentsPanel } from "./AgentsPanel";
import { AgentsContext, agentGroups, runningAgents } from "./agents";
import { getSubagentTranscript } from "@services/tauri";
import type { AgentNode, ConversationItem } from "@/types";
import { buildConversationItem } from "@utils/threadItems";

vi.mock("@services/tauri", () => ({ getSubagentTranscript: vi.fn() }));
vi.mock("@/features/messages/components/Messages", () => ({ Messages: ({ items }: { items: ConversationItem[] }) => <div>{items.map(i => <p key={i.id}>{i.kind === "message" ? i.text : i.kind === "tool" ? i.output : ""}</p>)}</div> }));
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.useRealTimers(); });
const node: AgentNode = { id: "call:agent:0", agent: "Researcher", model: "test/model", status: "running", startedAt: Date.now() - 3000, usage: { input: 10, output: 2, cost: .01 }, children: [] };
const item = (nodes: AgentNode[]): ConversationItem => ({ id: "call", kind: "tool", toolType: "collabToolCall", title: "Agents", detail: "", agentsTree: nodes });
const view = (items: ConversationItem[]) => <AgentsContext.Provider value={{ workspaceId: "ws", threadId: "main", itemsByThread: { main: items }, policy: { enabled: true, maxConcurrent: 2, maxPerTurn: 4, maxDepth: 3, allowedModels: [], defaultModel: null, roles: [] } }}><AgentsPanel onSelect={vi.fn()} /></AgentsContext.Provider>;

describe("Agents panel", () => {
  it("retries a failed read without starting an agent", async () => {
    vi.mocked(getSubagentTranscript).mockRejectedValueOnce(new Error("Read interrupted"))
      .mockResolvedValue({ messages: [{ role: "assistant", content: "Recovered result" }], truncated: false });
    render(view([item([{ ...node, status: "failed", errorMessage: "Process exited" }])]));
    fireEvent.click(screen.getByRole("button", { name: /Researcher/ }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Read interrupted");
    fireEvent.click(screen.getByRole("button", { name: "Retry transcript" }));
    expect(await screen.findByText("Recovered result")).toBeTruthy();
    expect(getSubagentTranscript).toHaveBeenCalledTimes(2);
  });
  it("ignores late reads when switching tasks and never requests the old agent from the new task", async () => {
    let resolve!: (value: Awaited<ReturnType<typeof getSubagentTranscript>>) => void;
    const load = vi.fn(() => new Promise<Awaited<ReturnType<typeof getSubagentTranscript>>>(done => { resolve = done; }));
    const scoped = (threadId: string) => <AgentsContext.Provider value={{ workspaceId: "space", threadId, itemsByThread: { [threadId]: [item([node])] } }}><AgentActivity loadTranscript={load} /></AgentsContext.Provider>;
    const { rerender } = render(scoped("first"));
    fireEvent.click(screen.getByRole("button", { name: /Researcher/ }));
    rerender(scoped("second"));
    await act(async () => resolve({ messages: [{ role: "assistant", content: "Old task answer" }], truncated: false }));
    expect(screen.queryByText("Old task answer")).toBeNull();
    expect(screen.getByRole("button", { name: /Researcher/ })).toBeTruthy();
    expect(load.mock.calls).toHaveLength(1);
  });
  it("does not overlap polls, fetches a final snapshot, and stops polling a completed selection", async () => {
    vi.useFakeTimers();
    let resolve!: (value: Awaited<ReturnType<typeof getSubagentTranscript>>) => void;
    vi.mocked(getSubagentTranscript).mockImplementationOnce(() => new Promise(done => { resolve = done; }))
      .mockResolvedValue({ messages: [{ role: "assistant", content: "Final snapshot" }], truncated: false });
    const other = { ...node, id: "other", agent: "Other" };
    const { rerender } = render(view([item([node, other])]));
    fireEvent.click(screen.getByRole("button", { name: /Researcher/ }));
    await act(async () => { await vi.advanceTimersByTimeAsync(12000); });
    expect(getSubagentTranscript).toHaveBeenCalledTimes(1);
    await act(async () => resolve({ messages: [], truncated: false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(getSubagentTranscript).toHaveBeenCalledTimes(2);
    rerender(view([item([{ ...node, status: "completed" }, other])]));
    await act(async () => {});
    expect(screen.getByText("Final snapshot")).toBeTruthy();
    expect(getSubagentTranscript).toHaveBeenCalledTimes(3);
    await act(async () => { await vi.advanceTimersByTimeAsync(30000); });
    expect(getSubagentTranscript).toHaveBeenCalledTimes(3);
  });
  it("shows policy when empty", () => {
    render(view([]));
    expect(screen.getByText("No subagents in this thread yet")).toBeTruthy();
    expect(screen.getByText(/2 concurrent · 4 per turn · depth 3/)).toBeTruthy();
  });
  it("does not invent elapsed time for interrupted agents with an unknown end", () => {
    render(view([item([{ ...node, status: "aborted", endedAt: null }])]));
    expect(screen.getByText("aborted · elapsed unavailable")).toBeTruthy();
  });
  it("distinguishes unavailable cost from a known zero", () => {
    const { rerender } = render(view([item([{ ...node, usage: { input: 10, output: 2, cost: null } }])]));
    expect(screen.getByText("12 tokens · cost unavailable")).toBeTruthy();
    expect(screen.queryByText(/\$0\.0000/)).toBeNull();
    rerender(view([item([{ ...node, usage: { input: 10, output: 2, cost: 0 } }])]));
    expect(screen.getByText("12 tokens · $0.0000")).toBeTruthy();
  });
  it("updates nested nodes and badges and loads only the selected transcript", async () => {
    const child = { ...node, id: "call:agent:0/nested:agent:0", agent: "Reviewer" };
    const { rerender } = render(view([item([{ ...node, children: [child] }])]));
    expect(screen.getByLabelText("2 running subagents").textContent).toBe("2");
    expect(getSubagentTranscript).not.toHaveBeenCalled();
    vi.mocked(getSubagentTranscript).mockResolvedValue({ messages: [{ role: "assistant", content: [{ type: "text", text: "Nested answer" }] }], truncated: true });
    fireEvent.click(screen.getByRole("button", { name: /Reviewer/ }));
    expect(await screen.findByText("Nested answer")).toBeTruthy();
    expect(getSubagentTranscript).toHaveBeenCalledWith("ws", "main", child.id);
    expect(screen.getByText(/Showing the retained transcript/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Back to agents/ }));
    rerender(view([item([{ ...node, status: "failed", errorMessage: "Child crashed", children: [] }])]));
    await waitFor(() => expect(screen.queryByLabelText("2 running subagents")).toBeNull());
    expect(screen.getByRole("alert").textContent).toBe("Child crashed");
  });
  it("maps native Codex thread IDs recursively without cycles or duplicate wait calls", () => {
    const collab = (id: string, child: string, status = "running"): ConversationItem => ({ id, kind: "tool", toolType: "collabToolCall", title: "spawn", detail: "", collabStatuses: [{ threadId: child, status }] });
    const groups = agentGroups([collab("spawn", "child"), collab("wait", "child", "completed")], { child: [collab("nested", "grandchild")], grandchild: [collab("cycle", "child")] }, new Set(["main"]));
    expect(groups[0].nodes).toHaveLength(1);
    expect(groups[0].nodes[0].id).toBe("child");
    expect(groups[0].nodes[0].status).toBe("completed");
    expect(groups[0].nodes[0].children[0].id).toBe("grandchild");
    expect(runningAgents(groups[0].nodes)).toBe(1);
  });
  it("groups by protocol turn even when steering adds user messages", () => {
    const call = item([node]);
    if (call.kind !== "tool") throw new Error("fixture must be a tool");
    const groups = agentGroups([
      { id: "user-1", kind: "message", role: "user", text: "Start" },
      { ...call, turnId: "turn-1" },
      { id: "steer", kind: "message", role: "user", text: "Also check" },
      { ...call, id: "second-call", turnId: "turn-1", agentsTree: [{ ...node, id: "second:agent:0" }] },
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].nodes).toHaveLength(2);
  });
  it("renders Claude Task metadata and explains the partial transcript", async () => {
    const task = buildConversationItem({ id: "task", type: "collabAgentToolCall", tool: "spawnAgent", status: "completed", receiverThreadIds: ["claude:agent:task"], agentsTree: [{ id: "claude:agent:task", agent: "Explore", task: "Inspect parser", status: "completed", lastMessage: "Found parser", children: [] }] });
    expect(task).toBeTruthy();
    const partial = { messages: [{ role: "assistant", content: "Found parser" }], truncated: true, note: "Claude Code exposes Task tool updates, not a separate subagent transcript." };
    vi.mocked(getSubagentTranscript).mockResolvedValue(partial);
    render(view([task!]));
    expect(screen.getByText("Inspect parser")).toBeTruthy();
    expect(screen.getByText("Found parser")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Explore/ }));
    expect(await screen.findByText(partial.note)).toBeTruthy();
    expect(getSubagentTranscript).toHaveBeenCalledWith("ws", "main", "claude:agent:task");
  });
});
