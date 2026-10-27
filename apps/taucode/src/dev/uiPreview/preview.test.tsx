// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, getRoles, within } from "@testing-library/react";
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

// Code exposes both a named dock pane and the Browser's own region.
function visibleBrowser() {
  const pane = document.querySelector<HTMLElement>('.dock-pane[data-dock-pane="browser"]');
  return (pane ? within(pane) : screen).getByRole("region", { name: "Browser" });
}

beforeEach(() => { localStorage.clear(); vi.stubGlobal("innerWidth", 1440); });
afterEach(async () => { cleanup(); localStorage.removeItem("taucode.browser.open"); await new Promise(resolve => setTimeout(resolve, 50)); clearMocks(); vi.unstubAllGlobals(); vi.unstubAllEnvs(); vi.restoreAllMocks(); });

it("hosts side chat only in the panel, with shared messages and composer, and opens an explicit split", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=code&thread=thread-rich&sidechat=thread-side");
  installUiPreview();
  render(<App />);
  await screen.findByRole("textbox", { name: "Side chat message" });
  const pane = document.querySelector(".side-chat-pane")!;
  expect(pane.closest('.dock-pane[data-dock-pane="sidechat"]')).toBeTruthy();
  expect(document.querySelector(".side-chat-split")).toBeNull();
  expect(document.querySelector(".main-header button[title='Side chat (⌘⇧K)']")).toBeNull();
  expect(pane.querySelector(".side-chat-messages .messages-inner")).toBeTruthy();
  expect(pane.querySelector(".composer-shell .reasoning-chip")).toBeTruthy();
  expect(pane.querySelector("select")).toBeNull();
  expect(within(pane as HTMLElement).queryByRole("button", { name: /Back/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Actions for side chat" }));
  expect(screen.getByRole("menuitem", { name: "Rename" })).toBeTruthy();
  expect(screen.getByRole("menuitem", { name: "Archive" })).toBeTruthy();
  fireEvent.click(screen.getByRole("menuitem", { name: "Open in split view" }));
  await waitFor(() => expect(document.querySelector(".side-chat-split .side-chat-pane")).toBeTruthy());
  expect(document.querySelector('.dock-pane[data-dock-pane="sidechat"] .side-chat-pane')).toBeNull();
}, 15000);

it.each([
  ["?mode=code&thread=thread-rich", "Update the greeting, run the tests"],
  ["?mode=chat&chat=chat-1", "A starting point"],
  ["?mode=work&task=task-needs-input", "Approval required"],
  ["?mode=code&thread=thread-rich&theme=light", "Update the greeting, run the tests"],
  ["?mode=chat&chat=chat-1&theme=light", "A starting point"],
  ["?mode=work&task=task-needs-input&theme=light", "Approval required"],
  ["?settings=display&theme=light", "Display"],
  ["?settings=appearance&theme=glass-dark", "Choose a palette"],
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
    if (query.includes("mode=code")) {
      const views = screen.getByRole("toolbar", { name: "Workspace views" });
      expect(views.closest(".workspace-titlebar")).toBeTruthy();
      expect(within(views).getByRole("button", { name: "Toggle session rail" })).toBeTruthy();
      expect(document.querySelector('.dock-pane[data-dock-pane="session"]')).toBeTruthy();
    } else {
      const toggle = screen.getByRole("button", { name: "Toggle side panel" });
      expect(toggle.closest(".workspace-titlebar")).toBeTruthy();
    }
    expect(document.querySelector(".browser-toggle")).toBeNull();
    if (query.includes("mode=code")) {
      expect(screen.getByPlaceholderText("Ask taucode to do something…").closest(".squircle")).toBeTruthy();
      expect(document.querySelectorAll(".workspace-card.squircle")).toHaveLength(3);
      expect(document.querySelector(".sidebar .squircle .squircle")).toBeNull();
      expect(screen.getByRole("button", { name: "Orbit" }).closest(".workspace-card")?.querySelector(".workspace-card-content.collapsed")).toBeTruthy();
    }
    if (query.includes("mode=work")) {
      await waitFor(() => expect(document.querySelectorAll(".work-space.squircle")).toHaveLength(2));
      expect(document.querySelector(".sidebar .squircle .squircle")).toBeNull();
    }
    if (query.includes("composer=attachments")) expect(document.querySelectorAll(".chat-attachments li")).toHaveLength(2);
    if (query.includes("task=task-running")) {
      expect(document.querySelector(".work-space .thread-row .thread-status.work-status-running")).toBeTruthy();
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

it.each(["code", "chat", "work"])("opens and restores Browser through the %s workspace controls", async mode => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=${mode}&thread=thread-rich`);
  installUiPreview();
  render(<App />);
  if (mode === "code") await screen.findByText(/Update the greeting, run the tests/);
  if (mode === "chat") await screen.findByText(/A starting point/);
  if (mode === "work") await screen.findByRole("tab", { name: "Task" });
  if (mode === "code") {
    const views = screen.getByRole("toolbar", { name: "Workspace views" });
    const toggle = within(views).getByRole("button", { name: "Toggle Browser" });
    expect(document.querySelectorAll(".workspace-titlebar .lucide-globe")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Toggle side panel" })).toBeNull();
    fireEvent.click(toggle);
    expect(await waitFor(visibleBrowser)).toBeTruthy();
    expect(document.querySelector('.dock-pane[data-dock-pane="browser"]')?.hasAttribute("hidden")).toBe(false);
    await waitFor(() => expect((toggle as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(toggle);
    expect(screen.queryByRole("region", { name: "Browser" })).toBeNull();
  } else {
    expect(document.querySelector(".workspace-titlebar .lucide-globe, .main-header .lucide-globe")).toBeNull();
    expect(screen.queryByRole("button", { name: "Browser" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Header actions" })).toBeNull();
    const toggle = screen.getByRole("button", { name: "Toggle side panel" });
    if (toggle.getAttribute("aria-expanded") !== "true") fireEvent.click(toggle);
    fireEvent.click(screen.getByRole("button", { name: "New panel tab" }));
    fireEvent.click(screen.getByRole("button", { name: /New page/ }));
    expect(await waitFor(visibleBrowser)).toBeTruthy();
    fireEvent.click(toggle);
    expect(screen.queryByRole("region", { name: "Browser" })).toBeNull();
  }
  fireEvent.keyDown(window, { key: "b", metaKey: true, shiftKey: true });
  expect(await waitFor(visibleBrowser)).toBeTruthy();
});

it("keeps Work title actions and returns overflow focus on Escape at narrow widths", async () => {
  vi.stubGlobal("innerWidth", 640);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=work&task=task-running");
  installUiPreview(); render(<App />);
  await screen.findByRole("button", { name: "Stop task" });
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  const trigger = screen.getByRole("button", { name: "Actions for thread" });
  fireEvent.click(trigger);
  const menu = screen.getByRole("menu", { name: "Actions for thread" });
  expect(menu.contains(document.activeElement)).toBe(true);
  expect(screen.getByRole("button", { name: "Stop task" }).textContent).toBe("Stop");
  expect(screen.getByRole("button", { name: "Stop all tasks" }).textContent).toBe("");
  expect(document.querySelector(".workspace-titlebar .thread-state-chip.processing")?.textContent).toBe("running");
  fireEvent.keyDown(menu, { key: "Escape" });
  expect(screen.queryByRole("menu", { name: "Actions for thread" })).toBeNull();
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
  await screen.findByRole("textbox", { name: "Message" });
  await waitFor(() => expect(document.querySelector(".titlebar-title")?.textContent).not.toBe("taucode"));
  fireEvent.click(await screen.findByRole("button", { name: "Toggle Changes" }));
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

it("honors the providers settings deep link", async () => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?settings=providers");
  installUiPreview(); render(<App />);
  await screen.findByRole("dialog", { name: "Settings" });
  await waitFor(() => expect(screen.getByRole("button", { name: "Providers" }).getAttribute("aria-current")).toBe("page"));
});
it.each(["new", "browser"])("renders the %s panel preview fixture", async panel => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", `/?mode=chat&chat=chat-1&panel=${panel}`);
  installUiPreview(); render(<App />);
  await screen.findByText(/A starting point/);
  expect(screen.getByRole("complementary", { name: "Side panel" })).toBeTruthy();
  if (panel === "browser") expect(await screen.findByRole("tab", { name: "Example" })).toBeTruthy();
  else {
    expect(screen.getByRole("heading", { name: "Tools" })).toBeTruthy();
    expect(document.querySelector(".side-panel-tools > button.squircle")).toBeTruthy();
  }
});

it.each(["?mode=code&thread=thread-plan&panel=new", "?mode=chat&chat=chat-1", "?mode=work&task=task-running", "?settings=display", "?settings=providers"])("names the controls at %s", async query => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/" + query); installUiPreview(); render(<App />);
  await screen.findByRole("button", { name: "Open settings" });
  if (query.includes("settings")) await screen.findByRole("dialog", { name: "Settings" });
  else if (query.includes("mode=code")) await screen.findByRole("textbox", { name: "Message" });
  else if (query.includes("mode=chat")) await screen.findByText(/A starting point/);
  else await screen.findByText("Review the notes, identify the key findings, and write a concise report in outputs/.");
  const unnamed = Object.entries(getRoles(document.body)).filter(([role]) => ["button", "textbox", "searchbox", "combobox", "checkbox", "spinbutton", "slider"].includes(role)).flatMap(([role, elements]) => {
    const named = new Set(screen.queryAllByRole(role, { name: /\S/ }));
    return elements.filter(element => !named.has(element)).map(element => element.outerHTML.slice(0, 240));
  });
  expect(unnamed).toEqual([]);
});

it.each(["chat", "work"])("keeps %s actions in the title row without a secondary header", async mode => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", mode === "chat" ? "/?mode=chat&chat=chat-1" : "/?mode=work&task=task-running");
  installUiPreview(); render(<App />);
  if (mode === "chat") await screen.findByText(/A starting point/);
  else await screen.findByRole("button", { name: "Stop task" });
  expect(document.querySelector(".chat-main .main-topbar")).toBeNull();
  if (mode === "work") expect(screen.getByRole("button", { name: "Stop task" }).closest(".workspace-titlebar")).toBeTruthy();
  else {
    fireEvent.click(screen.getByRole("button", { name: "Actions for thread" }));
    expect(screen.getByRole("menuitem", { name: "Export" })).toBeTruthy();
  }
});
it("uses desktop Code chrome and a focused dock pane at 960px", async () => {
  vi.stubGlobal("innerWidth", 960); vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?mode=code&thread=thread-rich&panel=new");
  installUiPreview(); render(<App />);
  await screen.findByText(/Update the greeting, run the tests/);
  expect(document.querySelector(".tablet-nav")).toBeNull();
  expect(document.querySelector(".app.layout-desktop")).toBeTruthy();
  expect(document.querySelector(".dock-workspace.is-compact")).toBeTruthy();
  expect(screen.queryByRole("dialog", { name: "Side panel" })).toBeNull();
  const input = screen.getByPlaceholderText("Ask taucode to do something…");
  fireEvent.change(input, { target: { value: "Keep this draft" } });
  fireEvent.click(screen.getByRole("button", { name: "Toggle session rail" }));
  expect(document.querySelector('.dock-pane[data-dock-pane="session"]')?.hasAttribute("hidden")).toBe(false);
  expect(input.closest(".main")?.hasAttribute("inert")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Show conversation" }));
  expect(input.closest(".main")?.hasAttribute("inert")).toBe(false);
  expect((input as HTMLTextAreaElement).value).toBe("Keep this draft");
});

it.each(["code", "chat", "work"].flatMap(mode => [760, 960, 1180, 1440].map(width => ({ mode, width }))))("keeps the $mode conversation beside its sidebar in a $width px shell", async ({ mode, width }) => {
  vi.stubEnv("MODE", "uipreview");
  // The container deliberately differs from innerWidth (1440).
  const rect = HTMLElement.prototype.getBoundingClientRect;
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.classList.contains("workspace-shell")) return { width, height: 700, top: 0, left: 0, right: width, bottom: 700, x: 0, y: 0, toJSON: () => ({}) };
    if (this.classList.contains("dock-stage")) {
      const shell = this.closest<HTMLElement>(".workspace-shell")!;
      const rail = shell.querySelector(".app.layout-tablet") ? 72 : 0;
      const left = parseFloat(shell.style.getPropertyValue("--layout-sidebar-column")) + rail;
      return { width: width - left, height: 700, top: 0, left, right: width, bottom: 700, x: left, y: 0, toJSON: () => ({}) };
    }
    return rect.call(this);
  });
  const query = mode === "code" ? "mode=code&thread=thread-rich" : mode === "chat" ? "mode=chat&chat=chat-1" : "mode=work&task=new";
  window.history.replaceState(null, "", "/?" + query);
  installUiPreview(); render(<App />);
  const input = mode === "code" ? await screen.findByPlaceholderText("Ask taucode to do something…") : await screen.findByRole("textbox", { name: mode === "chat" ? "Message" : "Task brief" });
  const shell = document.querySelector<HTMLElement>(".workspace-shell")!;
  await waitFor(() => expect(shell.style.getPropertyValue("--sidebar-width")).toBe(`${Math.min(280, width - 520)}px`));
  const conversation = input.closest(mode === "code" ? ".main" : ".chat-main")!;
  expect(conversation).toBeTruthy();
  expect(conversation.querySelector(mode === "code" ? ".messages" : ".chat-scroll")).toBeTruthy();
  expect(input.closest(".composer-shell")).toBeTruthy();
  const sidebar = conversation.parentElement!.querySelector(":scope > .sidebar")!;
  expect(sidebar).toBeTruthy();
  if (mode === "code") {
    const compact = width - parseFloat(shell.style.getPropertyValue("--layout-sidebar-column")) < 760;
    expect(shell.classList.contains("is-compact")).toBe(compact);
    expect(shell.style.getPropertyValue("--right-panel-width")).toBe("0px");
    expect(parseFloat(shell.style.getPropertyValue("--dock-conversation-width"))).toBeGreaterThan(0);
    fireEvent.change(input, { target: { value: "Keep this draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
    await waitFor(visibleBrowser);
    const browserPane = shell.querySelector<HTMLElement>('.dock-pane[data-dock-pane="browser"]')!;
    expect(browserPane.hidden).toBe(false);
    expect(Number(browserPane.style.getPropertyValue("--dock-box-width"))).toBeGreaterThan(0);
    expect(screen.queryByRole("dialog", { name: "Side panel" })).toBeNull();
    expect(conversation.hasAttribute("inert")).toBe(compact);
    if (compact) {
      expect(browserPane.style.getPropertyValue("--dock-box-width")).toBe("1");
      fireEvent.click(screen.getByRole("button", { name: "Show conversation" }));
      expect(conversation.hasAttribute("inert")).toBe(false);
      expect(browserPane.hidden).toBe(true);
    }
    expect(conversation.contains(input)).toBe(true);
    expect((input as HTMLTextAreaElement).value).toBe("Keep this draft");
    return;
  }
  for (const panelOpen of [false, true]) {
    if (panelOpen) fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
    const sidebarWidth = parseFloat(shell.style.getPropertyValue("--sidebar-width"));
    const sidebarColumn = parseFloat(shell.style.getPropertyValue("--layout-sidebar-column"));
    const panel = shell.querySelector<HTMLElement>(".tabbed-side-panel")!;
    const panelColumn = panelOpen && !panel.classList.contains("is-overlay") ? parseFloat(shell.style.getPropertyValue("--right-panel-width")) : 0;
    expect(sidebarColumn).toBe(sidebarWidth);
    expect(sidebarWidth).toBeLessThanOrEqual(280);
    expect(width - sidebarColumn - panelColumn).toBeGreaterThanOrEqual(520);
    expect(conversation.contains(input)).toBe(true);
  }
});

it.each(["code", "chat", "work"].flatMap(mode => [500, 740].map(width => ({ mode, width }))))("reopens the $mode sidebar as a sheet without replacing the conversation at $width px", async ({ mode, width }) => {
  vi.stubGlobal("innerWidth", width); vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/?" + (mode === "code" ? "mode=code&thread=thread-rich" : mode === "chat" ? "mode=chat&chat=chat-1" : "mode=work&task=new"));
  installUiPreview(); render(<App />);
  const input = mode === "code" ? await screen.findByPlaceholderText("Ask taucode to do something…") : await screen.findByRole("textbox", { name: mode === "chat" ? "Message" : "Task brief" });
  const shell = document.querySelector<HTMLElement>(".workspace-shell")!;
  expect(shell.classList.contains("sidebar-overlay")).toBe(true);
  expect(shell.style.getPropertyValue("--layout-sidebar-column")).toBe("0px");
  fireEvent.click(await screen.findByRole("button", { name: "Expand sidebar" }));
  expect(screen.getByRole("button", { name: "Collapse sidebar" })).toBeTruthy();
  expect(shell.style.getPropertyValue("--layout-sidebar-column")).toBe("0px");
  expect(input.closest(mode === "code" ? ".main" : ".chat-main")).toBeTruthy();
  expect(input.closest(".composer-shell")).toBeTruthy();
  expect(document.querySelector(".tablet-nav")).toBeNull();
});

it.each(["", "?setup=0", "?setup=1", "?setup=look", "?setup=provider", "?setup=engine", "?setup=project", "?setup=features", "?setup=permissions", "?setup=done"])("routes setup preview: %s", async query => {
  vi.stubEnv("MODE", "uipreview");
  window.history.replaceState(null, "", "/" + query);
  localStorage.setItem("taucode.setup-checklist.v1", "done");
  installUiPreview();
  const warn = vi.spyOn(console, "warn");
  render(<App />);
  await screen.findByRole("button", { name: "Open settings" });
  if (query && query !== "?setup=0") {
    const dialog = within(screen.getByRole("dialog"));
    const expected: Record<string, string> = { "1": "Welcome to taucode", look: "Pick a look.", provider: "Connect a model provider.", engine: "Choose your engine.", project: "Open your first project.", features: "Turn on what you want.", permissions: "Let taucode see and operate apps.", done: "You're all set." };
    expect(dialog.getByRole("heading", { level: 1 }).textContent?.trim()).toBe(expected[query.split("=")[1]]);
    if (query === "?setup=permissions") {
      await waitFor(() => expect(within(dialog.getByRole("group", { name: "Screen Recording" })).getByRole("status").textContent).toBe("Granted"));
      expect(within(dialog.getByRole("group", { name: "Accessibility" })).getByRole("status").textContent).toBe("Not granted");
    }
    fireEvent.click(dialog.getByRole("button", { name: "Set up later" }));
    fireEvent.click(dialog.getByRole("button", { name: "Confirm set up later" }));
  }
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(warn.mock.calls.filter(([message]) => String(message).includes("Unhandled IPC"))).toEqual([]);
});
