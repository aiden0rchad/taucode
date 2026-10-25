// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, act, cleanup } from "@testing-library/react";
import fixtures from "../../../../../docs/fixtures/diagnostics-redaction.json";
import { configureDiagnostics, diagnosticsEnabled, instrumentInvoke, openFeedback, record, redact } from "./client";
import { MessageRow } from "@/features/messages/components/MessageRows";
import { DiagnosticBoundary } from "./DiagnosticBoundary";
import { FeedbackComposer } from "./FeedbackComposer";
import { agentSnippets } from "./DiagnosticsSettings";
const send = vi.fn().mockResolvedValue(undefined);
function enable() { send.mockResolvedValue(undefined); vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] }); configureDiagnostics({ features: { diagnostics: true } }, send); }
afterEach(() => { cleanup(); configureDiagnostics({ features: { diagnostics: false } }, send); vi.useRealTimers(); vi.restoreAllMocks(); send.mockClear(); });
describe("diagnostics", () => {
  it("matches the shared Rust redaction fixtures", () => { for (const [input, output] of fixtures.fixtures) expect(redact(input, "/Users/test", ["real-key"])).toBe(output); });
  it("preserves identifiers while redacting known secrets", () => { for (const [input, output] of fixtures.identifierFixtures) expect(redact(input, "/Users/test", ["real-key"], true)).toBe(output); });
  it("does no work while off and cancels timers when disabled", () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] }); const spy = vi.spyOn(window, "addEventListener");
    configureDiagnostics({ features: { diagnostics: false } }, send); record({ kind: "error", area: "ui", code: "ui.test", message: "test" });
    expect(diagnosticsEnabled()).toBe(false); expect(vi.getTimerCount()).toBe(0); expect(spy).not.toHaveBeenCalled(); expect(send).not.toHaveBeenCalled();
    enable(); configureDiagnostics({ features: { diagnostics: false } }, send); expect(vi.getTimerCount()).toBe(0);
  });
  it("batches rejection, slow invoke and failure metadata without payloads", async () => {
    enable(); const rejection = new Event("unhandledrejection"); Object.assign(rejection, { reason: new Error("private message") }); window.dispatchEvent(rejection);
    const now = vi.spyOn(performance, "now").mockReturnValueOnce(0).mockReturnValueOnce(501);
    await expect(instrumentInvoke("get_git_status", async () => { throw new Error("private response"); })).rejects.toThrow("private response"); now.mockRestore();
    expect(send).not.toHaveBeenCalled(); await vi.advanceTimersByTimeAsync(2000);
    const events = send.mock.calls[0][0]; expect(events.map((e: { code: string }) => e.code)).toEqual(expect.arrayContaining(["ui.unhandled_rejection", "git.invoke_failed", "ipc.slow", "git.slow"])); expect(JSON.stringify(events)).not.toContain("private");
  });
  it("recovers a panel and records its crash", async () => {
    enable(); vi.spyOn(console, "error").mockImplementation(() => {});
    let crash = true; function Child() { if (crash) throw new Error("secret"); return <p>Recovered</p>; }
    render(<DiagnosticBoundary panel="sidebar"><Child /></DiagnosticBoundary>);
    expect(screen.getByRole("alert").textContent).toContain("This panel hit an error"); crash = false; fireEvent.click(screen.getByText("Reload panel")); expect(screen.getByText("Recovered")).toBeTruthy();
    await vi.advanceTimersByTimeAsync(2000); expect(send.mock.calls[0][0]).toEqual(expect.arrayContaining([expect.objectContaining({ code: "ui.render_crash" })]));
  });
  it("writes feedback and response flags", async () => {
    enable(); render(<FeedbackComposer />); act(() => openFeedback({ responseId: "response", threadId: "thread", turnId: "turn" }));
    fireEvent.change(screen.getByLabelText("Note"), { target: { value: "Please improve this" } }); fireEvent.click(screen.getByText("Save feedback")); await vi.advanceTimersByTimeAsync(2000);
    expect(send.mock.calls[0][0]).toEqual(expect.arrayContaining([expect.objectContaining({ kind: "feedback", code: "feedback.response_flag", context: expect.objectContaining({ threadId: "thread" }) })]));
  });
  it("opens the feedback composer from the response menu", () => {
    enable(); render(<><FeedbackComposer /><MessageRow diagnosticThreadId="thread" item={{ id: "response", kind: "message", role: "assistant", text: "Hello" }} isCopied={false} onCopy={() => {}} /></>);
    fireEvent.click(screen.getByText("Flag this response")); expect(screen.getByRole("dialog", { name: "Log an issue or idea" })).toBeTruthy();
  });
  it("uses resolved paths in setup snippets", () => { const result = agentSnippets("/Applications/Tau code/bin", "/Users/test/data"); expect(result.claude).toContain("'/Applications/Tau code/bin'"); expect(result.codex).toContain('command = "/Applications/Tau code/bin"'); expect(result.codex).toContain("/Users/test/data"); });
});
