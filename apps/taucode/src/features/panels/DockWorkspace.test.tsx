// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { WorkspaceShell, PanelToolRequest, useSidePanel, type PanelTool } from "./SidePanel";
import { usePanelVisibility } from "@/features/layout/hooks/usePanelVisibility";
import { dockGeometry, dockPanes, readPanel } from "./panelState";
import { browserNativeBounds, browserRequest, subscribeBrowser } from "@services/tauri";
import type { BrowserState } from "@/features/browser/browser";
import { FeatureSettings } from "@settings/features";
import type { AppSettings } from "@/types";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn(), subscribeBrowser: vi.fn(), openBrowserDownloads: vi.fn(), browserNativeBounds: vi.fn() }));
vi.mock("@/features/browser/BrowserAnnotations", () => ({ BrowserAnnotations: ({ onAdd }: { onAdd: (context: { text: string; image?: string }) => void }) => <button onClick={() => onAdd({ text: "Selected heading", image: "data:image/png;base64,cG5n" })}>Attach browser draft</button> }));
vi.mock("@services/events", () => ({ subscribeMenuViewAction: () => () => {}, subscribeAppServerEvents: () => () => {} }));
let host: BrowserState, callbacks: Set<(state: BrowserState) => void>, frames: Map<number, FrameRequestCallback>, serial: number;
const tools: PanelTool[] = [
  { id: "terminal", label: "Terminal", content: <input aria-label="Terminal command" defaultValue="" /> },
  { id: "git", label: "Changes", content: <p>Changed files</p> },
  { id: "files", label: "Files", content: <p>Workspace files</p> },
  { id: "sidechat", label: "Side chat", content: <p>Side conversation</p> },
  { id: "background", label: "Background tasks", content: <p>Running tasks</p> },
];
function Rail() {
  const panel = useSidePanel();
  return <><button onClick={() => panel?.openTool("git")}>Inspect changes</button><button onClick={() => { void panel?.openPage("http://localhost:47400"); }}>Open server</button></>;
}
const shell = (scope = "code:one") => <WorkspaceShell dock scope={scope} title="Code thread" tools={tools} sessionRail={<Rail />} onInterrupt={() => {}}>
  <div className="app layout-desktop"><aside className="sidebar"><button>Global navigation</button></aside><main className="main"><input aria-label="Conversation draft" /></main></div>
</WorkspaceShell>;
const pane = (id: string) => document.querySelector<HTMLElement>(`[data-dock-pane="${id}"]`)!;
const layout = () => readPanel("code:one").dock!;
async function frame() { await act(async () => { const batch = [...frames.values()]; frames.clear(); batch.forEach(callback => callback(0)); }); }
beforeEach(() => {
  localStorage.clear(); vi.clearAllMocks();
  vi.stubGlobal("innerWidth", 1600); host = { backend: "native", capabilities: { screenshot: true, evaluate: false }, tabs: [], active: "" }; callbacks = new Set(); frames = new Map(); serial = 0;
  vi.mocked(browserNativeBounds).mockResolvedValue(undefined);
  vi.mocked(subscribeBrowser).mockImplementation(async callback => { callbacks.add(callback); callback(host); return () => { callbacks.delete(callback); }; });
  vi.mocked(browserRequest).mockImplementation(async request => {
    if (request.action === "screenshot") return "cG5n";
    if (request.action === "new") { const id = `page-${host.tabs.length + 1}`; host = { ...host, active: id, tabs: [...host.tabs, { id, title: `Page ${host.tabs.length + 1}`, url: `https://page.test/${id}`, ready: true, loading: false }] }; }
    if (request.action === "activate") host = { ...host, active: String(request.id) };
    if (request.action === "navigate") host = { ...host, tabs: host.tabs.map(tab => tab.id === host.active ? { ...tab, url: String(request.url) } : tab) };
    if (request.action === "close") { const tabs = host.tabs.filter(tab => tab.id !== request.id); host = { ...host, tabs, active: host.active === request.id ? tabs[0]?.id ?? "" : host.active }; }
    callbacks.forEach(callback => callback(host));
  });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frames.set(++serial, callback); return serial; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  vi.stubGlobal("Image", class { src = ""; naturalWidth = 1000; naturalHeight = 700; async decode() {} });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const sidebar = localStorage.getItem("codexmonitor.sidebarCollapsed") === "true" || window.innerWidth < 760 ? 0 : 280;
    const width = window.innerWidth - sidebar;
    if (this.classList.contains("dock-stage")) return { x: sidebar, y: 100, left: sidebar, top: 100, width, height: 600 } as DOMRect;
    if (this.classList.contains("panel-resize-overlay")) return { x: 0, y: 0, width: window.innerWidth, height: 900 } as DOMRect;
    if (this.classList.contains("sidebar")) return { x: 0, y: 0, left: 0, top: 0, width: 280, height: 700 } as DOMRect;
    if (this.classList.contains("browser-native-slot") || this.classList.contains("browser-surface")) {
      const panel = this.closest<HTMLElement>("[data-dock-pane]")!;
      if (panel.hidden) return { x: 0, y: 0, width: 0, height: 0 } as DOMRect;
      return { x: sidebar + width * Number(panel.style.getPropertyValue("--dock-box-x")), y: 100 + 600 * parseFloat(panel.style.top) / 100, width: width * Number(panel.style.getPropertyValue("--dock-box-width")), height: 600 * parseFloat(panel.style.height) / 100 } as DOMRect;
    }
    return { x: 0, y: 0, width: 0, height: 0 } as DOMRect;
  });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("restores focus to the view toggle when closing a tiled pane", () => {
  render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  const command = screen.getByRole("textbox", { name: "Terminal command" });
  const close = screen.getByRole("button", { name: "Close Terminal pane" });
  close.focus(); fireEvent.click(close);
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(pane("terminal").hidden).toBe(true);
  expect(command.isConnected).toBe(true);
});

it.each(["Close Terminal pane", "Toggle Terminal"])("keeps Terminal closed after %s and remount with the legacy request controller", async closeLabel => {
  const onClose = vi.fn();
  function SyncedTerminal() {
    const visibility = usePanelVisibility({ isCompact: false, activeWorkspaceId: "workspace", setActiveTab: () => {}, setDebugOpen: () => {} });
    return <WorkspaceShell dock scope="code:one" title="Code thread" onInterrupt={() => {}} tools={[{
      id: "terminal", label: "Terminal", content: <input aria-label="Terminal command" />,
      onOpen: visibility.openTerminal, onClose: () => { onClose(); visibility.closeTerminal(); },
    }]}><PanelToolRequest id="terminal" requestKey={visibility.terminalOpen} /><main className="main">Conversation</main></WorkspaceShell>;
  }
  const view = render(<SyncedTerminal />);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  const command = await screen.findByRole("textbox", { name: "Terminal command" });
  fireEvent.change(command, { target: { value: "retained" } });
  fireEvent.click(screen.getByRole("button", { name: closeLabel }));
  expect(onClose).toHaveBeenCalledOnce();
  expect(command.isConnected).toBe(true);
  expect((command as HTMLInputElement).value).toBe("retained");
  expect(readPanel("code:one").tabs.some(tab => tab.kind === "terminal")).toBe(false);
  view.unmount(); render(<SyncedTerminal />);
  expect(screen.getByRole("button", { name: "Toggle Terminal" }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.queryByRole("textbox", { name: "Terminal command" })).toBeNull();
});

it("does not restore a stored active terminal that is absent from the saved dock tree", () => {
  const onOpen = vi.fn();
  localStorage.setItem("taucode.panel.code:one", JSON.stringify({
    tabs: [{ id: "terminal", kind: "terminal", title: "Terminal" }], active: "terminal", open: true,
    dock: { tree: { type: "pane", id: "conversation" }, expanded: null },
  }));
  render(<WorkspaceShell dock scope="code:one" title="Code thread" onInterrupt={() => {}} tools={[{ id: "terminal", label: "Terminal", content: <p>Terminal session</p>, onOpen }]}><main>Conversation</main></WorkspaceShell>);
  expect(onOpen).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Toggle Terminal" }).getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(onOpen).toHaveBeenCalledOnce();
});

it("uses one browser header with manual tab navigation and closes the focused tab", async () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  const slot = await screen.findByLabelText("Native browser page");
  fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
  await screen.findByRole("tab", { name: "Page 2", selected: true });
  fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
  const third = await screen.findByRole("tab", { name: "Page 3", selected: true });
  const first = screen.getByRole("tab", { name: "Page 1" });
  const second = screen.getByRole("tab", { name: "Page 2" });
  const header = pane("browser").querySelector(".dock-pane-header")!;
  expect(header.contains(screen.getByRole("tablist", { name: "Browser pages" }))).toBe(true);
  expect(header.querySelector("strong")).toBeNull();
  expect(third.getAttribute("tabindex")).toBe("0");
  expect(first.getAttribute("tabindex")).toBe("-1");
  act(() => third.focus()); fireEvent.keyDown(third, { key: "Home" });
  expect(document.activeElement).toBe(first);
  expect(third.getAttribute("aria-selected")).toBe("true");
  expect(first.getAttribute("tabindex")).toBe("0");
  fireEvent.keyDown(first, { key: "ArrowLeft" }); expect(document.activeElement).toBe(third);
  fireEvent.keyDown(third, { key: "ArrowRight" }); expect(document.activeElement).toBe(first);
  fireEvent.keyDown(first, { key: "End" }); expect(document.activeElement).toBe(third);
  fireEvent.click(first);
  expect(document.activeElement).toBe(first);
  expect(await screen.findByRole("tab", { name: "Page 1", selected: true })).toBe(first);
  fireEvent.keyDown(first, { key: "ArrowRight" });
  fireEvent.keyDown(second, { key: "Delete" });
  await waitFor(() => expect(screen.queryByRole("tab", { name: "Page 2" })).toBeNull());
  expect(document.activeElement).toBe(third);
  expect(screen.getByRole("tab", { name: "Page 1", selected: true })).toBe(first);
  expect(screen.getByLabelText("Native browser page")).toBe(slot);
});

it("supports Backspace tab close and returns focus to Browser when the final tab closes", async () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  const first = await screen.findByRole("tab", { name: "Page 1" });
  act(() => first.focus()); fireEvent.keyDown(first, { key: "Backspace" });
  await waitFor(() => expect(pane("browser").hidden).toBe(true));
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Toggle Browser" }));
});

it("tiles tools independently, restores filled layout, and preserves live content through closing/reopening", () => {
  render(shell());
  expect(pane("session").hidden).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  const command = screen.getByRole("textbox", { name: "Terminal command" });
  fireEvent.change(command, { target: { value: "npm run dev" } });
  fireEvent.click(screen.getByRole("button", { name: "Toggle Files" }));
  expect(pane("terminal").hidden).toBe(false); expect(pane("files").hidden).toBe(false);
  const before = layout().tree;
  fireEvent.click(screen.getByRole("button", { name: "Expand Terminal" }));
  expect(screen.getByRole("textbox", { name: "Conversation draft" }).hasAttribute("inert")).toBe(false);
  expect(screen.getByRole("textbox", { name: "Conversation draft" }).closest("[inert]")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Global navigation" }).closest("[inert]")).toBeNull();
  expect(pane("files").hidden).toBe(true); expect(pane("session").hidden).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Restore Terminal" }));
  expect(layout().tree).toEqual(before); expect(pane("files").hidden).toBe(false);
  expect(command.closest("[inert]")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Close Terminal pane" }));
  expect(command.isConnected).toBe(true); expect(pane("terminal").hidden).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(screen.getByRole("textbox", { name: "Terminal command" })).toBe(command);
  expect((command as HTMLInputElement).value).toBe("npm run dev");
});
it("moves/reorders and resizes both split axes, persists by scope, and honors the legacy visibility toggle", () => {
  const view = render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  fireEvent.click(screen.getByRole("button", { name: "Toggle Files" }));
  const command = screen.getByRole("textbox", { name: "Terminal command" });
  fireEvent.contextMenu(screen.getByRole("button", { name: "Expand Terminal" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Move top" }));
  expect(dockGeometry(layout().tree).panes.terminal.y).toBe(0);
  const separators = screen.getAllByRole("separator");
  expect(new Set(separators.map(separator => separator.getAttribute("aria-orientation")))).toEqual(new Set(["horizontal", "vertical"]));
  for (const separator of separators) {
    const previous = Number(separator.getAttribute("aria-valuenow"));
    fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(Number(separator.getAttribute("aria-valuenow"))).toBeGreaterThan(previous);
  }
  const saved = layout(); view.rerender(shell("code:other"));
  expect(screen.queryByRole("textbox", { name: "Terminal command" })).toBeNull();
  view.rerender(shell()); expect(layout()).toEqual(saved);
  act(() => window.dispatchEvent(new CustomEvent("taucode:panel-toggle", { detail: false })));
  expect(pane("terminal").hidden).toBe(true); expect(pane("files").hidden).toBe(true);
  act(() => window.dispatchEvent(new CustomEvent("taucode:panel-toggle", { detail: true })));
  expect(pane("terminal").hidden).toBe(false); expect(pane("files").hidden).toBe(false);
  expect(layout()).toEqual(saved);
  expect(command.isConnected).toBe(false); // Switching scope resets scope-owned content.
});
it("keeps one native browser and selected page through tool selection, moving, filling and closing its group", async () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  const slot = await screen.findByLabelText("Native browser page"); await frame();
  const browserPane = pane("browser"), body = document.querySelector(".dock-body");
  expect(browserPane.parentElement).toBe(body);
  expect(document.querySelector(".dock-stage")?.childElementCount).toBe(0);
  fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
  await screen.findByRole("tab", { name: "Page 2", selected: true });
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(screen.getByRole("tab", { name: "Page 2", selected: true })).toBeTruthy();
  expect(host.active).toBe("page-2");
  fireEvent.click(screen.getByRole("tab", { name: "Page 1" }));
  await waitFor(() => expect(host.active).toBe("page-1"));
  fireEvent.contextMenu(screen.getByRole("button", { name: "Expand Browser" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Move top" })); await frame();
  expect(screen.getByLabelText("Native browser page")).toBe(slot);
  fireEvent.click(screen.getByRole("button", { name: "Expand Browser" })); await frame();
  expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Restore Browser" }));
  fireEvent.click(screen.getByRole("button", { name: "Close Browser pane" })); await frame();
  expect(slot.isConnected).toBe(true); expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(false);
  const count = host.tabs.length;
  fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" })); await frame();
  expect(screen.getAllByLabelText("Native browser page")).toEqual([slot]);
  expect(pane("browser")).toBe(browserPane); expect(browserPane.parentElement).toBe(body);
  expect(host.tabs).toHaveLength(count); expect(host.active).toBe("page-1");
  expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(true);
});
it("hides native content under a reopened narrow sidebar and through a pointer resize without losing the page", async () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  const slot = await screen.findByLabelText("Native browser page"); await frame();
  const separator = screen.getAllByRole("separator")[0];
  separator.setPointerCapture = vi.fn(); separator.hasPointerCapture = vi.fn(() => true); separator.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(separator, { pointerId: 1, button: 0, isPrimary: true, clientX: 1177, clientY: 100 });
  await frame(); expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(false);
  fireEvent.pointerMove(separator, { pointerId: 1, clientX: 1100, clientY: 100 });
  fireEvent.pointerUp(separator, { pointerId: 1 }); await frame();
  expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Expand Browser" }));
  vi.stubGlobal("innerWidth", 360); fireEvent(window, new Event("resize")); await frame();
  fireEvent.click(screen.getByRole("button", { name: "Expand sidebar" })); await frame();
  expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" })); await frame();
  expect(vi.mocked(browserNativeBounds).mock.calls[vi.mocked(browserNativeBounds).mock.calls.length - 1]?.[2]).toBe(true);
  expect(screen.getByLabelText("Native browser page")).toBe(slot);
});
it("clears a failed page-open error once that page recovers through Retry", async () => {
  const request = vi.mocked(browserRequest).getMockImplementation()!;
  vi.mocked(browserRequest).mockImplementation(async r => {
    if (r.action !== "new" || host.tabs.length) return request(r);
    host = { ...host, active: "page-1", tabs: [{ id: "page-1", title: "New tab", url: "about:blank", ready: false, loading: false, error: "Could not open this tab: injected test fault at layout. Choose Retry to try again." }] };
    callbacks.forEach(callback => callback(host));
    throw new Error("injected test fault at layout");
  });
  render(shell());
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open server" })); });
  await waitFor(() => expect(document.querySelector(".dock-error")?.textContent).toContain("injected test fault"));
  // The tab keeps its own error and Retry; the toast goes once that error clears (a failed retry sets it again).
  act(() => { host = { ...host, tabs: [{ ...host.tabs[0], error: "Still failing" }] }; callbacks.forEach(callback => callback(host)); });
  expect(document.querySelector(".dock-error")).toBeTruthy();
  act(() => { host = { ...host, tabs: [{ ...host.tabs[0], error: undefined, loading: true }] }; callbacks.forEach(callback => callback(host)); });
  expect(document.querySelector(".dock-error")).toBeNull();
});
it("keeps one panel tab when a failed page's state arrives after the open request fails", async () => {
  const request = vi.mocked(browserRequest).getMockImplementation()!;
  let late: (() => void) | undefined;
  vi.mocked(browserRequest).mockImplementation(async r => {
    if (r.action !== "new" || host.tabs.length) return request(r);
    host = { ...host, active: "page-1", tabs: [{ id: "page-1", title: "New tab", url: "about:blank", ready: false, loading: false, error: "Could not open this tab: injected. Choose Retry to try again." }] };
    late = () => callbacks.forEach(callback => callback(host));
    throw new Error("injected");
  });
  render(shell());
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open server" })); });
  await act(async () => { late!(); });
  const browserTabs = () => readPanel("code:one").tabs.filter(tab => tab.kind === "browser");
  expect(browserTabs()).toHaveLength(1);
  expect(browserTabs()[0].target).toBe("page-1");
});
it("reveals a hidden Browser when the rail opens a server", async () => {
  render(shell());
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open server" })); });
  await waitFor(() => expect(host.tabs[0]?.url).toBe("http://localhost:47400/"));
  fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  expect(dockPanes(layout().tree)).not.toContain("browser");
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open server" })); });
  await waitFor(() => expect(dockPanes(layout().tree)).toContain("browser"));
  expect(pane("browser").hidden).toBe(false);
  expect(screen.getByRole("button", { name: "Toggle Browser" }).getAttribute("aria-pressed")).toBe("true");
});
it("opens rail and secondary view actions and respects live feature gates", async () => {
  const view = render(shell()); fireEvent.click(screen.getByRole("button", { name: "Inspect changes" }));
  expect(pane("git").hidden).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Actions for View options" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Background tasks" }));
  expect(screen.getByText("Running tasks")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Open server" }));
  await waitFor(() => expect(host.tabs[0]?.url).toBe("http://localhost:47400/"));
  view.rerender(<FeatureSettings.Provider value={{ features: { terminal: false, browser: false } } as AppSettings}>{shell()}</FeatureSettings.Provider>);
  expect(screen.queryByRole("button", { name: "Toggle Terminal" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Toggle Browser" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Actions for View options" }));
  expect(within(screen.getByRole("menu", { name: "Actions for View options" })).getByRole("menuitem", { name: "Background tasks" })).toBeTruthy();
  expect(dockPanes(layout().tree)).toContain("conversation");
});

it("tiles a filled browser before forwarding its attachment to the visible draft callback", async () => {
  const add = vi.fn(() => {
    expect(document.querySelector(".main")?.hasAttribute("inert")).toBe(false);
    expect(layout().expanded).toBeNull();
    screen.getByRole("textbox", { name: "Conversation draft" }).focus();
  });
  const children = <div className="app layout-desktop"><aside className="sidebar"/><main className="main"><input aria-label="Conversation draft" /></main></div>;
  render(<WorkspaceShell dock scope="code:one" title="Thread" sessionRail={<Rail />} tools={tools} onInterrupt={() => {}} onBrowserContext={add}>{children}</WorkspaceShell>);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Browser" }));
  const slot = await screen.findByLabelText("Native browser page"), browserPane = pane("browser");
  fireEvent.click(screen.getByRole("button", { name: "Expand Browser" }));
  fireEvent.click(screen.getByRole("button", { name: "Annotate page" }));
  fireEvent.click(await screen.findByRole("button", { name: "Attach browser draft" }));
  expect(add).toHaveBeenCalledExactlyOnceWith({ text: "Selected heading", image: "data:image/png;base64,cG5n" });
  expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Conversation draft" }));
  fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
  await screen.findByRole("tab", { name: "Page 2", selected: true });
  const body = browserPane.parentElement!, slotParent = slot.parentElement;
  const measurement = body.querySelector(".dock-stage")!;
  expect(measurement.previousElementSibling).toBe(browserPane);
  const moves = new MutationObserver(() => {});
  moves.observe(body, { childList: true });
  fireEvent.click(screen.getByRole("button", { name: "Expand Browser" }));
  const records = moves.takeRecords(); moves.disconnect();
  expect(records.some(record => [...record.addedNodes].includes(browserPane))).toBe(true);
  expect(measurement.nextElementSibling).not.toBeNull();
  expect(body.querySelector(".dock-stage")).toBe(measurement);
  expect(pane("browser")).toBe(browserPane);
  expect(browserPane.parentElement).toBe(body);
  expect(screen.getByLabelText("Native browser page")).toBe(slot);
  expect(slot.parentElement).toBe(slotParent);
  expect(within(browserPane).getByRole("button", { name: "Restore Browser" })).toBeTruthy();
});

it("uses a full-width focused pane in compact layouts and restores the saved tile tree on widening", () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  const terminal = pane("terminal"), command = screen.getByRole("textbox", { name: "Terminal command" });
  fireEvent.change(command, { target: { value: "retained command" } });
  fireEvent.click(screen.getByRole("button", { name: "Toggle Files" }));
  const tree = layout().tree;
  vi.stubGlobal("innerWidth", 720); fireEvent(window, new Event("resize"));
  expect(pane("files").hidden).toBe(false); expect(pane("files").style.getPropertyValue("--dock-box-width")).toBe("1");
  expect(terminal.hidden).toBe(true); expect(screen.queryByRole("separator")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Show conversation" }));
  expect(screen.getByRole("textbox", { name: "Conversation draft" }).closest("[inert]")).toBeNull();
  expect(pane("files").hidden).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(terminal.hidden).toBe(false); expect(terminal.style.getPropertyValue("--dock-box-width")).toBe("1");
  vi.stubGlobal("innerWidth", 1600); fireEvent(window, new Event("resize"));
  expect(terminal.hidden).toBe(false); expect(pane("files").hidden).toBe(false);
  expect(layout().tree).toEqual(tree);
  expect(screen.getByRole("textbox", { name: "Terminal command" })).toBe(command);
  expect((command as HTMLInputElement).value).toBe("retained command");
});

it("migrates the selected legacy browser page independently from the first stored tab", async () => {
  host = { backend: "native", active: "page-2", tabs: [{ id: "page-1", title: "First", url: "https://one.test/", loading: false, ready: true }, { id: "page-2", title: "Selected", url: "https://two.test/", loading: false, ready: true }] };
  localStorage.setItem("taucode.panel.code:one", JSON.stringify({ open: true, active: "web-2", tabs: [{ id: "web-1", kind: "browser", target: "page-1", title: "First", url: "https://one.test/" }, { id: "web-2", kind: "browser", target: "page-2", title: "Selected", url: "https://two.test/" }] }));
  render(shell());
  await screen.findByRole("tab", { name: "Selected", selected: true });
  expect(host.active).toBe("page-2");
  expect(readPanel("code:one").browserActive).toBe("web-2");
});

it("keeps the full browser list and new-tab action outside the scrolling tabs", async () => {
  render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Open server" }));
  await screen.findByRole("tab", { name: "Page 1" });
  fireEvent.click(screen.getByRole("button", { name: "New browser tab" }));
  await screen.findByRole("tab", { name: "Page 2" });
  const list = screen.getByRole("combobox", { name: "All browser tabs" });
  expect(list.closest(".dock-browser-tabs")).toBeNull();
  expect(screen.getByRole("button", { name: "New browser tab" }).closest(".dock-browser-tabs")).toBeNull();
  fireEvent.change(list, { target: { value: readPanel("code:one").tabs.find(tab => tab.title === "Page 1")!.id } });
  await waitFor(() => expect(host.active).toBe("page-1"));
  act(() => { host = { ...host, tabs: host.tabs.map(page => page.id === "page-1" ? { ...page, loading: true } : page) }; callbacks.forEach(callback => callback(host)); });
  expect(screen.getByRole("tab", { name: "Page 1", selected: true }).getAttribute("aria-description")).toBe("Loading");
});
it("does not create a second page while native tab notifications lag behind the new-page response", async () => {
  vi.mocked(browserRequest).mockImplementation(async request => {
    if (request.action === "new") {
      const id = `page-${host.tabs.length + 1}`;
      host = { ...host, active: id, tabs: [...host.tabs, { id, title: "New tab", url: "about:blank", loading: true }] };
      return { id };
    }
    if (request.action === "navigate") host = { ...host, tabs: host.tabs.map(page => page.id === host.active ? { ...page, url: String(request.url), title: "Local preview", loading: false, ready: true } : page) };
    // Native subscriptions are asynchronous; command completion is not a state event.
  });
  render(shell());
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open server" })); });
  expect(vi.mocked(browserRequest).mock.calls.filter(([request]) => request.action === "new")).toHaveLength(1);
  act(() => { callbacks.forEach(callback => callback(host)); });
  expect(screen.getByRole("tab", { name: "Local preview", selected: true })).toBeTruthy();
  expect(readPanel("code:one").tabs.filter(tab => tab.kind === "browser")).toHaveLength(1);
});
it("keeps a newer user selection when delayed creation notifications arrive", async () => {
  render(shell());
  fireEvent.click(screen.getByRole("button", { name: "Open server" }));
  await screen.findByRole("tab", { name: "Page 1" });
  const first = readPanel("code:one").browserActive!;
  vi.mocked(browserRequest).mockImplementation(async request => {
    if (request.action === "new") host = { ...host, active: "page-2", tabs: [...host.tabs, { id: "page-2", title: "Page 2", url: "about:blank", loading: false, ready: true }] };
    if (request.action === "activate") { host = { ...host, active: String(request.id) }; callbacks.forEach(callback => callback(host)); }
  });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "New browser tab" })); });
  fireEvent.change(screen.getByRole("combobox", { name: "All browser tabs" }), { target: { value: first } });
  await act(async () => { callbacks.forEach(callback => callback(host)); });
  expect(readPanel("code:one").browserActive).toBe(first);
  expect(host.active).toBe("page-1");
});

it("paints dock geometry during a drag and persists only its final frame", async () => {
  const view = render(shell());
  const divider = screen.getByRole("separator", { name: "Resize dock split 1" });
  divider.setPointerCapture = vi.fn(); divider.hasPointerCapture = () => true; divider.releasePointerCapture = vi.fn();
  const saved = JSON.stringify(layout());
  const draft = screen.getByRole("textbox", { name: "Conversation draft" });
  fireEvent.change(draft, { target: { value: "keep me" } });
  fireEvent.pointerDown(divider, { pointerId: 1, button: 0, isPrimary: true, clientX: 1100 });
  fireEvent.pointerMove(divider, { pointerId: 1, clientX: 950, clientY: 100 });
  await frame();
  expect(JSON.stringify(layout())).toBe(saved);
  expect(Number(pane("session").style.getPropertyValue("--dock-box-x"))).toBeCloseTo((950 - 280) / 1320, 4);
  fireEvent.pointerMove(divider, { pointerId: 1, clientX: 1000, clientY: 100 });
  fireEvent.pointerUp(divider, { pointerId: 1 });
  expect(dockGeometry(layout().tree).panes.session.x / 100).toBeCloseTo((1000 - 280) / 1320, 4);
  expect(screen.getByRole("textbox", { name: "Conversation draft" })).toBe(draft);
  expect((draft as HTMLInputElement).value).toBe("keep me");
  const final = layout(); view.unmount(); render(shell());
  expect(layout()).toEqual(final);
});
