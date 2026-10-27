// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { WorkspaceShell, useSidePanel, type PanelTool } from "./SidePanel";
import { dockGeometry, dockPanes, readPanel } from "./panelState";
import { browserNativeBounds, browserRequest, subscribeBrowser } from "@services/tauri";
import type { BrowserState } from "@/features/browser/browser";
import { FeatureSettings } from "@settings/features";
import type { AppSettings } from "@/types";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn(), subscribeBrowser: vi.fn(), openBrowserDownloads: vi.fn(), browserNativeBounds: vi.fn() }));
vi.mock("@/features/browser/BrowserAnnotations", () => ({ BrowserAnnotations: ({ onAdd }: { onAdd: (context: { text: string; image?: string }) => void }) => <button onClick={() => onAdd({ text: "Selected heading", image: "data:image/png;base64,cG5n" })}>Attach browser draft</button> }));
vi.mock("@services/events", () => ({ subscribeAppServerEvents: () => () => {} }));
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
    if (request.action === "new") { const id = `page-${host.tabs.length + 1}`; host = { ...host, active: id, tabs: [...host.tabs, { id, title: `Page ${host.tabs.length + 1}`, url: "about:blank", ready: true, loading: false }] }; }
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
      return { x: sidebar + width * parseFloat(panel.style.left) / 100, y: 100 + 600 * parseFloat(panel.style.top) / 100, width: width * parseFloat(panel.style.width) / 100, height: 600 * parseFloat(panel.style.height) / 100 } as DOMRect;
    }
    return { x: 0, y: 0, width: 0, height: 0 } as DOMRect;
  });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
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
  await screen.findByLabelText("Native browser page");
  fireEvent.click(screen.getByRole("button", { name: "Expand Browser" }));
  fireEvent.click(screen.getByRole("button", { name: "Annotate page" }));
  fireEvent.click(await screen.findByRole("button", { name: "Attach browser draft" }));
  expect(add).toHaveBeenCalledExactlyOnceWith({ text: "Selected heading", image: "data:image/png;base64,cG5n" });
  expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Conversation draft" }));
});

it("uses a full-width focused pane in compact layouts and restores the saved tile tree on widening", () => {
  render(shell()); fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  const terminal = pane("terminal"), command = screen.getByRole("textbox", { name: "Terminal command" });
  fireEvent.change(command, { target: { value: "retained command" } });
  fireEvent.click(screen.getByRole("button", { name: "Toggle Files" }));
  const tree = layout().tree;
  vi.stubGlobal("innerWidth", 720); fireEvent(window, new Event("resize"));
  expect(pane("files").hidden).toBe(false); expect(pane("files").style.width).toBe("100%");
  expect(terminal.hidden).toBe(true); expect(screen.queryByRole("separator")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Show conversation" }));
  expect(screen.getByRole("textbox", { name: "Conversation draft" }).closest("[inert]")).toBeNull();
  expect(pane("files").hidden).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Toggle Terminal" }));
  expect(terminal.hidden).toBe(false); expect(terminal.style.width).toBe("100%");
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
