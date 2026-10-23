// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserPane } from "./BrowserPane";
import { drawBrowserFrame, normalizeBrowserUrl } from "./browser";
import { browserRequest, subscribeBrowser } from "@services/tauri";
import type { BrowserState } from "./browser";

vi.mock("@services/tauri", () => ({ browserRequest: vi.fn(), subscribeBrowser: vi.fn() }));
const drawImage = vi.fn();
let receive: (s: BrowserState) => void;
const stop = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(browserRequest).mockResolvedValue(undefined);
  vi.mocked(subscribeBrowser).mockImplementation(async fn => { receive = fn; return stop; });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage, clearRect: vi.fn() } as unknown as CanvasRenderingContext2D);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
const state: BrowserState = { tabs: [{ id: "one", title: "Example", url: "https://example.test/", loading: false }], active: "one" };
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
