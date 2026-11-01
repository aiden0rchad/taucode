// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ComputerApprovals } from "./ComputerApprovals";
import { ComputerApprovalStatus, ComputerRunItem } from "./ComputerRunItem";
import { defaultComputerSettings, parseComputerRun } from "./computer";
import { SettingsComputerSection } from "@settings/components/sections/SettingsComputerSection";
import { computerRequest, computerRequestPermissions } from "@services/tauri";
import type { AppSettings } from "@/types";
import { buildConversationItem } from "@/utils/threadItems.conversion";
import { prepareThreadItems } from "@/utils/threadItems.explore";
vi.mock("@services/tauri", () => ({ engineRuntime: async () => [{ engine: "computer", state: "ready", message: "Ready" }], computerRequest: vi.fn(), computerRequestPermissions: vi.fn() }));
const approvalPush = vi.hoisted(() => ({ listener: null as null | ((approval: unknown) => void) }));
vi.mock("@services/events", () => ({ subscribeComputerApproval: (listener: (approval: unknown) => void) => { approvalPush.listener = listener; return () => { approvalPush.listener = null; }; } }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const step = { step: 1, thumbnail: "aGVsbG8=", action: { kind: "click" }, reflection: "Found button" };
describe("computer use", () => {
  it("shows live progress and controls pause, resume and stop", async () => {
    let run = { id: "run", instruction: "Calculate", running: true, paused: false, steps: [{ ...step, target: "Clear" }] };
    vi.mocked(computerRequest).mockImplementation(async p => {
      if (p.action === "pause") run = { ...run, paused: p.paused === true };
      if (p.action === "stop") run = { ...run, running: false };
      return run as never;
    });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    expect(await screen.findByText("Last action · Step 1: Clear")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Pause computer use" }));
    expect(await screen.findByText("Computer Use paused")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Resume computer use" }));
    expect(await screen.findByText("Computer Use running")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Stop computer use" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Computer Use controls" })).toBeNull());
    expect(computerRequest).toHaveBeenCalledWith({ action: "pause", runId: "run", paused: true });
    expect(computerRequest).toHaveBeenCalledWith({ action: "pause", runId: "run", paused: false });
    expect(computerRequest).toHaveBeenCalledWith({ action: "stop", runId: "run" });
  });
  it.each([
    ["observing", "Capturing window"], ["planning", "Planning"], ["acting", "Acting"],
    ["waitingForApproval", "Waiting for approval"], ["settling", "Waiting for the app"],
  ])("shows the live %s phase", async (phase, label) => {
    vi.mocked(computerRequest).mockResolvedValue({ id: "run", instruction: "Calculate", running: true, phase, steps: [] });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    expect(await screen.findByText(`Computer Use · ${label}`)).toBeTruthy();
  });
  it("shows the bound app and window while capturing", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ id: "run", instruction: "Find", running: true, phase: "observing", mode: "background", targetApp: { bundleId: "com.apple.TextEdit", name: "TextEdit", window: { id: 7, title: "Background QA.txt" } }, steps: [] });
    render(<ComputerApprovalStatus detail='{"instruction":"Find"}' />);
    expect(await screen.findByText("Background control · TextEdit")).toBeTruthy();
    expect(screen.getByText("Background QA.txt")).toBeTruthy();
    expect(screen.getByText("Computer Use · Capturing window")).toBeTruthy();
  });
  it("keeps Stop available while Pause is pending and ignores its late response", async () => {
    let releasePause!: () => void;
    const pending = new Promise<void>(resolve => { releasePause = resolve; });
    let running = true;
    vi.mocked(computerRequest).mockImplementation(async p => {
      if (p.action === "pause") await pending;
      if (p.action === "stop") running = false;
      return { id: "run", instruction: "Calculate", running, steps: [] } as never;
    });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    fireEvent.click(await screen.findByRole("button", { name: "Pause computer use" }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "pause", runId: "run", paused: true }));
    const stop = screen.getByRole("button", { name: "Stop computer use" }) as HTMLButtonElement;
    expect(stop.disabled).toBe(false);
    fireEvent.click(stop);
    await waitFor(() => expect(screen.queryByRole("region", { name: "Computer Use controls" })).toBeNull());
    await act(async () => { releasePause(); await pending; });
    expect(screen.queryByRole("region", { name: "Computer Use controls" })).toBeNull();
    expect(computerRequest).toHaveBeenCalledWith({ action: "stop", runId: "run" });
  });
  it("gives pause and approval precedence over the underlying phase", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ id: "run", instruction: "Calculate", running: true, paused: true, phase: "planning", steps: [] });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    expect(await screen.findByText("Computer Use paused")).toBeTruthy();
    cleanup();
    vi.mocked(computerRequest).mockResolvedValue({ id: "run", instruction: "Calculate", running: true, phase: "acting", steps: [], approval: { reason: "Confirm typing" } });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    expect(await screen.findByText("Computer Use · Waiting for approval")).toBeTruthy();
    expect(screen.getByText(/Waiting for approval: Confirm typing/)).toBeTruthy();
  });
  it("shows a pushed approval at once instead of waiting for the next status poll", async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(computerRequest).mockResolvedValue({ running: false, steps: [], approval: null });
      render(<ComputerApprovals />);
      await act(async () => { await Promise.resolve(); });
      expect(computerRequest).toHaveBeenCalledTimes(1);
      act(() => approvalPush.listener?.({ id: "a1", reason: "Confirm typing", target: "Display", action: { kind: "type" }, app: "com.apple.calculator" }));
      expect(screen.getByRole("button", { name: "Approve" })).toBeTruthy();
      expect(computerRequest).toHaveBeenCalledTimes(1);
      act(() => approvalPush.listener?.(null));
      expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
    } finally { vi.useRealTimers(); }
  });
  it("keeps diagnostics collapsed and prepares a continuation without replaying input", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<ComputerRunItem run={{ id: "failed", instruction: "Append once", error: "Input interrupted\nSidecar stderr (last 20 lines):\nraw diagnostic", summary: "Input interrupted", steps: [{ ...step, action: { kind: "type", text: "partial" }, error: "Paused" }] }} />);
    expect(screen.getByText("Computer Use interrupted")).toBeTruthy();
    expect(screen.getByText("Error details").closest("details")?.open).toBe(false);
    expect(screen.getByText("Input interrupted", { selector: 'p[role="alert"]' })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Copy continuation request" }));
    await screen.findByText("Copied. Paste into this conversation to review and send.");
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("Take a fresh observation first"));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("interrupted or failed; effects uncertain"));
    expect(writeText).not.toHaveBeenCalledWith(expect.stringContaining("raw diagnostic"));
    expect(computerRequest).not.toHaveBeenCalled();
  });
  it("distinguishes rejected actions from uncertain effects in recovery", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<ComputerRunItem run={{ instruction: "Find a line", error: "Action not executed: Background hotkey menu disabled: Edit > Find > Find…", steps: [{ ...step, recovered: true, error: "Disabled", action: { kind: "hotkey", keys: ["command", "f"] } }] }} />);
    expect(screen.getByText("Computer Use blocked")).toBeTruthy();
    expect(screen.getByText(/This command is disabled in the app’s background window/)).toBeTruthy();
    expect(screen.getByText(/Not executed/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Copy continuation request" }));
    await screen.findByText(/Copied. Paste/);
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("not executed; rejected before input"));
    expect(computerRequest).not.toHaveBeenCalled();
  });
  it("explains an unconfirmed native action without claiming it was rejected", () => {
    render(<ComputerRunItem run={{ error: "ax_press_failed(-25204): macOS could not confirm the action", steps: [{ ...step, error: "CannotComplete" }] }} />);
    expect(screen.getByText("Computer Use interrupted")).toBeTruthy();
    expect(screen.getByText(/Taucode stopped without sending a second click or edit/)).toBeTruthy();
    expect(screen.getByText(/Check result/)).toBeTruthy();
    expect(screen.queryByText(/Not executed/)).toBeNull();
  });
  it("shows completed work even when the final persisted phase was acting", () => {
    render(<ComputerRunItem run={{ running: false, phase: "acting", summary: "Verified 48", steps: [{ ...step, action: { kind: "done" } }] }} />);
    expect(screen.getByRole("status").textContent).toContain("Computer Use completed");
    expect(screen.queryByLabelText("Computer Use recovery")).toBeNull();
  });
  it("does not control a replacement run from stale displayed controls", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ id: "old", instruction: "Calculate", running: true, steps: [] });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    await screen.findByRole("button", { name: "Pause computer use" });
    vi.mocked(computerRequest).mockResolvedValue({ id: "new", instruction: "Different task", running: true, steps: [] });
    fireEvent.click(screen.getByRole("button", { name: "Stop computer use" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Computer Use controls" })).toBeNull());
    expect(vi.mocked(computerRequest).mock.calls.some(([p]) => p.action === "stop")).toBe(false);
  });
  it("shows control failures and keeps the controls available for retry", async () => {
    vi.mocked(computerRequest).mockImplementation(async p => {
      if (p.action === "pause") throw new Error("Host unavailable");
      return { id: "run", instruction: "Calculate", running: true, steps: [] } as never;
    });
    render(<ComputerApprovalStatus detail='{"instruction":"Calculate"}' />);
    fireEvent.click(await screen.findByRole("button", { name: "Pause computer use" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Host unavailable");
    expect((screen.getByRole("button", { name: "Pause computer use" }) as HTMLButtonElement).disabled).toBe(false);
  });
  it("persists risky-only typing with a Return warning", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={{ computer: { ...defaultComputerSettings, confirmTyping: "sensitive" } } as AppSettings} onUpdateAppSettings={save} />);
    expect(screen.queryByText(/Return can submit or send in some apps/)).toBeNull();
    fireEvent.change(screen.getByLabelText("Typing confirmation"), { target: { value: "risky-only" } });
    expect(screen.getByText(/Return can submit or send in some apps/)).toBeTruthy();
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ confirmTyping: "risky-only" }) })));
  });
  it("persists typing confirmation and app access, migrating strings and locking System Settings", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    const save = vi.fn().mockResolvedValue(undefined);
    const computer = { ...defaultComputerSettings, appAccess: "listed" as const, allowedApps: ["com.apple.TextEdit", { bundleId: "com.apple.systempreferences", access: "full" }] };
    render(<SettingsComputerSection appSettings={{ computer } as AppSettings} onUpdateAppSettings={save} />);
    expect((screen.getByLabelText("Typing confirmation") as HTMLSelectElement).value).toBe("risky-only");
    expect(screen.getByText(/Safe shortcuts: ⌘N, ⌘F, ⌘A, ⌘C, ⌘V, ⌘Z, ⌘⇧Z, arrows, Tab, Escape/)).toBeTruthy();
    const locked = screen.getByLabelText("Access for com.apple.systempreferences") as HTMLSelectElement;
    expect(locked.disabled).toBe(true);
    expect(locked.value).toBe("read-only");
    expect(screen.getByText(/System Settings is locked to read-only.*Battery → Options/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Typing confirmation"), { target: { value: "always" } });
    fireEvent.change(screen.getByLabelText("Access for com.apple.TextEdit"), { target: { value: "read-only" } });
    fireEvent.change(screen.getByLabelText("Allowed app bundle IDs (one per line)"), { target: { value: "com.apple.TextEdit\ncom.apple.systempreferences\ncom.apple.Preview" } });
    expect((screen.getByLabelText("Access for com.apple.Preview") as HTMLSelectElement).value).toBe("full");
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ confirmTyping: "always", allowedApps: [
      { bundleId: "com.apple.TextEdit", access: "read-only" },
      { bundleId: "com.apple.systempreferences", access: "read-only" },
      { bundleId: "com.apple.Preview", access: "full" },
    ] }) })));
  });
  it("shows grounding decisions independently of the input route", () => {
    render(<ComputerRunItem run={{ running: false, paused: false, steps: [{ ...step,
      path: "pixel-skylight", action: { kind: "click", element_index: 0, snapshot_id: 3 },
      executionFallbackReason: "ax_element_action_unavailable_or_rejected",
      grounding: [{ mode: "elements", usable_elements: 6, snapshot_id: 3, model_result: "index", raw_reply: "0", element_index: 0 }],
    }] }} />);
    expect(screen.getByText("Grounding decisions")).toBeTruthy();
    expect(screen.getByText("Input: pixel-skylight")).toBeTruthy();
    expect(screen.getByText(/Pixel fallback: ax_element_action_unavailable_or_rejected/)).toBeTruthy();
    expect(screen.getByText(/"usable_elements": 6/)).toBeTruthy();
    expect(screen.getByText(/"kind": "click"/).textContent).toContain('"element_index": 0');
  });
  it("defaults to background and persists explicit fallback and cursor settings", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={save} />);
    expect((screen.getByLabelText("Input mode") as HTMLSelectElement).value).toBe("background");
    expect(screen.getByLabelText("Allow foreground fallback (approval required for each step)").getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByLabelText("Show virtual cursor").getAttribute("aria-pressed")).toBe("true");
    fireEvent.change(screen.getByLabelText("Input mode"), { target: { value: "foreground" } });
    fireEvent.click(screen.getByLabelText("Allow foreground fallback (approval required for each step)"));
    fireEvent.click(screen.getByLabelText("Show virtual cursor"));
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ mode: "foreground", allowForegroundFallback: true, virtualCursor: false }) })));
  });
  it.each([
    { screen: false, accessibility: false },
    { screen: true, accessibility: false },
    { screen: false, accessibility: true },
    { screen: true, accessibility: true },
  ])("requests access and opens only missing panes: %j", async permissions => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions: { screen: false, accessibility: false } });
    vi.mocked(computerRequestPermissions).mockResolvedValue(permissions);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Request access" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false));
    expect(computerRequestPermissions).toHaveBeenCalledTimes(1);
    for (const pane of ["screen", "accessibility"] as const) {
      const call = expect(computerRequest);
      if (permissions[pane]) call.not.toHaveBeenCalledWith({ action: "permissions", pane });
      else call.toHaveBeenCalledWith({ action: "permissions", pane });
      expect(screen.getByText(`${pane === "screen" ? "Screen Recording" : "Accessibility"}: ${permissions[pane] ? "Granted" : "Not granted"}`)).toBeTruthy();
    }
  });
  it.each([false, true])("requests once on enable only when permissions are missing (granted: %s)", async granted => {
    const permissions = { screen: granted, accessibility: granted };
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions });
    vi.mocked(computerRequestPermissions).mockResolvedValue(permissions);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByLabelText("Enable Computer Use"));
    await waitFor(() => expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false));
    expect(computerRequestPermissions).toHaveBeenCalledTimes(granted ? 0 : 1);
    fireEvent.click(screen.getByLabelText("Enable Computer Use"));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Refresh permissions" })); });
    expect(computerRequestPermissions).toHaveBeenCalledTimes(granted ? 0 : 1);
  });
  it("explains Screen Recording restart and wires Quit & Reopen", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    expect(screen.getByText("Screen Recording takes effect only after quitting and reopening taucode.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Quit & Reopen" }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "restart" }));
  });
  it("shows request failures and allows retry", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [] });
    vi.mocked(computerRequestPermissions).mockRejectedValue(new Error("Permission request failed"));
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Request access" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Error: Permission request failed");
    expect((screen.getByRole("button", { name: "Request access" }) as HTMLButtonElement).disabled).toBe(false);
  });
  it("renders persisted tool steps and validates unknown output", () => {
    const run = parseComputerRun(JSON.stringify({ computerRun: { steps: [step], summary: "Done" } }));
    expect(run).not.toBeNull(); expect(parseComputerRun('{"computerRun":{"steps":[{}]}}')).toBeNull();
    render(<ComputerRunItem run={run!} />);
    expect(screen.getByText("Step 1: click")).toBeTruthy(); expect(screen.getByAltText("Screen before step 1")).toBeTruthy(); expect(screen.getByText("Done")).toBeTruthy();
  });
  it("loads a compact stored result without inline thumbnails", async () => {
    const { thumbnail: _thumbnail, ...compactStep } = step;
    const compact = { id: "stored-run", stored: true, steps: [compactStep], error: "Input interrupted" };
    vi.mocked(computerRequest).mockResolvedValue({ ...compact, steps: [step] });
    const run = parseComputerRun(JSON.stringify({ computerRun: compact }));
    expect(run).not.toBeNull();
    render(<ComputerRunItem run={run!} />);
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "runLog", id: "stored-run" }));
    expect(await screen.findByAltText("Screen before step 1")).toBeTruthy();
    expect(screen.getByText("Computer Use interrupted")).toBeTruthy();
    for (const invalid of [{ ...compact, stored: false }, { ...compact, id: "" }, { ...compact, steps: [null] }]) {
      expect(parseComputerRun(JSON.stringify({ computerRun: invalid }))).toBeNull();
    }
  });
  it("recovers structured output from a failed OMP MCP call", async () => {
    const run = { id: "failed-omp", stored: true, steps: [], instruction: "Append once", error: "Focus changed" };
    const item = buildConversationItem({ id: "call", type: "mcpToolCall", server: "Computer Use", tool: "computer_run_task", status: "failed", result: null, error: { message: `Error: ${JSON.stringify({ computerRun: run })}` } });
    expect(item?.kind).toBe("tool");
    if (item?.kind !== "tool") throw new Error("Expected tool item");
    const parsed = parseComputerRun(item.output);
    expect(parsed).toEqual(run);
    vi.mocked(computerRequest).mockResolvedValue(run);
    render(<ComputerRunItem run={parsed!} />);
    expect(await screen.findByRole("button", { name: "Copy continuation request" })).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toBe("Focus changed");
  });
  it("keeps large failed run references valid through transcript truncation", async () => {
    const run = { id: "long-failure", stored: true, instruction: "Find a line", error: "Disabled\nSidecar stderr: " + "diagnostic ".repeat(4000), steps: [{ ...step, recovered: true, error: "Disabled" }] };
    const item = buildConversationItem({ id: "call", type: "mcpToolCall", server: "Computer Use", tool: "computer_run_task", status: "failed", result: null, error: { message: `Error: ${JSON.stringify({ computerRun: run })}` } });
    if (!item) throw new Error("Expected tool item");
    const prepared = prepareThreadItems([item, ...Array.from({ length: 45 }, (_, i) => ({ id: `message-${i}`, kind: "message" as const, role: "user" as const, text: "Follow-up" }))]);
    const tool = prepared[0];
    if (tool.kind !== "tool") throw new Error("Expected tool item");
    expect(tool.output!.length).toBeLessThan(200);
    const parsed = parseComputerRun(tool.output);
    expect(parsed).toEqual({ id: run.id, stored: true, steps: [] });
    vi.mocked(computerRequest).mockResolvedValue(run);
    render(<ComputerRunItem run={parsed!} />);
    expect(await screen.findByText("Computer Use blocked")).toBeTruthy();
    expect(screen.getByText(/Not executed/)).toBeTruthy();
  });
  it("shows provider failures from a stored run in the thread item", async () => {
    const error = "Provider error: Model unavailable";
    vi.mocked(computerRequest).mockResolvedValue({ id: "run-id", steps: [{ ...step, action: { kind: "fail", reason: error }, error }], error });
    render(<ComputerRunItem run={{ id: "run-id", stored: true, steps: [] }} />);
    await screen.findByText("Step 1: fail");
    expect(screen.getAllByText(error).length).toBeGreaterThan(0);
    expect(computerRequest).toHaveBeenCalledWith({ action: "runLog", id: "run-id" });
  });
  it("shows approval reason and target without expanding the thread step", () => {
    render(<ComputerRunItem run={{ steps: [{ ...step, kind: "approval_wait", reason: "Destructive click", target: "Delete account", predictMs: 12, executeMs: 0, waitMs: 900 }] }} />);
    expect(screen.getByText("Approval required: Destructive click (Delete account)")).toBeTruthy();
    expect(screen.getByText("Step 1: Approval wait")).toBeTruthy();
    expect(screen.getByText("Predict: 12 ms · Execute: 0 ms · Wait: 900 ms")).toBeTruthy();
  });
  it.each([true, false])("routes one-action approval from the main UI (accept: %s)", async accept => {
    vi.mocked(computerRequest).mockResolvedValue({ running: true, steps: [step], approval: { id: "approval", reason: "Secure field", action: { kind: "type" }, app: "com.apple.TextEdit" } });
    render(<ComputerApprovals />);
    await screen.findByText("Computer Use approval needed");
    expect(screen.getByText("Secure field")).toBeTruthy();
    fireEvent.keyDown(window, { key: "Enter" });
    expect(computerRequest).not.toHaveBeenCalledWith(expect.objectContaining({ action: "approve" }));
    const globalShortcut = vi.fn();
    window.addEventListener("keydown", globalShortcut);
    fireEvent.keyDown(screen.getByText("Approve", { selector: "button" }), { key: "Enter" });
    window.removeEventListener("keydown", globalShortcut);
    expect(globalShortcut).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText(accept ? "Approve" : "Decline", { selector: "button" }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "approve", id: "approval", accept }));
    expect(screen.queryByText("Allow for session")).toBeNull();
  });
  it.each([["Allow for this session", {}], ["Always allow", { persist: "always" }]] as const)("asks once per app like Codex desktop (%s)", async (button, persist) => {
    vi.mocked(computerRequest).mockResolvedValue({ running: true, steps: [], approval: { id: "app1", kind: "app", app: "com.google.Chrome", name: "Google Chrome", reason: 'Allow Computer Use to use "Google Chrome"?', warning: "Allowing taucode to use this app adds risks such as prompt injection." } });
    render(<ComputerApprovals />);
    expect(await screen.findByText('Allow Computer Use to use "Google Chrome"?')).toBeTruthy();
    expect(screen.getByText(/prompt injection/)).toBeTruthy();
    expect(screen.queryByText("Approve")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: button }));
    await waitFor(() => expect(computerRequest).toHaveBeenCalledWith({ action: "approve", id: "app1", accept: true, ...persist }));
  });
  it("turns approvals on, then lists always-allowed apps and can remove one", async () => {
    let always = ["com.apple.Notes", "com.apple.Maps"];
    vi.mocked(computerRequest).mockImplementation(async p => {
      if (p.action === "appApprovals") return always as never;
      if (p.action === "forgetApp") { always = always.filter(a => a !== p.app); return null as never; }
      return { steps: [] } as never;
    });
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={{ computer: defaultComputerSettings } as AppSettings} onUpdateAppSettings={save} />);
    expect((screen.getByLabelText("App access") as HTMLSelectElement).value).toBe("ask");
    // Approvals are off by default, so there is no always-allowed list to manage.
    expect(screen.queryByText("Always allowed apps")).toBeNull();
    expect(screen.getByText(/Computer Use can use any installed app\. Password managers/)).toBeTruthy();
    expect(screen.getByLabelText("Ask for approval").getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(screen.getByLabelText("Ask for approval"));
    expect(await screen.findByText("com.apple.Notes")).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(screen.queryByText("com.apple.Notes")).toBeNull());
    expect(computerRequest).toHaveBeenCalledWith({ action: "forgetApp", app: "com.apple.Notes" });
    expect(screen.getByText("com.apple.Maps")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("App access"), { target: { value: "listed" } });
    expect(screen.queryByText("com.apple.Maps")).toBeNull();
    expect(screen.getByLabelText("Allowed app bundle IDs (one per line)")).toBeTruthy();
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ askApproval: true }) })));
  });
  it("shows registry models, coordinate presets and permission states", async () => {
    vi.mocked(computerRequest).mockResolvedValue({ steps: [], permissions: { screen: true, accessibility: false } });
    const settings = { computer: defaultComputerSettings, providers: [{ id: "local", name: "Local", enabled: true, kind: "ollama", models: [{ id: "vision", enabled: true }] }] } as AppSettings;
    const save = vi.fn().mockResolvedValue(undefined);
    render(<SettingsComputerSection appSettings={settings} onUpdateAppSettings={save} />);
    await screen.findByText("Screen Recording: Granted"); expect(screen.getByText("Accessibility: Not granted")).toBeTruthy();
    expect(screen.getAllByText("vision")).toHaveLength(2);
    expect((screen.getByLabelText("Grounding mode") as HTMLSelectElement).value).toBe("elements");
    expect(screen.queryByLabelText("Grounding coordinates")).toBeNull();
    fireEvent.change(screen.getByLabelText("Grounding mode"), { target: { value: "ui-tars" } });
    fireEvent.change(screen.getByLabelText("Grounding coordinates"), { target: { value: "1000x1000" } });
    fireEvent.click(screen.getByText("Save Computer Use"));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ computer: expect.objectContaining({ groundingMode: "ui-tars", groundingWidth: 1000, groundingHeight: 1000 }) })));
    fireEvent.click(screen.getByText("Open System Settings — Accessibility")); expect(computerRequest).toHaveBeenCalledWith({ action: "permissions", pane: "accessibility" });
  });
  it("backs off approval polling while idle or failing", async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(computerRequest).mockRejectedValue(new Error("down"));
      render(<ComputerApprovals />);
      await act(() => vi.advanceTimersByTimeAsync(10_000));
      expect(vi.mocked(computerRequest).mock.calls.length).toBeLessThanOrEqual(4);
      cleanup(); vi.mocked(computerRequest).mockReset().mockResolvedValue({ steps: [], running: false });
      render(<ComputerApprovals />);
      await act(() => vi.advanceTimersByTimeAsync(10_000));
      expect(vi.mocked(computerRequest).mock.calls.length).toBeLessThanOrEqual(6);
      cleanup(); vi.mocked(computerRequest).mockReset().mockResolvedValue({ steps: [], running: true });
      render(<ComputerApprovals />);
      await act(() => vi.advanceTimersByTimeAsync(2_000));
      expect(vi.mocked(computerRequest).mock.calls.length).toBeGreaterThanOrEqual(4);
    } finally { vi.useRealTimers(); }
  });
});
