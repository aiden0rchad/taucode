// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { managedTasksRequest } from "@services/tauri";
import { createPreviewHandler } from "@/dev/uiPreview/handler";
import { BackgroundTasks } from "./BackgroundTasks";
import { useManagedTasks, type ManagedTask, type ManagedTasksController } from "./useManagedTasks";
vi.mock("@services/tauri", () => ({ managedTasksRequest: vi.fn() }));
const task = (overrides: Partial<ManagedTask> = {}): ManagedTask => ({
  id: "dev", workspaceId: "workspace", threadId: "thread", command: "npm run dev", label: "Dev server", cwd: "/project", pid: 123, status: "running", startedAt: Date.now() - 5000, endedAt: null, exitCode: null, output: "Local: http://localhost:5173/", outputTruncated: false, urls: ["http://localhost:5173/"], ...overrides,
});
function controller(tasks: ManagedTask[] = []): ManagedTasksController {
  return { tasks, servers: tasks.flatMap(task => task.urls.map(url => ({ id: `${task.id}:${url}`, taskId: task.id, url, title: task.label, status: task.status, startedAt: task.startedAt }))), error: "", busy: false, canStart: true, start: vi.fn().mockResolvedValue(undefined), stop: vi.fn().mockResolvedValue(undefined), restart: vi.fn().mockResolvedValue(undefined), clearFinished: vi.fn().mockResolvedValue(undefined), refresh: vi.fn().mockResolvedValue(undefined) };
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
beforeEach(() => { vi.useFakeTimers(); vi.mocked(managedTasksRequest).mockReset(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("managed task snapshots", () => {
  it("never starts automatically and requires both workspace and session", async () => {
    vi.mocked(managedTasksRequest).mockResolvedValue([]);
    const { result, rerender } = renderHook(({ workspace, thread }) => useManagedTasks(workspace, thread), { initialProps: { workspace: null as string | null, thread: null as string | null } });
    await expect(result.current.start("echo hello")).rejects.toThrow("Select a workspace session");
    expect(managedTasksRequest).not.toHaveBeenCalled();
    rerender({ workspace: "workspace", thread: "thread" }); await act(async () => {});
    expect(managedTasksRequest).toHaveBeenCalledExactlyOnceWith("workspace", "thread", "list", {});
    expect(result.current.canStart).toBe(true);
    rerender({ workspace: "workspace", thread: null });
    expect(result.current.canStart).toBe(false); expect(result.current.tasks).toEqual([]);
  });
  it("rejects late snapshots from another scope and stops polling when unmounted", async () => {
    const old = deferred<ManagedTask[]>(); const next = task({ id: "next", threadId: "next-thread" });
    vi.mocked(managedTasksRequest).mockImplementation((_, thread) => (thread === "thread" ? old.promise : Promise.resolve([next])) as never);
    const { result, rerender, unmount } = renderHook(({ thread }) => useManagedTasks("workspace", thread), { initialProps: { thread: "thread" } });
    rerender({ thread: "next-thread" }); await act(async () => {});
    expect(result.current.tasks).toEqual([next]);
    await act(async () => old.resolve([task()])); expect(result.current.tasks).toEqual([next]);
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(managedTasksRequest).toHaveBeenLastCalledWith("workspace", "next-thread", "list", {});
    const count = vi.mocked(managedTasksRequest).mock.calls.length; unmount(); await vi.advanceTimersByTimeAsync(10000);
    expect(managedTasksRequest).toHaveBeenCalledTimes(count);
  });
  it("protects mutations from older polls, skips overlapping polls, and retains tasks on failure", async () => {
    const old = deferred<ManagedTask[]>(); const started = deferred<ManagedTask[]>();
    vi.mocked(managedTasksRequest).mockImplementation((_, __, action) => (action === "list" ? old.promise : started.promise) as never);
    const { result } = renderHook(() => useManagedTasks("workspace", "thread")); let start!: Promise<void>;
    act(() => { start = result.current.start("npm run dev", "Dev server"); }); expect(result.current.busy).toBe(true);
    await act(async () => { old.resolve([]); await result.current.refresh(); }); expect(managedTasksRequest).toHaveBeenCalledTimes(2);
    const running = task(); await act(async () => { started.resolve([running]); await start; });
    expect(result.current.tasks).toEqual([running]); expect(result.current.servers[0]).toMatchObject({ taskId: "dev", status: "running" });
    vi.mocked(managedTasksRequest).mockRejectedValueOnce(new Error("Stop failed"));
    await act(async () => { await expect(result.current.stop("dev")).rejects.toThrow("Stop failed"); });
    expect(result.current.tasks).toEqual([running]); expect(result.current.error).toContain("Stop failed"); expect(result.current.busy).toBe(false);
  });
});

describe("Background Tasks", () => {
  it("shows elapsed time/output and opens detected URLs only on explicit click", async () => {
    const value = controller([task({ outputTruncated: true })]); const open = vi.fn(); render(<BackgroundTasks controller={value} onOpenUrl={open} />);
    expect(open).not.toHaveBeenCalled(); expect(screen.getByText("0:05")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Stop Dev server" })).toBeTruthy();
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); }); expect(screen.getByText("0:07")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "http://localhost:5173/" })); expect(open).toHaveBeenCalledExactlyOnceWith("http://localhost:5173/");
    fireEvent.click(screen.getByText("Dev server")); expect(screen.getByText("Showing the latest 128 KB of output.")).toBeTruthy(); expect(screen.getByLabelText("Output for Dev server").textContent).toContain("Local:");
    fireEvent.click(screen.getByRole("button", { name: "Stop Dev server" })); expect(value.stop).toHaveBeenCalledExactlyOnceWith("dev");
  });
  it("runs configured/manual commands explicitly, preserving the draft on start failure", async () => {
    const value = controller(); vi.mocked(value.start).mockRejectedValueOnce(new Error("Cannot start"));
    render(<BackgroundTasks controller={value} commands={[{ id: "lint", script: "npm run lint", icon: "lint", label: "Lint" }]} onOpenUrl={vi.fn()} />);
    const input = screen.getByRole("textbox"); fireEvent.change(input, { target: { value: "printf hello" } });
    fireEvent.click(screen.getByRole("button", { name: /^Run$/ })); await act(async () => {}); expect((input as HTMLInputElement).value).toBe("printf hello");
    fireEvent.click(screen.getByRole("button", { name: /^Run$/ })); await act(async () => {}); expect((input as HTMLInputElement).value).toBe("");
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Lint" })); }); expect(value.start).toHaveBeenLastCalledWith("npm run lint", "Lint");
  });
  it("preserves the next command typed while a task start is pending", async () => {
    const value = controller(); const started = deferred<void>();
    vi.mocked(value.start).mockReturnValueOnce(started.promise);
    const { rerender } = render(<BackgroundTasks controller={value} onOpenUrl={vi.fn()} />);
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "printf first" } });
    fireEvent.click(screen.getByRole("button", { name: /^Run$/ }));
    rerender(<BackgroundTasks controller={{ ...value, busy: true }} onOpenUrl={vi.fn()} />);
    fireEvent.change(input, { target: { value: "printf next" } });
    await act(async () => started.resolve());
    expect(value.start).toHaveBeenCalledExactlyOnceWith("printf first", undefined);
    expect(input.value).toBe("printf next");
  });
  it("keeps Stop available while Terminal is disabled and prevents new commands", () => {
    const value = controller([task()]);
    render(<BackgroundTasks controller={value} commandsEnabled={false} commands={[{ id: "dev", script: "npm run dev", icon: "dev", label: "Dev" }]} onOpenUrl={vi.fn()} />);
    expect(screen.getByText("Enable Terminal in Features to start tasks.")).toBeTruthy();
    expect((screen.getByRole("textbox") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /^Run$/ }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Dev" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Stop Dev server" }));
    expect(value.stop).toHaveBeenCalledExactlyOnceWith("dev");
  });
  it("offers restart and finished clearing through scoped actions", () => {
    const value = controller([task({ status: "failed", endedAt: Date.now(), exitCode: 7 })]); render(<BackgroundTasks controller={value} onOpenUrl={vi.fn()} />);
    fireEvent.click(screen.getByText("Finished")); fireEvent.click(screen.getByText("Dev server")); expect(screen.getByText("failed · 7")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Restart" })); expect(value.restart).toHaveBeenCalledExactlyOnceWith("dev");
    fireEvent.click(screen.getByRole("button", { name: "Clear finished" })); expect(value.clearFinished).toHaveBeenCalledOnce();
  });
});

it("isolates preview task history/actions and respects the Terminal gate", async () => {
  const { handler, state } = createPreviewHandler(); const thread = state.threads[0]; const args = { workspaceId: thread.workspaceId, threadId: thread.id };
  const invoke = (action: string, params = {}) => handler("managed_tasks_request", { ...args, action, params }) as Promise<ManagedTask[]>;
  const initial = await invoke("list"); expect(initial.map(task => task.status)).toEqual(["running", "completed"]); expect(initial[0].output).toContain("No process was started");
  const started = (await invoke("start", { command: "printf custom", label: "Custom" }))[0]; expect(started).toMatchObject({ status: "running", urls: [], command: "printf custom" });
  await expect(handler("managed_tasks_request", { ...args, threadId: "thread-empty", action: "stop", params: { id: started.id } })).rejects.toThrow("Task not found");
  await expect(handler("managed_tasks_request", { ...args, workspaceId: "workspace-notes", action: "start", params: { command: "no" } })).rejects.toThrow("another workspace");
  await invoke("stop", { id: started.id }); const restarted = (await invoke("restart", { id: started.id }))[0]; expect(restarted.id).not.toBe(started.id); expect(restarted.command).toBe(started.command);
  const running = await invoke("clearFinished"); expect(running.every(task => task.status === "running")).toBe(true); expect(running.some(task => task.id === initial[0].id)).toBe(true);
  state.settings.features = { ...state.settings.features, terminal: false };
  await expect(invoke("start", { command: "echo blocked" })).rejects.toThrow("Enable Terminal"); await expect(invoke("restart", { id: started.id })).rejects.toThrow("Enable Terminal");
  expect((await invoke("stop", { id: restarted.id })).find(task => task.id === restarted.id)?.status).toBe("stopped");
});
