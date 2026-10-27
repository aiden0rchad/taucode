// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserPane } from "./BrowserPane";
import { drawBrowserFrame, normalizeBrowserUrl } from "./browser";
import { browserRequest, subscribeBrowser, browserNativeBounds } from "@services/tauri";
import type { BrowserState } from "./browser";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn(), subscribeBrowser: vi.fn(), browserNativeBounds: vi.fn(), openBrowserDownloads: vi.fn(), openBrowserUrl: vi.fn(), exportBrowserScreenshot: vi.fn() }));
const drawImage = vi.fn();
let receive: (s: BrowserState) => void;
const stop = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(browserRequest).mockResolvedValue(undefined);
  vi.mocked(browserNativeBounds).mockResolvedValue(undefined);
  vi.mocked(subscribeBrowser).mockImplementation(async fn => { receive = fn; return stop; });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage, clearRect: vi.fn() } as unknown as CanvasRenderingContext2D);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
const state: BrowserState = { tabs: [{ id: "one", title: "Example", url: "https://example.test/", loading: false }], active: "one", capabilities: { screenshot: true, evaluate: true } };
describe("Browser pane", () => {
  it("normalizes URLs and search terms, rejecting executable schemes", () => {
    const search = "https://search.test/?q={query}";
    expect(normalizeBrowserUrl("example.com/path",search)).toBe("https://example.com/path");
    expect(normalizeBrowserUrl("localhost:3000",search)).toBe("https://localhost:3000/");
    expect(normalizeBrowserUrl("find cats & dogs",search)).toBe("https://search.test/?q=find%20cats%20%26%20dogs");
    expect(() => normalizeBrowserUrl("javascript:alert(1)",search)).toThrow();
  });
  it("attaches, navigates, changes tabs and stops streaming on unmount", async () => {
    const view = render(<BrowserPane onClose={vi.fn()} onTakeOver={vi.fn()} />);
    await waitFor(() => expect(browserRequest).toHaveBeenCalledWith(expect.objectContaining({ action:"attach" })));
    act(() => receive(state));
    fireEvent.change(screen.getByLabelText("URL or search"), { target: { value:"example.org" } });
    fireEvent.submit(screen.getByLabelText("URL or search").closest("form")!);
    expect(browserRequest).toHaveBeenCalledWith({ action:"navigate",url:"https://example.org/" });
    for (const name of ["Back","Forward","Reload"]) fireEvent.click(screen.getByRole("button",{name}));
    fireEvent.click(screen.getByRole("tab",{name:"Example"}));
    expect(browserRequest).toHaveBeenCalledWith({ action:"activate",id:"one" });
    act(() => receive({ ...state, tabs: [{ ...state.tabs[0], loading:true }] }));
    fireEvent.click(screen.getByRole("button",{name:"Stop loading"}));
    expect(browserRequest).toHaveBeenCalledWith({ action:"stop" });
    view.unmount(); expect(stop).toHaveBeenCalled();
  });
  it("renders frames at DPR and forwards mouse, keys, wheel, paste and IME", async () => {
    vi.stubGlobal("Image", class { onload?: () => void; set src(_: string) { this.onload?.(); } });
    render(<BrowserPane onClose={vi.fn()} onTakeOver={vi.fn()} />);
    await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
    act(() => receive({ ...state, frame:"AAAA",frameTarget:"one",width:400,height:300,dpr:2 }));
    const canvas = screen.getByLabelText(/Browser page/) as HTMLCanvasElement;
    expect(canvas.width).toBe(800); expect(canvas.height).toBe(600); expect(drawImage).toHaveBeenCalled();
    fireEvent.mouseDown(canvas,{clientX:20,clientY:30,button:2,buttons:2,ctrlKey:true});
    expect(browserRequest).toHaveBeenCalledWith(expect.objectContaining({action:"mouse",type:"mousePressed",button:2,modifiers:2}));
    const keyboard = screen.getByLabelText("Browser keyboard input");
    fireEvent.keyDown(keyboard,{key:"a",code:"KeyA",keyCode:65});
    expect(browserRequest).toHaveBeenCalledWith(expect.objectContaining({action:"key",type:"keyDown",key:"a",text:"a"}));
    fireEvent.keyUp(keyboard,{key:"a",code:"KeyA"});
    fireEvent.wheel(canvas,{deltaY:120});
    expect(browserRequest).toHaveBeenCalledWith(expect.objectContaining({action:"mouse",type:"mouseWheel",deltaY:120}));
    fireEvent.paste(keyboard,{clipboardData:{getData:()=>"pasted"}});
    fireEvent.compositionStart(keyboard); fireEvent.compositionEnd(keyboard,{data:"日本語"});
    expect(browserRequest).toHaveBeenCalledWith({action:"text",text:"pasted"});
    expect(browserRequest).toHaveBeenCalledWith({action:"text",text:"日本語"});
  });
  it("shows agent activity and takes over", () => {
    const takeover = vi.fn();
    render(<BrowserPane agentActive onClose={vi.fn()} onTakeOver={takeover} />);
    expect(screen.getByText("Agent is using the browser")).toBeTruthy();
    fireEvent.click(screen.getByText("Take over")); expect(takeover).toHaveBeenCalledOnce();
  });
  it("scales backing pixels without changing CSS coordinates", () => {
    const canvas = document.createElement("canvas");
    const source = document.createElement("img");
    drawBrowserFrame(canvas,source,600,400,1.5);
    expect([canvas.width,canvas.height]).toEqual([900,600]);
    expect(drawImage).toHaveBeenCalledWith(source,0,0,900,600);
  });
});

it("uses the native slot, aligned toolbar sizes, and native takeover events", async () => {
  const takeover = vi.fn();
  render(<BrowserPane onClose={vi.fn()} onTakeOver={takeover} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive({ ...state, backend: "native", takeover: 1, tabs: [{ ...state.tabs[0], ready: true, canGoBack: false, canGoForward: false }] }));
  expect(screen.getByLabelText("Native browser page")).toBeTruthy();
  expect(document.querySelector("canvas")).toBeNull();
  expect(screen.getByRole("button", { name: "Back" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("progressbar", { name: "Page loading" })).toBeTruthy();
  expect(document.querySelector(".browser-toolbar .browser-address")).toBeTruthy();
  expect(takeover).toHaveBeenCalledOnce();
});

it("captures before hiding the native page and discards late captures after a tab change", async () => {
  vi.stubGlobal("Image", class { naturalWidth = 800; naturalHeight = 600; src = ""; decode = async () => {}; });
  let finish!: (image: string) => void;
  vi.mocked(browserRequest).mockImplementation(async params => params.action === "screenshot" ? new Promise<string>(resolve => { finish = resolve; }) : undefined);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, width: 500, height: 400 } as DOMRect);
  render(<BrowserPane onBrowserContext={vi.fn()} onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive({ ...state, backend: "native", tabs: [{ ...state.tabs[0], ready: true }] }));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Annotate page" })));
  expect(screen.queryByRole("group", { name: "Browser annotations" })).toBeNull();
  await act(async () => finish("AAAA"));
  expect(screen.getByRole("group", { name: "Browser annotations" })).toBeTruthy();
  expect(screen.getByRole("img", { name: /Page screenshot/ }).getAttribute("viewBox")).toBe("0 0 800 600");
  fireEvent.click(screen.getByRole("button", { name: "Close annotations" }));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Annotate page" })));
  act(() => receive({ ...state, active: "two", tabs: [{ ...state.tabs[0], id: "two" }] }));
  await act(async () => finish("AAAA"));
  expect(screen.queryByRole("group", { name: "Browser annotations" })).toBeNull();
});

it.each(["reload", "agent"])("closes stale annotations when %s begins", async reason => {
  vi.stubGlobal("Image", class { naturalWidth = 800; naturalHeight = 600; src = ""; decode = async () => {}; });
  vi.mocked(browserRequest).mockImplementation(async params => params.action === "screenshot" ? "AAAA" : undefined);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 500, height: 400 } as DOMRect);
  render(<BrowserPane onBrowserContext={vi.fn()} onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive(state));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Annotate page" })));
  expect(screen.getByRole("group", { name: "Browser annotations" })).toBeTruthy();
  act(() => receive(reason === "reload" ? { ...state, tabs: [{ ...state.tabs[0], loading: true }] } : { ...state, agent: true }));
  expect(screen.queryByRole("group", { name: "Browser annotations" })).toBeNull();
  expect(screen.getByRole("button", { name: "Annotate page" }).hasAttribute("disabled")).toBe(true);
});

it("does not offer capture tools when the backend reports no screenshot support", async () => {
  render(<BrowserPane onBrowserContext={vi.fn()} onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive({ ...state, backend: "native", capabilities: { screenshot: false, evaluate: false } }));
  expect(screen.queryByRole("button", { name: "Annotate page" })).toBeNull();
});

it.each(["tab", "URL"])("dismisses clear-site confirmation after its %s changes without clearing another site", async change => {
  render(<BrowserPane onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  const original: BrowserState = { ...state, backend: "native", tabs: [{ ...state.tabs[0], ready: true }] };
  act(() => receive(original));
  fireEvent.click(screen.getByRole("button", { name: "Actions for page" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Clear site data" }));
  const confirm = screen.getByRole("button", { name: "Clear data" });
  act(() => receive(change === "tab"
    ? { ...original, active: "two", tabs: [...original.tabs, { ...original.tabs[0], id: "two", url: "https://second.test/" }] }
    : { ...original, tabs: [{ ...original.tabs[0], url: "https://second.test/" }] }));
  expect(screen.queryByRole("button", { name: "Clear data" })).toBeNull();
  fireEvent.click(confirm);
  expect(browserRequest).not.toHaveBeenCalledWith(expect.objectContaining({ action: "clearSite" }));
  act(() => receive(original));
  expect(screen.queryByRole("button", { name: "Clear data" })).toBeNull();
});

it("binds confirmed site-data clearing to the original tab and page URL", async () => {
  render(<BrowserPane onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive({ ...state, backend: "native" }));
  fireEvent.click(screen.getByRole("button", { name: "Actions for page" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Clear site data" }));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Clear data" })));
  expect(browserRequest).toHaveBeenCalledWith({ action: "clearSite", confirmed: true, tabId: "one", expectedUrl: "https://example.test/" });
  expect(screen.queryByRole("button", { name: "Clear data" })).toBeNull();
});

it("treats a new native blank tab as idle and focuses its URL without stealing focus on existing-tab selection", async () => {
  render(<BrowserPane onBrowserContext={vi.fn()} onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  const original: BrowserState = { ...state, backend: "native", tabs: [{ ...state.tabs[0], ready: true }] };
  act(() => receive(original));
  const tabs = [...original.tabs, { id: "two", title: "New tab", url: "about:blank", loading: true, ready: false }];
  act(() => receive({ ...original, active: "two", tabs }));
  const address = screen.getByRole("textbox", { name: "URL or search" }) as HTMLInputElement;
  expect(document.activeElement).toBe(address);
  expect(address.value).toBe("");
  expect(screen.getByText("Enter a URL or search to open a page.")).toBeTruthy();
  expect(screen.queryByText("Loading page…")).toBeNull();
  expect(screen.queryByRole("button", { name: "Stop loading" })).toBeNull();
  expect(screen.getByRole("progressbar", { name: "Page loading" }).getAttribute("aria-valuenow")).toBe("100");
  expect(screen.getByRole("button", { name: "Annotate page" }).hasAttribute("disabled")).toBe(true);
  act(() => receive({ ...original, tabs }));
  const tab = screen.getByRole("tab", { name: "New tab" });
  tab.focus();
  act(() => receive({ ...original, active: "two", tabs }));
  expect(document.activeElement).toBe(tab);
  act(() => receive({ ...original, active: "two", tabs: [tabs[0], { ...tabs[1], url: "https://second.test/", loading: true }] }));
  expect(screen.getByText("Loading page…")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Stop loading" })).toBeTruthy();
});

it.each(["Escape", "Close"])("returns annotation focus to its trigger after explicit %s", async dismiss => {
  vi.stubGlobal("Image", class { naturalWidth = 800; naturalHeight = 600; src = ""; decode = async () => {}; });
  vi.mocked(browserRequest).mockImplementation(async params => params.action === "screenshot" ? "AAAA" : undefined);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 500, height: 400 } as DOMRect);
  render(<BrowserPane onBrowserContext={vi.fn()} onClose={vi.fn()} onTakeOver={vi.fn()} />);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive(state));
  const trigger = screen.getByRole("button", { name: "Annotate page" });
  await act(async () => fireEvent.click(trigger));
  const annotations = screen.getByRole("group", { name: "Browser annotations" });
  expect(document.activeElement).toBe(annotations);
  if (dismiss === "Escape") fireEvent.keyDown(annotations, { key: "Escape" });
  else fireEvent.click(screen.getByRole("button", { name: "Close annotations" }));
  expect(screen.queryByRole("group", { name: "Browser annotations" })).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

it("keeps composer focus after adding an annotated image to chat", async () => {
  vi.stubGlobal("Image", class { naturalWidth = 800; naturalHeight = 600; src = ""; decode = async () => {}; });
  vi.mocked(browserRequest).mockImplementation(async params => params.action === "screenshot" ? "AAAA" : undefined);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 500, height: 400 } as DOMRect);
  vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue({ drawImage, clearRect: vi.fn(), scale: vi.fn() } as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue("data:image/png;base64,RESULT");
  const onContext = vi.fn(() => screen.getByRole("textbox", { name: "Chat draft" }).focus());
  render(<><BrowserPane onBrowserContext={onContext} onClose={vi.fn()} onTakeOver={vi.fn()} /><textarea aria-label="Chat draft" /></>);
  await waitFor(() => expect(subscribeBrowser).toHaveBeenCalled());
  act(() => receive(state));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Annotate page" })));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Add to chat" })));
  expect(onContext).toHaveBeenCalledExactlyOnceWith({ image: "data:image/png;base64,RESULT", text: "Browser: https://example.test/" });
  expect(screen.queryByRole("group", { name: "Browser annotations" })).toBeNull();
  expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Chat draft" }));
});
