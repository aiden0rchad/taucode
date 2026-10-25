// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppSettings, WorkState, WorkTask } from "@/types";
import { WorkView } from "./WorkView";
import { workRequest, workOutput, workAttachment, pickWorkAttachments } from "@services/tauri";

const events = vi.hoisted(() => ({ listener: (_event: unknown) => {}, importsChanged: () => {} }));
vi.mock("@services/tauri", () => ({ subscribeBrowser: vi.fn().mockResolvedValue(() => {}), browserRequest: vi.fn().mockResolvedValue(undefined),
  workRequest: vi.fn(), workOutput: vi.fn(), workAttachment: vi.fn(),
  pickWorkspacePath: vi.fn(), pickWorkspacePaths: vi.fn(), pickWorkAttachments: vi.fn(),
  checkpointRequest: vi.fn(),
}));
vi.mock("@services/events", () => ({
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
beforeEach(() => {
  localStorage.clear();
  state = { spaces: [{ id: "space", kind: "work", name: "Research", path: "/synthetic", readOnlyFolders: [] }], tasks: [], concurrency: 2 };
  vi.mocked(workRequest).mockImplementation(async (action, params) => {
    if (action === "list") return structuredClone(state);
    if (action === "create") { const created = { ...task("new-task", "queued"), ...params, threadId: null } as WorkTask; state.tasks.push(created); return created; }
    if (action === "read") return { result: { thread: { id: params?.taskId, turns: [] } } };
    if (action === "respond") { state.tasks[0].status = "running"; state.tasks[0].pending = []; }
    if (action === "stop-all") state.tasks.forEach(t => { t.status = "stopped"; });
    return {};
  });
  vi.mocked(workOutput).mockResolvedValue();
  vi.mocked(workAttachment).mockResolvedValue(".taucode/uploads/copied-notes.pdf");
  vi.mocked(pickWorkAttachments).mockResolvedValue(["/synthetic-source/notes.pdf"]);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("Work mode", () => {
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
  it("groups tasks, exposes the inbox, renders plans and output actions, and responds", async () => {
    const needs = task("Review brief", "needs-input");
    needs.plan = [{ step: "Write report", status: "completed" }];
    needs.outputs = ["outputs/report.pdf"];
    needs.pending = [{ id: "approval", method: "item/fileChange/requestApproval", params: { reason: "Replace notes" } }];
    state.tasks = [needs, task("Running task", "running"), task("Queued task", "queued"), task("Finished task", "done")];
    render(<WorkView settings={settings} />);
    const inbox = await screen.findByRole("region", { name: "Needs input inbox" });
    fireEvent.click(within(inbox).getByRole("button", { name: /^Review brief/ }));
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
    fireEvent.click(screen.getByRole("button", { name: "Stop all tasks" }));
    await waitFor(() => expect(workRequest).toHaveBeenCalledWith("stop-all", {}));
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
