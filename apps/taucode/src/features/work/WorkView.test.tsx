// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, ReasoningCapability, WorkState, WorkTask } from "@/types";
import { WorkView } from "./WorkView";
import { workRequest, workOutput, workAttachment, pickWorkAttachments, browserRequest } from "@services/tauri";

const events = vi.hoisted(() => ({ listener: (_event: unknown) => {}, importsChanged: () => {} }));
vi.mock("@services/tauri", () => ({ subscribeBrowser: vi.fn().mockResolvedValue(() => {}), browserRequest: vi.fn().mockResolvedValue(undefined),
  workRequest: vi.fn(), workOutput: vi.fn(), workAttachment: vi.fn(),
  pickWorkspacePath: vi.fn(), pickWorkspacePaths: vi.fn(), pickWorkAttachments: vi.fn(),
  checkpointRequest: vi.fn(),
}));
vi.mock("@services/events", () => ({ subscribeMenuViewAction: () => () => {},
  subscribeAppServerEvents: (listener: typeof events.listener) => { events.listener = listener; return () => {}; },
  subscribeImportsChanged: (listener: () => void) => { events.importsChanged = listener; return () => {}; },
}));
vi.mock("@services/dragDrop", () => ({ subscribeWindowDragDrop: () => () => {} }));
vi.mock("@/features/chat/ModeShell", () => ({ ModeSwitcher: () => <nav>Code Chat Work</nav> }));
vi.mock("@/features/checkpoints/Checkpoints", () => ({ Checkpoints: () => null, RewindAction: () => <button>Rewind…</button> }));
const settings = { providers: [{ id: "local", name: "Local", enabled: true, models: [{ id: "test", enabled: true }] }] } as AppSettings;
let state: WorkState;
function task(id: string, status: WorkTask["status"]): WorkTask {
  return { id, spaceId: "space", threadId: id, title: id, brief: "Make a brief", attachments: [], model: "local/test", accessMode: "ask", status, plan: [], outputs: [], pending: [], error: null };
}
function readHistory(turns: Record<string, unknown>[]) {
  const request = vi.mocked(workRequest).getMockImplementation()!;
  vi.mocked(workRequest).mockImplementation((action, params) => action === "read"
    ? Promise.resolve({ result: { thread: { turns } } }) : request(action, params));
}
beforeEach(() => {
  localStorage.clear();
  Element.prototype.scrollIntoView = vi.fn();
  state = { spaces: [{ id: "space", kind: "work", name: "Research", path: "/synthetic", readOnlyFolders: [] }], tasks: [], concurrency: 2 };
  vi.mocked(workRequest).mockImplementation(async (action, params) => {
    if (action === "list") return structuredClone(state);
    if (action === "preview") return [];
    if (action === "create") { const created = { ...task("new-task", "queued"), ...params, threadId: null } as WorkTask; state.tasks.push(created); return created; }
    if (action === "read") return { result: { thread: { id: params?.taskId, turns: [] } } };
    if (action === "respond") { state.tasks[0].status = "running"; state.tasks[0].pending = []; }
    if (action === "rename") state.tasks.find(t => t.id === params?.taskId)!.title = String(params?.title).trim();
    if (action === "stop-all") state.tasks.forEach(t => { t.status = "stopped"; });
    return {};
  });
  vi.mocked(workOutput).mockResolvedValue();
  vi.mocked(workAttachment).mockResolvedValue(".taucode/uploads/copied-notes.pdf");
  vi.mocked(pickWorkAttachments).mockResolvedValue(["/synthetic-source/notes.pdf"]);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("Work mode", () => {
  it("shows task-owned preview controls and preserves the follow-up draft across actions", async () => {
    state.tasks = [task("First", "done"), task("Second", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    let previews = [{ id: "server", workspaceId: "space", threadId: "First", command: "python3 -m http.server", label: "Dashboard", cwd: "/synthetic", pid: 123, status: "running", startedAt: Date.now(), endedAt: null as number | null, exitCode: null, output: "Listening on loopback", outputTruncated: false, urls: ["http://127.0.0.1:8765/"] }];
    vi.mocked(workRequest).mockImplementation(async (action, params) => {
      if (action !== "preview") return request(action, params);
      if (params?.taskId !== "First") return [];
      if (params.action === "stop") previews[0] = { ...previews[0], status: "stopped", endedAt: Date.now() };
      if (params.action === "restart") previews.unshift({ ...previews[0], id: "restarted", status: "running", endedAt: null });
      if (params.action === "clearFinished") previews = previews.filter(p => p.status === "running");
      return structuredClone(previews);
    });
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "First" }));
    const draft = screen.getByRole("textbox", { name: "Message task" });
    fireEvent.change(draft, { target: { value: "Keep this follow-up" } });
    fireEvent.click(await screen.findByRole("button", { name: "View previews" }));
    const panel = screen.getByRole("region", { name: "Background tasks" });
    expect(within(panel).queryByRole("textbox")).toBeNull();
    fireEvent.click(within(panel).getByText("Dashboard"));
    expect(within(panel).getByLabelText("Output for Dashboard").textContent).toContain("Listening on loopback");
    fireEvent.click(within(panel).getByRole("button", { name: "http://127.0.0.1:8765/" }));
    await waitFor(() => expect(browserRequest).toHaveBeenCalledWith(expect.objectContaining({ action: "navigate", url: "http://127.0.0.1:8765/" })));
    fireEvent.click(screen.getByRole("tab", { name: "Previews" }));
    fireEvent.click(within(panel).getByRole("button", { name: "Stop Dashboard" }));
    await waitFor(() => expect(within(panel).queryByRole("button", { name: "Stop Dashboard" })).toBeNull());
    fireEvent.click(within(panel).getByText("Finished"));
    fireEvent.click(within(panel).getByText("Dashboard"));
    fireEvent.click(within(panel).getByRole("button", { name: "Restart" }));
    await within(panel).findByRole("button", { name: "Stop Dashboard" });
    expect(workRequest).toHaveBeenCalledWith("preview", { taskId: "First", action: "restart", id: "server" });
    expect(draft).toHaveProperty("value", "Keep this follow-up");
    fireEvent.click(screen.getByRole("button", { name: "Second" }));
    expect(screen.queryByLabelText("Output for Dashboard")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "First" }));
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "Keep this follow-up");
    expect(workRequest).not.toHaveBeenCalledWith("follow-up", expect.anything());
  });
  it("opens nested agent transcripts in the task panel while preserving drafts and task scope", async () => {
    state.tasks = [task("First", "done"), task("Second", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    let resolveTranscript!: (value: unknown) => void;
    vi.mocked(workRequest).mockImplementation((action, params) => {
      if (action === "subagent-transcript") return new Promise(resolve => { resolveTranscript = resolve; });
      if (action === "read") return Promise.resolve({ result: { thread: { turns: [{ id: "turn", items: [{
        id: "delegate", type: "collabAgentToolCall", tool: "spawnAgent", status: "completed", receiverThreadIds: [], agentsTree: [{
          id: "delegate:agent:0", agent: "Researcher", status: "completed", children: [{ id: "delegate:agent:0/review:agent:0", agent: "Reviewer", status: "failed", errorMessage: "Check interrupted", children: [] }],
        }],
      }] }] } } });
      return request(action, params);
    });
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "First" }));
    const draft = screen.getByRole("textbox", { name: "Message task" });
    fireEvent.change(draft, { target: { value: "Keep this draft" } });
    fireEvent.click((await screen.findAllByRole("button", { name: "View agents" }))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "View Reviewer transcript, failed" }));
    expect(workRequest).toHaveBeenCalledWith("subagent-transcript", { taskId: "First", nodeId: "delegate:agent:0/review:agent:0" });
    await act(async () => resolveTranscript({ messages: [{ role: "assistant", content: "Partial review evidence" }], truncated: false }));
    expect(await screen.findByText("Partial review evidence")).toBeTruthy();
    expect(draft).toHaveProperty("value", "Keep this draft");
    fireEvent.click(screen.getByRole("button", { name: "Second" }));
    expect(screen.queryByText("Partial review evidence")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "First" }));
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "Keep this draft");
    expect(workRequest).not.toHaveBeenCalledWith("follow-up", expect.anything());
  });
  it("does not offer unavailable transcripts for imported tasks", async () => {
    state.tasks = [{ ...task("Imported", "done"), importId: "copy" }];
    readHistory([{ id: "turn", items: [{ id: "delegate", type: "collabAgentToolCall", tool: "spawnAgent", status: "completed", receiverThreadIds: [], agentsTree: [{ id: "child", agent: "Researcher", status: "completed", children: [] }] }] }]);
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Imported" }));
    await screen.findByRole("log", { name: "Task progress" });
    expect(screen.queryByRole("button", { name: "View agents" })).toBeNull();
    expect(workRequest).not.toHaveBeenCalledWith("subagent-transcript", expect.anything());
  });
  it("opens loaded history at the latest result and follows output only while near the bottom", async () => {
    state.tasks = [task("First", "running"), task("Second", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    let resolveHistory!: (value: unknown) => void;
    const history = new Promise(resolve => { resolveHistory = resolve; });
    vi.mocked(workRequest).mockImplementation((action, params) => action === "read" && params?.taskId === "First" ? history : request(action, params));
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "First" }));
    const progress = screen.getByRole("log", { name: "Task progress" });
    Object.defineProperties(progress, { scrollHeight: { configurable: true, writable: true, value: 2000 }, clientHeight: { configurable: true, value: 500 } });
    await act(async () => { resolveHistory({ result: { thread: { turns: [{ id: "turn", items: [{ id: "answer", type: "agentMessage", text: "Latest result" }] }] } } }); });
    expect(await screen.findByText("Latest result")).toBeTruthy();
    expect(progress.scrollTop).toBe(2000);
    progress.scrollTop = 300;
    fireEvent.scroll(progress);
    expect(screen.getByRole("button", { name: "Jump to latest" })).toBeTruthy();
    Object.defineProperty(progress, "scrollHeight", { value: 2500 });
    act(() => events.listener({ workspace_id: "space", message: { method: "item/agentMessage/delta", params: { threadId: "First", itemId: "answer", delta: " More output." } } }));
    expect(progress.scrollTop).toBe(300);
    fireEvent.click(screen.getByRole("button", { name: "Jump to latest" }));
    expect(progress.scrollTop).toBe(2500);
    expect(screen.queryByRole("button", { name: "Jump to latest" })).toBeNull();
    Object.defineProperty(progress, "scrollHeight", { value: 2800 });
    act(() => events.listener({ workspace_id: "space", message: { method: "item/agentMessage/delta", params: { threadId: "First", itemId: "answer", delta: " Finished." } } }));
    expect(progress.scrollTop).toBe(2800);
    progress.scrollTop = 300;
    fireEvent.scroll(progress);
    fireEvent.click(screen.getByRole("button", { name: "Second" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Second" }).getAttribute("aria-current")).toBe("page"));
    expect(progress.scrollTop).toBe(2800);
    expect(screen.queryByRole("button", { name: "Jump to latest" })).toBeNull();
  });
  it("places pending requests after progress and jumps to them from the input callout", async () => {
    state.tasks = [{ ...task("Approval", "needs-input"), pending: [{ id: "approval", method: "item/commandExecution/requestApproval", params: { command: "test-command" } }] }];
    readHistory([{ id: "turn", items: [{ id: "answer", type: "agentMessage", text: "Please review this command." }] }]);
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Approval" }));
    const message = await screen.findByText("Please review this command.");
    const approval = screen.getByRole("region", { name: "Approval required" });
    expect(message.compareDocumentPosition(approval) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const progress = screen.getByRole("log", { name: "Task progress" });
    Object.defineProperties(progress, { scrollHeight: { configurable: true, value: 2000 }, clientHeight: { configurable: true, value: 500 } });
    progress.scrollTop = 100;
    fireEvent.scroll(progress);
    fireEvent.click(screen.getByRole("button", { name: "1 task needs your input" }));
    expect(progress.scrollTop).toBe(2000);
    expect(screen.queryByRole("button", { name: "Jump to latest" })).toBeNull();
    expect(workRequest).not.toHaveBeenCalledWith("respond", expect.anything());
  });
  it("renames a sidebar task without navigating away or losing an unsent draft", async () => {
    state.tasks = [task("Report", "done"), { ...task("Queued", "queued"), threadId: null }];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    const draft = screen.getByRole("textbox", { name: "Message task" });
    fireEvent.change(draft, { target: { value: "Keep this draft" } });
    fireEvent.contextMenu(screen.getByRole("button", { name: "Queued" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    const dialog = screen.getByRole("dialog", { name: "Rename task" });
    const input = within(dialog).getByRole("textbox", { name: "New name" });
    expect(input).toBe(document.activeElement);
    expect(input).toHaveProperty("selectionEnd", "Queued".length);
    fireEvent.change(input, { target: { value: "Dashboard review" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByRole("button", { name: "Dashboard review" });
    expect(workRequest).toHaveBeenCalledWith("rename", { taskId: "Queued", title: "Dashboard review" });
    expect(screen.queryByRole("dialog", { name: "Rename task" })).toBeNull();
    expect(screen.getByRole("button", { name: "Report" }).getAttribute("aria-current")).toBe("page");
    expect(draft).toHaveProperty("value", "Keep this draft");
    expect(workRequest).not.toHaveBeenCalledWith("create", expect.anything());
    expect(workRequest).not.toHaveBeenCalledWith("follow-up", expect.anything());
  });
  it("keeps failed renames editable and updates the selected task through the header menu", async () => {
    state.tasks = [task("Report", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    let rejectRename = true;
    vi.mocked(workRequest).mockImplementation((action, params) => action === "rename" && rejectRename ? Promise.reject("Could not save task name") : request(action, params));
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    fireEvent.click(screen.getByRole("button", { name: "Actions for thread" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    const input = screen.getByRole("textbox", { name: "New name" });
    fireEvent.change(input, { target: { value: "Weekly review" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    const error = await screen.findByRole("alert");
    expect(error.textContent).toBe("Could not save task name");
    expect(error.closest('[role="dialog"]')).not.toBeNull();
    expect(input).toHaveProperty("value", "Weekly review");
    rejectRename = false;
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByRole("button", { name: "Weekly review" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename task" })).toBeNull());
    expect(screen.getByRole("button", { name: "Weekly review" }).getAttribute("aria-current")).toBe("page");
    expect(state.tasks[0].brief).toBe("Make a brief");
  });
  it("cancels renaming with Escape and keeps imported names read-only", async () => {
    state.tasks = [task("Report", "done"), { ...task("Imported", "done"), importId: "copy" }];
    render(<WorkView settings={settings} />);
    fireEvent.contextMenu(await screen.findByRole("button", { name: "Report" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    fireEvent.change(screen.getByRole("textbox", { name: "New name" }), { target: { value: "Discard this" } });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "New name" }), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(workRequest).not.toHaveBeenCalledWith("rename", expect.anything());
    fireEvent.contextMenu(screen.getByRole("button", { name: "Imported" }));
    expect(screen.queryByRole("menuitem", { name: "Rename" })).toBeNull();
    fireEvent.click(screen.getByRole("menuitem", { name: "Open task" }));
    fireEvent.click(screen.getByRole("button", { name: "Actions for thread" }));
    expect(screen.queryByRole("menuitem", { name: "Rename" })).toBeNull();
  });
  it("shows one compact error and retries the failed request with a new model while preserving the draft", async () => {
    const error = 'API error (429): {"type":"chatgpt_quota_paused"}';
    state.spaces[0].readOnlyFolders = ["/reference"];
    state.tasks = [{ ...task("Report", "failed"), error, attachments: ["notes.txt"] }];
    const configured = { ...settings, providers: [...settings.providers!, { ...settings.providers![0], id: "backup", name: "Backup", models: [{ id: "alternate", displayName: "Alternate", enabled: true }] }] };
    readHistory([
      { id: "first", items: [{ id: "original", type: "userMessage", content: [{ type: "text", text: "Make a brief" }] }] },
      { id: "failed", status: "failed", error: { message: error }, items: [{ id: "follow", type: "userMessage", content: [{ type: "text", text: "Check the output\n\nAttached files (inside this space):\nnotes.txt\n\nRead-only folders:\n/reference" }] }] },
    ]);
    render(<WorkView settings={configured} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    await screen.findByRole("button", { name: "Retry" });
    const card = screen.getByRole("region", { name: "Account usage limit reached" });
    expect(card.querySelector("details")).toHaveProperty("open", false);
    expect(screen.getAllByText(error, { exact: true })).toHaveLength(1);
    expect(screen.queryByText(`Turn failed: ${error}`)).toBeNull();
    const draft = screen.getByRole("textbox", { name: "Message task" });
    fireEvent.change(draft, { target: { value: "Keep this unsent note" } });
    fireEvent.click(within(card).getByRole("button", { name: "Change model" }));
    fireEvent.click(await screen.findByRole("option", { name: "Alternate" }));
    await screen.findByRole("button", { name: "Alternate · ?" });
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    fireEvent.click(within(card).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("follow-up", { taskId: "Report", message: "Check the output", accessMode: "ask", model: "backup/alternate", effort: null }));
    expect(draft).toHaveProperty("value", "Keep this unsent note");
    expect(workRequest).not.toHaveBeenCalledWith("create", expect.anything());
  });
  it("retries a startup failure's pending message instead of an older failed turn", async () => {
    state.tasks = [{ ...task("Report", "failed"), error: "Connection failed", pendingMessage: "The new request" }];
    readHistory([{ id: "old", status: "failed", error: { message: "Old error" }, items: [{ id: "old-user", type: "userMessage", content: [{ type: "text", text: "The old request" }] }] }]);
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    await screen.findByText("Turn failed: Old error");
    expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("follow-up", expect.objectContaining({ message: "The new request" })));
  });
  it("does not retry an older request when the failed turn had no user message", async () => {
    state.tasks = [{ ...task("Report", "failed"), error: "Compaction failed" }];
    readHistory([
      { id: "old", items: [{ id: "old-user", type: "userMessage", content: [{ type: "text", text: "An unrelated request" }] }] },
      { id: "compact", status: "failed", error: { message: "Compaction failed" }, items: [] },
    ]);
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    await screen.findByText("An unrelated request");
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(screen.getAllByRole("region", { name: "Request couldn’t complete" })).toHaveLength(1);
  });
  it("keeps the request and unsent draft available after a retry is rejected", async () => {
    state.tasks = [{ ...task("Report", "failed"), error: "Connection failed", pendingMessage: "Retry this request" }];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation((action, params) => action === "follow-up" ? Promise.reject(new Error("No available slot")) : request(action, params));
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Message task" }), { target: { value: "Unsent draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByText("Couldn’t restart the request. Your message is still available.");
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "Unsent draft");
    expect(state.tasks[0].pendingMessage).toBe("Retry this request");
  });
  it("renders plan formatting and accessible step names without Markdown markers", async () => {
    state.tasks = [{ ...task("Report", "done"), plan: [{ step: "Verify **filters** and `count`.", status: "completed" }] }];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    const step = await screen.findByRole("checkbox", { name: "Verify filters and count." });
    expect(step.getAttribute("aria-checked")).toBe("true");
    expect(step.getAttribute("aria-readonly")).toBe("true");
    const plan = screen.getByRole("region", { name: "Plan" });
    expect(within(plan).getByText("filters").tagName).toBe("STRONG");
    expect(within(plan).getByText("count").tagName).toBe("CODE");
    expect(plan.textContent).not.toContain("**");
  });
  it("keeps a starting task cancellable and preserves its pending message", async () => {
    state.tasks = [{ ...task("Starting report", "starting"), pendingMessage: "Check the report" }];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Starting report" }));
    expect(await screen.findByText(/Starting the model/)).toBeTruthy();
    expect((screen.getByRole("textbox", { name: "Message task" }) as HTMLTextAreaElement).value).toBe("Check the report");
    expect((screen.getByRole("button", { name: "Send follow-up" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Stop task" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("stop", { taskId: "Starting report" }));
  });

  it.each(["done", "failed", "stopped", "running"] as const)("continues a %s task in the same conversation", async status => {
    state.tasks = [{ ...task("Report", status), accessMode: "full-access" }];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    const input = await screen.findByRole("textbox", { name: "Message task" });
    fireEvent.change(input, { target: { value: "Check the filters" } });
    fireEvent.click(screen.getByRole("button", { name: status === "running" ? "Send instruction" : "Send follow-up" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("follow-up", { taskId: "Report", message: "Check the filters", accessMode: "full-access", ...(status === "running" ? {} : { model: "local/test", effort: null }) }));
    await waitFor(() => expect(input).toHaveProperty("value", ""));
    expect(workRequest).not.toHaveBeenCalledWith("create", expect.anything());
  });
  it("preserves task drafts across selection and failed sends", async () => {
    state.tasks = [task("First", "done"), task("Second", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation(async (action, params) => {
      if (action === "follow-up") throw new Error("No available slot");
      return request(action, params);
    });
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "First" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Message task" }), { target: { value: "Keep my draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Second" }));
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "");
    fireEvent.click(screen.getByRole("button", { name: "First" }));
    fireEvent.click(screen.getByRole("button", { name: "Send follow-up" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "Keep my draft");
  });
  it("keeps a follow-up's model, reasoning and draft together when switching tasks or retrying a rejected send", async () => {
    const configured = { ...settings, providers: structuredClone(settings.providers!) };
    const reasoning: ReasoningCapability = { supported: true, levels: ["low", "medium", "high"], default: "medium", control: "effort", source: "user", detectedAt: 0 };
    configured.providers[0].models = [{ id: "test", enabled: true, reasoning }];
    configured.providers.push({ ...configured.providers[0], id: "backup", name: "Backup", models: [{ id: "alternate", displayName: "Alternate", enabled: true, reasoning }] });
    state.tasks = [{ ...task("First", "failed"), effort: "high" }, task("Second", "done")];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation(async (action, params) => {
      if (action === "follow-up") throw new Error("No available slot");
      return request(action, params);
    });
    render(<WorkView settings={configured} />);
    fireEvent.click(await screen.findByRole("button", { name: "First" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Message task" }), { target: { value: "Keep my draft" } });
    fireEvent.click(screen.getByRole("button", { name: "test · High" }));
    fireEvent.click(screen.getByRole("button", { name: "test" }));
    fireEvent.click(screen.getByRole("option", { name: "Alternate" }));
    const slider = await screen.findByRole("slider", { name: "Reasoning level" });
    fireEvent.keyDown(slider, { key: "Home" });
    fireEvent.keyDown(slider, { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Second" }));
    expect(screen.getByRole("button", { name: "test · Medium" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "First" }));
    expect(screen.getByRole("button", { name: "Alternate · Low" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Send follow-up" }));
    await screen.findByRole("alert");
    expect(workRequest).toHaveBeenCalledWith("follow-up", { taskId: "First", message: "Keep my draft", accessMode: "ask", model: "backup/alternate", effort: "low" });
    expect(screen.getByRole("textbox", { name: "Message task" })).toHaveProperty("value", "Keep my draft");
    expect(screen.getByRole("button", { name: "Alternate · Low" })).toBeTruthy();
    expect(workRequest).not.toHaveBeenCalledWith("create", expect.anything());
  });
  it("clears reasoning when a follow-up switches to a model without it", async () => {
    const configured = { ...settings, providers: structuredClone(settings.providers!) };
    configured.providers[0].models.push({ id: "reasoner", enabled: true, reasoning: { supported: true, levels: ["low", "high"], default: "low", control: "effort", source: "user", detectedAt: 0 } });
    state.tasks = [{ ...task("Report", "done"), model: "local/reasoner", effort: "high" }];
    render(<WorkView settings={configured} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    fireEvent.click(screen.getByRole("button", { name: "reasoner · High" }));
    fireEvent.click(screen.getByRole("button", { name: "reasoner" }));
    fireEvent.click(screen.getByRole("option", { name: "test" }));
    await screen.findByRole("button", { name: "test · ?" });
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    fireEvent.change(screen.getByRole("textbox", { name: "Message task" }), { target: { value: "Continue" } });
    fireEvent.click(screen.getByRole("button", { name: "Send follow-up" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("follow-up", { taskId: "Report", message: "Continue", accessMode: "ask", model: "local/test", effort: null }));
  });
  it.each(["queued", "starting", "running", "stopping", "needs-input"] as const)("locks model selection while a task is %s", async status => {
    state.tasks = [task("Report", status)];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    expect(await screen.findByRole("button", { name: "test · ?" })).toHaveProperty("disabled", true);
  });
  it("keeps a composer visible but prevents sending around a pending approval", async () => {
    state.tasks = [task("Report", "needs-input")];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Message task" }), { target: { value: "Continue" } });
    expect(screen.getByRole("button", { name: "Send follow-up" })).toHaveProperty("disabled", true);
  });
  it.each([undefined, "imported"])("shows the brief once while preserving later user messages (import: %s)", async importId => {
    state.tasks = [{ ...task("Report", "done"), importId }];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation(async (action, params) => action === "read" ? {
      result: { thread: { turns: [{ items: [
        { id: "prompt", type: "userMessage", content: [{ type: "text", text: importId ? "Make a brief" : "Make a brief\n\nAttached files (inside this space):\nnotes.txt\n\nRead-only folders:\n/reference" }] },
        { id: "answer", type: "agentMessage", text: "Report ready" },
        { id: "follow-up", type: "userMessage", content: [{ type: "text", text: "Keep this follow-up" }] },
      ] }] } },
    } : request(action, params));
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    await screen.findByText("Report ready");
    expect(screen.getAllByText("Make a brief")).toHaveLength(1);
    expect(screen.queryByText(/Attached files \(inside this space\)/)).toBeNull();
    expect(screen.getByText("Keep this follow-up")).toBeTruthy();
  });
  it("opens output links from the transcript and surfaces rejected paths", async () => {
    state.tasks = [{ ...task("Report", "done"), outputs: ["outputs/report.md"] }];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation(async (action, params) => action === "read" ? {
      result: { thread: { turns: [{ items: [{ id: "answer", type: "agentMessage", text: "[Report](outputs/report.md) · [Absolute](/synthetic/outputs/report.md) · [Missing](outputs/missing.md)" }] }] } },
    } : request(action, params));
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "Report" }));
    fireEvent.click(await screen.findByRole("link", { name: "Report" }));
    await waitFor(() => expect(workOutput).toHaveBeenCalledWith("Report", "outputs/report.md", "open"));
    vi.mocked(workOutput).mockClear();
    fireEvent.click(screen.getByRole("link", { name: "Absolute" }));
    await waitFor(() => expect(workOutput).toHaveBeenCalledWith("Report", "outputs/report.md", "open"));
    vi.mocked(workOutput).mockRejectedValueOnce(new Error("Unknown task output"));
    fireEvent.click(screen.getByRole("link", { name: "Missing" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Unknown task output");
  });
  it("starts new tasks with the configured full-access default", async () => {
    render(<WorkView settings={{ ...settings, defaultAccessMode: "full-access" }} />);
    await screen.findByRole("option", { name: "Research" });
    expect(screen.getByRole("combobox", { name: "Access mode" })).toHaveProperty("value", "full-access");
    fireEvent.change(screen.getByRole("combobox", { name: "Space" }), { target: { value: "space" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Task brief" }), { target: { value: "Validate a report" } });
    fireEvent.click(screen.getByRole("button", { name: "Run now" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("create", expect.objectContaining({ accessMode: "full-access" })));
  });
  it("renders selected space cards and collapses their entire contents", async () => {
    state.spaces.push({ id: "other", kind: "work", name: "Other", path: "/other", readOnlyFolders: [] });
    state.spaces[0].readOnlyFolders = ["/reference"];
    state.tasks = [task("Report", "done")];
    render(<WorkView settings={settings} />);
    const space = await screen.findByRole("region", { name: "Research" });
    expect(document.querySelectorAll(".work-space.squircle")).toHaveLength(2);
    expect(document.querySelector(".sidebar .squircle .squircle")).toBeNull();
    fireEvent.click(within(space).getByRole("button", { name: "Report" }));
    await waitFor(() => expect(space.classList.contains("is-selected")).toBe(true));
    fireEvent.click(within(space).getByRole("button", { name: "Research" }));
    expect(space.querySelector(".sidebar-tree")).toBeNull();
    expect(space.querySelector(".work-readonly-folders")).toBeNull();
    expect(within(space).getByRole("button", { name: "Research" }).getAttribute("aria-expanded")).toBe("false");
  });
  it("keeps task actions in a keyboard-accessible context menu", async () => {
    state.tasks = [task("Draft report", "running")];
    render(<WorkView settings={settings} />);
    const row = await screen.findByRole("button", { name: "Draft report" });
    expect(screen.queryByRole("menuitem")).toBeNull();
    fireEvent.contextMenu(row);
    expect(screen.getByRole("menuitem", { name: "Open task" })).toBe(document.activeElement);
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Stop task" })).toBe(document.activeElement);
    fireEvent.click(document.activeElement!);
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("stop", { taskId: "Draft report" }));
    expect(screen.queryByRole("menuitem")).toBeNull();
  });
  it("shows imported transcripts and copies, continues in Work, and refreshes after undo", async () => {
    const imported = { ...task("Cowork report", "done"), brief: "## Task context\n\n**User brief**", importId: "synthetic-import", outputs: ["outputs/report.pdf", "uploads/notes.txt"] };
    state.tasks = [imported];
    const request = vi.mocked(workRequest).getMockImplementation()!;
    vi.mocked(workRequest).mockImplementation(async (action, params) => {
      if (action === "read") return { result: { thread: { turns: [{ items: [{ id: "answer", type: "agentMessage", text: "## Imported report transcript\n\n**Reviewed**\n\n```js\nconst a = 1;\nconsole.log(a);\n```" }] }] } } };
      if (action === "continue") { const next = task("Continued report", "running"); state.tasks.push(next); return next; }
      return request(action, params);
    });
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: /^Cowork report/ }));
    expect(await screen.findByRole("heading", { level: 2, name: "Imported report transcript" })).toBeTruthy();
    expect(screen.getByText("Reviewed").tagName).toBe("STRONG");
    expect(screen.getByRole("button", { name: "Copy code block" }).closest(".message .markdown .markdown-codeblock")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Task context" })).toBeTruthy();
    expect(screen.getByText("User brief").tagName).toBe("STRONG");
    expect(screen.queryByRole("button", { name: "Rewind…" })).toBeNull();
    expect(screen.getByTitle("uploads/notes.txt")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Quick Look" }));
    await waitFor(() => expect(workOutput).toHaveBeenCalledWith("Cowork report", "outputs/report.pdf", "preview"));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("continue", { taskId: "Cowork report", spaceId: "space", model: "local/test" }));
    expect(await screen.findByRole("button", { name: "Stop task" })).toBeTruthy();
    state.tasks = state.tasks.filter(t => !t.importId);
    events.importsChanged();
    await waitFor(() => expect(screen.queryByRole("button", { name: /^Cowork report/ })).toBeNull());
    expect(screen.getByRole("button", { name: /^Continued report/ })).toBeTruthy();
  });
  it("keeps a missing-folder space read-only and requires a real continuation destination", async () => {
    state.spaces = [{ id: "imported-missing", name: "Imported (folder missing)", path: "", kind: "work", readOnly: true, readOnlyFolders: [] }];
    state.tasks = [{ ...task("Missing report", "stopped"), spaceId: "imported-missing", importId: "synthetic-import" }];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: /^Missing report/ }));
    expect(screen.getByRole("button", { name: "Continue" })).toHaveProperty("disabled", true);
    expect(screen.queryByRole("option", { name: "Imported (folder missing)" })).toBeNull();
    state.spaces.push({ id: "space", name: "Available", path: "/synthetic", kind: "work", readOnlyFolders: [] });
    events.importsChanged();
    await waitFor(() => expect(screen.getByRole("button", { name: "Continue" })).toHaveProperty("disabled", false));
    expect(screen.getByRole("combobox", { name: "Continuation space" })).toHaveProperty("value", "space");
  });
  it("queues a task with registry model, attachments, space and Ask default", async () => {
    render(<WorkView settings={settings} />);
    await screen.findByRole("option", { name: "Research" });
    fireEvent.change(screen.getByRole("combobox", { name: "Space" }), { target: { value: "space" } });
    expect(screen.getByRole("combobox", { name: "Access mode" })).toHaveProperty("value", "ask");
    const left = screen.getByRole("combobox", { name: "Access mode" }).closest(".composer-shell-left");
    expect(left).toBeTruthy();
    expect(left?.querySelectorAll("select")[0]).toBe(screen.getByRole("combobox", { name: "Access mode" }));
    expect(left?.querySelectorAll("select")[1]).toBe(screen.getByRole("combobox", { name: "Space" }));
    expect(screen.queryByRole("combobox", { name: "Work model" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Research brief" }));
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Attach files to space" }));
    await screen.findByText("copied-notes.pdf");
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Queue task" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("create", expect.objectContaining({ spaceId: "space", model: "local/test", accessMode: "ask", runNow: false, attachments: [".taucode/uploads/copied-notes.pdf"] })));
    expect(await screen.findByText(/Waiting for an available slot/)).toBeTruthy();
  });
  it("starts new tasks on the policy default model rather than the first enabled one", async () => {
    const withDefault = { providers: [{ id: "local", name: "Local", enabled: true, models: [{ id: "test", enabled: true }, { id: "sol", enabled: true }] }], agentPolicy: { defaultModel: { providerId: "local", modelId: "sol" } } } as AppSettings;
    render(<WorkView settings={withDefault} />);
    await screen.findByRole("option", { name: "Research" });
    fireEvent.change(screen.getByRole("combobox", { name: "Space" }), { target: { value: "space" } });
    fireEvent.click(screen.getByRole("button", { name: "Research brief" }));
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Queue task" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("create", expect.objectContaining({ model: "local/sol" })));
  });
  it("groups tasks, exposes the input callout, renders plans and output actions, and responds", async () => {
    const needs = task("Review brief", "needs-input");
    needs.plan = [{ step: "Write report", status: "completed" }];
    needs.outputs = ["outputs/report.pdf"];
    needs.pending = [{ id: "approval", method: "item/fileChange/requestApproval", params: { reason: "Replace notes" } }];
    state.tasks = [needs, task("Running task", "running"), task("Queued task", "queued"), task("Finished task", "done")];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: "1 task needs your input" }));
    expect(screen.getAllByRole("button", { name: "Review brief" })).toHaveLength(1);
    expect((await screen.findByRole("checkbox", { name: "Write report" })).getAttribute("aria-checked")).toBe("true");
    const space = screen.getByRole("region", { name: "Research" });
    const rows = space.querySelectorAll(".sidebar-tree-node");
    expect(rows.length).toBeGreaterThan(0);
    expect(Array.from(rows).every(row => row.getAttribute("data-depth") === "1")).toBe(true);
    expect(rows[rows.length - 1].classList.contains("tree-last-child")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Quick Look" }));
    await waitFor(() => expect(workOutput).toHaveBeenCalledWith("Review brief", "outputs/report.pdf", "preview"));
    fireEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("respond", { taskId: "Review brief", requestId: "approval", answer: { decision: "accept" } }));
    await waitFor(() => expect(screen.queryByText("Approval required")).toBeNull());
    expect(screen.queryByRole("button", { name: /needs your input/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Stop all tasks" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("stop-all", {}));
  });
  it("renders one-line rows with distinct status glyphs and tooltip metadata", async () => {
    const statuses = ["needs-input", "running", "queued", "done", "failed", "stopped"] as const;
    state.tasks = statuses.map(status => task(`Task ${status}`, status));
    render(<WorkView settings={settings} />);
    await screen.findByRole("button", { name: "Task running" });
    for (const status of statuses) {
      const row = screen.getByRole("button", { name: `Task ${status}` });
      expect(row.classList.contains("has-secondary-line")).toBe(false);
      expect(row.querySelector(".thread-details")).toBeNull();
      expect(row.textContent).not.toContain("local/test");
      expect(row.title).toContain(`Research · Local · test · ${status.replace("-", " ")}`);
      expect(row.querySelector(`.thread-status.work-status-${status}`)).toBeTruthy();
      expect(row.querySelector(".thread-time")?.textContent).toBe(["needs-input", "failed"].includes(status) ? status.replace("-", " ") : undefined);
    }
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Task done" })); });
    expect(document.querySelector(".thread-header-model")?.textContent).toBe("pi · test");
    expect(document.querySelector('.tree-selected .work-task-row.active')).toBeTruthy();
  });
  it("uses available timestamps for relative time and tooltip details", async () => {
    const updatedAt = Math.floor(Date.now() / 1000) - 120;
    state.tasks = [{ ...task("Timed task", "done"), createdAt: updatedAt - 60, updatedAt }];
    render(<WorkView settings={settings} />);
    const row = await screen.findByRole("button", { name: "Timed task" });
    expect(row.querySelector(".thread-time")?.textContent).toBe("2m");
    expect(row.title).toContain(`Created: ${new Date((updatedAt - 60) * 1000).toLocaleString()}`);
    expect(row.title).toContain(`Updated: ${new Date(updatedAt * 1000).toLocaleString()}`);
  });
  it.each(["needs-input", "failed"] as const)("keeps %s attention visible when a timestamp is available", async status => {
    state.tasks = [{ ...task("Attention task", status), updatedAt: Math.floor(Date.now() / 1000) - 120 }];
    render(<WorkView settings={settings} />);
    const row = await screen.findByRole("button", { name: "Attention task" });
    expect(row.querySelector(`.work-attention-${status}`)?.textContent).toBe(status.replace("-", " "));
    expect(row.title).toContain("Updated:");
  });
  it("cycles input tasks, expands their spaces and clears search before scrolling", async () => {
    state.tasks = [task("First", "needs-input"), task("Second", "needs-input"), task("Third", "needs-input")];
    render(<WorkView settings={settings} />);
    const callout = await screen.findByRole("button", { name: "3 tasks need your input" });
    const space = screen.getByRole("region", { name: "Research" });
    fireEvent.click(within(space).getByRole("button", { name: "Research" }));
    fireEvent.click(screen.getByRole("button", { name: /Search/ }));
    fireEvent.change(screen.getByRole("searchbox", { name: "Search tasks" }), { target: { value: "missing" } });
    for (const title of ["First", "Second", "Third", "First"]) {
      await act(async () => { fireEvent.click(callout); });
      const row = within(space).getByRole("button", { name: title });
      expect(row.getAttribute("aria-current")).toBe("page");
      expect(vi.mocked(row.scrollIntoView).mock.contexts.slice(-1)[0]).toBe(row);
    }
    expect(screen.getByRole("searchbox", { name: "Search tasks" })).toHaveProperty("value", "");
  });
  it.each([0, 1, 2])("pluralizes the read-only folder disclosure for %i folders", async count => {
    state.spaces[0].readOnlyFolders = Array.from({ length: count }, (_, i) => `/reference-${i}`);
    render(<WorkView settings={settings} />);
    const space = await screen.findByRole("region", { name: "Research" });
    expect(screen.queryByRole("button", { name: /tasks? needs? your input/ })).toBeNull();
    const summary = space.querySelector(".work-readonly-folders summary");
    if (!count) expect(summary).toBeNull();
    else {
      expect(summary?.textContent).toBe(`${count} read-only ${count === 1 ? "folder" : "folders"}`);
      expect(summary?.querySelector(".lucide-folder-lock")).toBeTruthy();
    }
  });
  it("offers five templates, with read-only cleanup and Run now", async () => {
    render(<WorkView settings={settings} />);
    await screen.findByRole("option", { name: "Research" });
    fireEvent.change(screen.getByRole("combobox", { name: "Space" }), { target: { value: "space" } });
    for (const title of ["Organize a folder", "Research brief", "Spreadsheet from documents", "Slide deck from notes", "Clean up downloads"]) expect(screen.getByRole("button", { name: title })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Clean up downloads" }));
    expect(screen.getByRole("combobox", { name: "Access mode" })).toHaveProperty("value", "read-only");
    fireEvent.click(screen.getByRole("button", { name: "Run now" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("create", expect.objectContaining({ accessMode: "read-only", runNow: true })));
  });
  it("updates a selected task's checklist and outputs from backend progress", async () => {
    state.tasks = [task("Live task", "running")];
    render(<WorkView settings={settings} />);
    fireEvent.click(await screen.findByRole("button", { name: /^Live task/ }));
    state.tasks[0].plan = [{ step: "Analyze data", status: "pending" }];
    state.tasks[0].outputs = ["outputs/chart.png"];
    events.listener({ workspace_id: "work", message: { method: "work/updated", params: { task: state.tasks[0] } } });
    expect((await screen.findByRole("checkbox", { name: "Analyze data" })).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByTitle("outputs/chart.png")).toBeTruthy();
  });
});


it("includes the shared sidebar resize handle", () => {
  render(<WorkView settings={settings} />);
  expect(screen.getByRole("separator", { name: "Resize sidebar" })).toBeTruthy();
});
