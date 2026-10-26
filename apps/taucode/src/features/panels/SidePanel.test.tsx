// @vitest-environment jsdom
import { assertRowAlignment } from "@/test/rowAlignment.mjs";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { WorkspaceShell, type PanelTool } from "./SidePanel";
import { panelReducer, readPanel } from "./panelState";
import { browserRequest, subscribeBrowser } from "@services/tauri";
import type { BrowserState } from "@/features/browser/browser";
import { useSidebarToggles } from "@/features/layout/hooks/useSidebarToggles";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn(), subscribeBrowser: vi.fn(), openBrowserDownloads: vi.fn() }));
vi.mock("@services/events", () => ({ subscribeAppServerEvents: () => () => {} }));
let host: BrowserState;
let callbacks: Set<(state: BrowserState) => void>;
const tools: PanelTool[] = [
  { id: "git", label: "Changes", shortcut: "⌃⇧G", content: <p>Changed files</p> },
  { id: "terminal", label: "Terminal", shortcut: "⌃`", content: <p>Terminal session</p> },
  { id: "files", label: "Files", shortcut: "⌘P", content: <p>Project files</p> },
  { id: "sidechat", label: "Side chat", shortcut: "⌘⇧K", content: <p>Side conversation</p> },
  { id: "agents", label: "Agents", content: <p>Agent tree</p> },
  { id: "prompts", label: "Prompts", content: <p>Saved prompts</p> },
];
const shell = (scope = "code:one", items = tools) => <WorkspaceShell scope={scope} title="A thread" tools={items} onInterrupt={() => {}}><p>Conversation</p></WorkspaceShell>;
beforeEach(() => {
  localStorage.clear(); vi.clearAllMocks(); host = { tabs: [], active: "" }; callbacks = new Set();
  vi.mocked(subscribeBrowser).mockImplementation(async callback => { callbacks.add(callback); callback(host); return () => { callbacks.delete(callback); }; });
  vi.mocked(browserRequest).mockImplementation(async request => {
    if (request.action === "new") { const id = `page-${host.tabs.length + 1}`; host = { tabs: [...host.tabs, { id, title: "New page", url: "about:blank", loading: false }], active: id }; }
    if (request.action === "navigate") host = { ...host, tabs: host.tabs.map(tab => tab.id === host.active ? { ...tab, title: new URL(String(request.url)).hostname, url: String(request.url) } : tab) };
    if (request.action === "activate") host = { ...host, active: String(request.id) };
    if (request.action === "close") host = { ...host, tabs: host.tabs.filter(tab => tab.id !== request.id) };
    callbacks.forEach(callback => callback(host));
  });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});
afterEach(() => { assertRowAlignment(document.body); cleanup(); vi.restoreAllMocks(); vi.unstubAllEnvs(); vi.unstubAllGlobals(); });
it("opens the tool grid, keeps a shared title row, and persists tab selection, close and keyboard reorder per thread", async () => {
  const view = render(shell());
  const titlebar = screen.getByRole("button", { name: "Toggle side panel" }).closest("header")!;
  expect(titlebar.hasAttribute("data-tauri-drag-region")).toBe(true);
  expect(within(titlebar).getByRole("button", { name: "Collapse sidebar" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  for (const tool of tools) expect(screen.getByRole("button", { name: new RegExp(tool.label) })).toBeTruthy();
  expect(screen.getByText("⌃⇧G")).toBeTruthy();
  expect(screen.getByText("⌃`")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /Changes/ }));
  fireEvent.click(screen.getByRole("button", { name: "New panel tab" }));
  fireEvent.click(screen.getByRole("button", { name: /Terminal/ }));
  fireEvent.keyDown(screen.getByRole("tab", { name: "Terminal" }), { key: "ArrowLeft", altKey: true });
  expect(readPanel("code:one").tabs.map(tab => tab.id)).toEqual(["terminal", "git"]);
  view.rerender(shell("code:two"));
  expect(screen.queryByRole("tab")).toBeNull();
  view.rerender(shell());
  expect(screen.getByRole("tab", { name: "Terminal", selected: true })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Close Terminal" }));
  expect(readPanel("code:one").tabs.map(tab => tab.id)).toEqual(["git"]);
  view.unmount(); render(shell());
  expect(screen.getByRole("tab", { name: "Changes", selected: true })).toBeTruthy();
  expect(document.querySelector(".panel-tabs, .browser-dock")).toBeNull();
});
it("opens a URL as its own tab, switches browser pages and restores a missing host target", async () => {
  const view = render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  fireEvent.change(screen.getByRole("textbox", { name: "URL or search" }), { target: { value: "example.com" } });
  fireEvent.submit(screen.getByRole("textbox", { name: "URL or search" }).closest("form")!);
  await screen.findByRole("tab", { name: "example.com" });
  expect(browserRequest).toHaveBeenCalledWith({ action: "navigate", url: "https://example.com/" });
  await waitFor(() => expect(readPanel("code:one").tabs[0]).toMatchObject({ target: "page-1", url: "https://example.com/" }));
  fireEvent.click(screen.getByRole("button", { name: "New panel tab" }));
  fireEvent.click(screen.getByRole("button", { name: /New page/ }));
  await screen.findByRole("tab", { name: "New page" });
  fireEvent.click(screen.getByRole("tab", { name: "example.com" }));
  await waitFor(() => expect(host.active).toBe("page-1"));
  view.unmount(); host = { tabs: [], active: "" }; render(shell());
  await waitFor(() => expect(host.tabs[0]?.url).toBe("https://example.com/"));
  expect(screen.getByRole("button", { name: "Downloads" })).toBeTruthy();
});
it("opens Task by default and validates corrupt saved tab data", () => {
  render(shell("work:task", [{ id: "task", label: "Task", content: <p>Plan and outputs</p> }]));
  expect(screen.getByRole("tab", { name: "Task", selected: true })).toBeTruthy();
  expect(screen.getByText("Plan and outputs")).toBeTruthy();
  localStorage.setItem("taucode.panel.invalid", '{"tabs":[null,{}, {"id":5}],"open":true}');
  expect(readPanel("invalid").tabs).toEqual([]);
  const initial = { tabs: [{ id: "a", kind: "files", title: "Files" }, { id: "b", kind: "git", title: "Changes" }], active: "a", open: true };
  expect(panelReducer(initial, { type: "reorder", id: "a", before: "b", after: true }).tabs.map(tab => tab.id)).toEqual(["b", "a"]);
});
it("resizes with the keyboard and retains mounted tools when the panel is hidden", () => {
  vi.stubGlobal("innerWidth", 1600);
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  fireEvent.keyDown(screen.getByRole("separator", { name: "Resize side panel" }), { key: "ArrowLeft" });
  expect(localStorage.getItem("codexmonitor.rightPanelWidth")).toBe("380");
  fireEvent.click(screen.getByRole("button", { name: /Terminal/ }));
  const session = screen.getByText("Terminal session");
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  expect(session.isConnected).toBe(true);
  act(() => window.dispatchEvent(new Event("taucode:panel-toggle")));
  expect(screen.getByRole("tab", { name: "Terminal" })).toBeTruthy();
});

it.each([[1860, "ArrowLeft", 1100], [1360, "ArrowRight", 580]])("uses the dynamic keyboard limit at %ipx", (width, key, expected) => {
  vi.stubGlobal("innerWidth", width);
  localStorage.setItem("codexmonitor.rightPanelWidth", "900");
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  const handle = screen.getByRole("separator", { name: "Resize side panel" });
  expect(handle.getAttribute("aria-valuemax")).toBe(String(width - 280 - 480));
  for (let i = 0; i < (key === "ArrowLeft" ? 11 : 1); i++) fireEvent.keyDown(handle, { key });
  expect(localStorage.getItem("codexmonitor.rightPanelWidth")).toBe(String(expected));
  expect(handle.getAttribute("aria-valuenow")).toBe(String(expected));
});

it("supplies the terminal preview fixture without starting a real process", () => {
  vi.stubEnv("MODE", "uipreview"); window.history.replaceState(null, "", "/?panel=terminal");
  expect(readPanel("code:workspace:thread")).toMatchObject({ open: true, active: "terminal", tabs: [{ kind: "terminal", title: "Terminal" }] });
});

it("restores the saved side-chat target when its tab is selected", async () => {
  localStorage.setItem("taucode.panel.code:one", JSON.stringify({ open: true, active: "sidechat", tabs: [{ id: "sidechat", kind: "sidechat", title: "Side chat", target: "fork-123" }] }));
  const open = vi.fn();
  render(shell("code:one", [{ id: "sidechat", label: "Side chat", content: <p>Side chat</p>, onOpen: open }]));
  await waitFor(() => expect(open).toHaveBeenCalledWith(expect.objectContaining({ target: "fork-123" })));
  expect(readPanel("code:one").tabs[0].target).toBe("fork-123");
});

it("synchronizes title-bar visibility with existing panel shortcuts and file polling", () => {
  function App() {
    const { rightPanelCollapsed, expandRightPanel } = useSidebarToggles({ isCompact: false });
    return <><output>{rightPanelCollapsed ? "collapsed" : "open"}</output><button onClick={expandRightPanel}>Expand existing panel</button>{shell()}</>;
  }
  render(<App />);
  expect(screen.getByRole("status").textContent).toBe("collapsed");
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  expect(screen.getByRole("status").textContent).toBe("open");
  fireEvent.click(screen.getByRole("button", { name: "Expand existing panel" }));
  expect(screen.getByRole("button", { name: "Toggle side panel" }).getAttribute("aria-expanded")).toBe("true");
  expect(screen.queryByRole("button", { name: "Close side panel" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  expect(screen.getByRole("status").textContent).toBe("collapsed");
});

it("uses a dismissible overlay at 960px and returns to a column when enough space is available", () => {
  vi.stubGlobal("innerWidth", 960);
  const view = render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  expect(screen.getByRole("dialog", { name: "Side panel" }).classList.contains("is-overlay")).toBe(true);
  expect(view.container.querySelector(".workspace-main")?.hasAttribute("inert")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Dismiss side panel" }));
  expect(screen.queryByRole("dialog", { name: "Side panel" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  fireEvent.keyDown(window, { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "Side panel" })).toBeNull();
  vi.stubGlobal("innerWidth", 1440); fireEvent(window, new Event("resize"));
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  expect(screen.getByRole("complementary", { name: "Side panel" }).classList.contains("is-overlay")).toBe(false);
  expect(screen.queryByRole("button", { name: "Dismiss side panel" })).toBeNull();
  vi.unstubAllGlobals();
});

it.each([235, 360, 900])("resizes a saved %ipx panel in overlay mode at 995px", savedWidth => {
  vi.stubGlobal("innerWidth", 995);
  vi.stubGlobal("innerHeight", 530);
  localStorage.setItem("codexmonitor.sidebarWidth", "280");
  localStorage.setItem("codexmonitor.rightPanelWidth", String(savedWidth));
  const view = render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  const panel = screen.getByRole("dialog", { name: "Side panel" });
  const handle = screen.getByRole("separator", { name: "Resize side panel" });
  const initialWidth = Math.max(320, savedWidth);
  expect(handle.getAttribute("aria-valuenow")).toBe(String(initialWidth));
  expect(handle.getAttribute("aria-valuemin")).toBe("320");
  expect(handle.getAttribute("aria-valuemax")).toBe("915");
  handle.setPointerCapture = vi.fn();
  handle.hasPointerCapture = vi.fn(() => true);
  handle.releasePointerCapture = vi.fn();
  vi.spyOn(panel, "getBoundingClientRect").mockImplementation(() => ({ width: Number(handle.getAttribute("aria-valuenow")) }) as DOMRect);
  const drag = (clientX: number) => {
    fireEvent.pointerDown(handle, { pointerId: 1, button: 0, isPrimary: true, clientX: 500 });
    fireEvent.pointerMove(handle, { pointerId: 1, clientX });
    fireEvent.pointerUp(handle, { pointerId: 1 });
  };
  drag(380);
  expect(handle.getAttribute("aria-valuenow")).toBe(String(Math.min(915, initialWidth + 120)));
  expect(view.container.querySelector<HTMLElement>(".workspace-shell")?.style.getPropertyValue("--right-panel-width")).toBe(`${Math.min(915, initialWidth + 120)}px`);
  drag(1500);
  expect(handle.getAttribute("aria-valuenow")).toBe("320");
  expect(panel.classList.contains("is-overlay")).toBe(true);
});

it("switches to overlay exactly when the docked space falls below 320px", () => {
  vi.stubGlobal("innerWidth", 1080);
  localStorage.setItem("codexmonitor.sidebarWidth", "280");
  render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  const handle = screen.getByRole("separator", { name: "Resize side panel" });
  expect(screen.getByRole("complementary", { name: "Side panel" }).classList.contains("is-overlay")).toBe(false);
  expect(handle.getAttribute("aria-valuenow")).toBe("320");
  expect(handle.getAttribute("aria-valuemax")).toBe("320");
  vi.stubGlobal("innerWidth", 1079); fireEvent(window, new Event("resize"));
  expect(screen.getByRole("dialog", { name: "Side panel" }).classList.contains("is-overlay")).toBe(true);
  expect(handle.getAttribute("aria-valuenow")).toBe("360");
  expect(handle.getAttribute("aria-valuemax")).toBe("999");
  vi.stubGlobal("innerWidth", 1080); fireEvent(window, new Event("resize"));
  expect(screen.getByRole("complementary", { name: "Side panel" }).classList.contains("is-overlay")).toBe(false);
  expect(handle.getAttribute("aria-valuenow")).toBe("320");
  expect(localStorage.getItem("codexmonitor.rightPanelWidth")).toBe("360");
});
it("automatically collapses the sidebar below 760px and allows the title toggle to reopen it", () => {
  vi.stubGlobal("innerWidth", 740);
  render(shell());
  expect(localStorage.getItem("codexmonitor.sidebarCollapsed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Expand sidebar" }));
  expect(localStorage.getItem("codexmonitor.sidebarCollapsed")).toBe("false");
  vi.unstubAllGlobals();
});

it("removes disabled panel tools and browser subscriptions live", async () => {
  const { FeatureSettings } = await import("@settings/features");
  const view = render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  fireEvent.click(screen.getByRole("button", { name: /Changes/ }));
  view.rerender(<FeatureSettings.Provider value={{ features: { git: false, terminal: false, subagents: false, sideChats: false, browser: false } } as import("@/types").AppSettings}>{shell()}</FeatureSettings.Provider>);
  for (const name of [/Changes/, /Terminal/, /Agents/, /Side chat/, /New page/]) expect(screen.queryByRole("button", { name })).toBeNull();
  expect(screen.queryByRole("tab", { name: "Changes" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "URL or search" })).toBeNull();
  await waitFor(() => expect(callbacks.size).toBe(0));
  vi.mocked(browserRequest).mockClear();
  fireEvent.keyDown(window, { key: "b", metaKey: true, shiftKey: true });
  expect(browserRequest).not.toHaveBeenCalled();
});
