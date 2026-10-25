// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { StrictMode, type ReactNode } from "react";
import App from "@/App";
import { installUiPreview } from "./index";

// jsdom has no Web Workers; render diffs on the main thread in these UI tests.
vi.mock("@pierre/diffs/react", async () => ({
  ...await vi.importActual<typeof import("@pierre/diffs/react")>("@pierre/diffs/react"),
  WorkerPoolContextProvider: ({ children }: { children: ReactNode }) => children,
}));

beforeEach(() => { vi.stubGlobal("innerWidth", 1440); });
afterEach(async () => { cleanup(); localStorage.removeItem("taucode.browser.open"); await new Promise(resolve => setTimeout(resolve, 50)); clearMocks(); vi.unstubAllGlobals(); vi.unstubAllEnvs(); vi.restoreAllMocks(); });

it.each([
  ["?mode=code&thread=thread-rich", "Update the greeting, run the tests"],
  ["?mode=chat&chat=chat-1", "A starting point"],
  ["?mode=work&task=task-needs-input", "Approval required"],
  ["?mode=code&thread=thread-rich&theme=light", "Update the greeting, run the tests"],
  ["?mode=chat&chat=chat-1&theme=light", "A starting point"],
  ["?mode=work&task=task-needs-input&theme=light", "Approval required"],
  ["?settings=display&theme=light", "Display"],
  ["?mode=chat&chat=new&composer=attachments", "brief.md"],
  ["?mode=chat&chat=chat-streaming", "Responding…"],
  ["?mode=work&task=task-running", "Prepare the launch brief"],
  ["?mode=work&task=new", "What would you like to make?"],
  ["?mode=code&thread=thread-empty", "What should we build?"],
])("mounts the real app at %s", async (query, text) => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/" + query);
  installUiPreview();
  const warn = vi.spyOn(console, "warn");
  const error = vi.spyOn(console, "error");
  render(<StrictMode><App /></StrictMode>);
  await waitFor(() => expect(screen.getAllByText(new RegExp(text)).length).toBeGreaterThan(0), { timeout: 10000 });
  if (query.includes("mode=")) {
    expect(screen.getByRole("navigation", { name: "App mode" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open settings" }).closest(".sidebar-bottom-rail")).toBeTruthy();
    const browser = screen.getByRole("button", { name: "Browser" });
    expect(browser.closest(".main-header-actions")).toBeTruthy();
    expect(document.querySelector(".browser-toggle")).toBeNull();
    if (query.includes("mode=code")) expect(screen.getByPlaceholderText("Ask taucode to do something…")).toBeTruthy();
    if (query.includes("composer=attachments")) expect(document.querySelectorAll(".chat-attachments li")).toHaveLength(2);
    if (query.includes("task=task-running")) {
      expect(screen.getByRole("img", { name: "Plan: 1 of 3 complete" })).toBeTruthy();
      expect(screen.getByRole("progressbar", { name: "Plan completion" })).toHaveProperty("value", 1);
    }
    if (query.includes("chat=chat-1")) {
      expect(screen.getByRole("heading", { level: 2, name: "A starting point" })).toBeTruthy();
      expect(screen.getByText("options").tagName).toBe("STRONG");
      expect(screen.getByRole("button", { name: "Copy code block" }).closest(".message .markdown .markdown-codeblock")).toBeTruthy();
      expect(document.querySelector(".chat-message .markdown > pre")).toBeNull();
    }
  }
  expect(warn.mock.calls.filter(([message]) => String(message).includes("Unhandled IPC"))).toEqual([]);
  expect(error).not.toHaveBeenCalled();
}, 15000);

it.each(["code", "chat", "work"])("toggles the browser from the %s header", async mode => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=${mode}&thread=thread-rich`);
  installUiPreview();
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Browser" }));
  expect(await screen.findByRole("button", { name: "Close browser pane" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Browser" }));
  expect(screen.queryByRole("button", { name: "Close browser pane" })).toBeNull();
});

it("opens header overflow actions and returns keyboard focus on Escape", async () => {
  vi.stubGlobal("innerWidth", 640);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=work&task=task-running");
  installUiPreview();
  render(<App />);
  const trigger = await screen.findByRole("button", { name: "Header actions" });
  fireEvent.click(trigger);
  const menu = screen.getByRole("dialog", { name: "Header actions" });
  expect(menu.contains(document.activeElement)).toBe(true);
  expect(screen.getByRole("button", { name: "Stop task" }).textContent).toBe("Stop");
  expect(screen.getByRole("button", { name: "Stop all tasks" }).textContent).toBe("");
  expect(document.querySelector(".main-header .thread-state-chip.processing")?.textContent).toBe("running");
  fireEvent.keyDown(menu, { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "Header actions" })).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

it.each(["chat", "work"])("opens shared settings without leaving %s", async mode => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=${mode}`);
  installUiPreview();
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));
  expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
  expect(screen.getByRole("button", { name: mode === "chat" ? "Chat" : "Work", pressed: true })).toBeTruthy();
  expect(screen.queryByText("Reduce transparency")).toBeNull();
});

it("shows one non-Git state without a branch chip or error toast", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=code&thread=thread-notes");
  installUiPreview();
  const error = vi.spyOn(console, "error");
  render(<App />);
  expect(await screen.findByText("This workspace is not a Git repository.")).toBeTruthy();
  expect(screen.getAllByRole("button", { name: "Initialize Git" })).toHaveLength(1);
  expect(document.querySelector(".workspace-branch-menu")).toBeNull();
  expect(screen.queryByText(/could not find repository/)).toBeNull();
  expect(error).not.toHaveBeenCalled();
});

it("sends a chat through real IPC and event subscriptions", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=chat&chat=chat-1");
  installUiPreview();
  render(<StrictMode><App /></StrictMode>);
  await screen.findByText(/A starting point/);
  const input = screen.getByRole("textbox", { name: "Message" });
  fireEvent.change(input, { target: { value: "A preview question" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await screen.findByText(/Preview response:/);
});

it.each(["chat", "work"])("keeps %s controls in two groups and shows available dictation", async mode => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=${mode}&chat=new&task=new&dictation=ready`);
  installUiPreview();
  render(<App />);
  const mic = await screen.findByRole("button", { name: "Start dictation" });
  const right = mic.closest(".composer-shell-actions");
  expect(right).toBeTruthy();
  expect(right?.querySelectorAll(".reasoning-control")).toHaveLength(1);
  expect(screen.queryByRole("combobox", { name: mode === "chat" ? "Chat model" : "Work model" })).toBeNull();
  expect(screen.getByRole("combobox", { name: "Access mode" }).closest(".composer-shell-left")).toBeTruthy();
  expect(document.querySelector(".composer-shell-toolbar")?.children).toHaveLength(2);
  if (mode === "work") {
    expect(screen.getByRole("combobox", { name: "Space" }).closest(".composer-shell-left")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Access mode" }).closest(".composer-shell-left")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Queue task" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add attachments and tools" }));
    expect(screen.getByRole("button", { name: "Queue task" }).closest('[role="dialog"]')).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(document.querySelector(".work-readonly-folders summary svg.lucide-chevron-down")).toBeTruthy();
    for (const [name, icon] of [["Organize a folder", "folder-tree"], ["Research brief", "file-search"], ["Spreadsheet from documents", "table"], ["Slide deck from notes", "presentation"], ["Clean up downloads", "trash-2"]]) {
      expect(screen.getByRole("button", { name }).querySelector(`.lucide-${icon}`)).toBeTruthy();
    }
  } else {
    for (const [name, icon] of [["Explore an idea", "lightbulb"], ["Plan a weekend", "calendar"], ["Write a story", "pen-line"]]) {
      expect(screen.getByRole("button", { name }).querySelector(`.lucide-${icon}`)).toBeTruthy();
    }
  }
});

it("keeps Chat actions outside the user bubble", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=chat&chat=chat-1");
  installUiPreview();
  render(<App />);
  const edit = await screen.findByRole("button", { name: "Edit and resend" });
  expect(edit.closest(".chat-message-body")).toBeNull();
  expect(edit.closest(".chat-message")?.querySelector(".chat-message-body .markdown")).toBeTruthy();
});
