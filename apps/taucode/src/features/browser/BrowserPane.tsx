import { NativeSlot } from "./NativeSlot";
import { RowMenu } from "@/features/chat/RowMenu";
import { ArrowLeft, ArrowRight, Download, LockKeyhole, Pencil, Plus, RotateCw, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { browserRequest, subscribeBrowser, openBrowserDownloads, openBrowserUrl, exportBrowserScreenshot } from "@services/tauri";
import { isMacPlatform } from "@utils/platformPaths";
import { defaultBrowserSettings, drawBrowserFrame, modifiers, normalizeBrowserUrl } from "./browser";
import type { BrowserState } from "./browser";
import { BrowserAnnotations } from "./BrowserAnnotations";
import type { BrowserCapture, BrowserPageGeometry } from "./BrowserAnnotations";
import "./browser.css";

export type BrowserContext = { text: string; image?: string };
type Props = { embedded?: boolean; searchUrl?: string; agentActive?: boolean; onBrowserContext?: (context: BrowserContext) => void; onTakeOver: () => void | Promise<void>; onClose: () => void };
export function BrowserPane({ embedded = false, searchUrl = defaultBrowserSettings.searchUrl, agentActive = false, onBrowserContext, onTakeOver, onClose }: Props) {
  const [state, setState] = useState<BrowserState>({ tabs: [], active: "" });
  const native = state.backend === "native";
  const backend = useRef(state.backend); backend.current = state.backend;
  const [confirmClear, setConfirmClear] = useState<{ tabId: string; expectedUrl: string } | null>(null);
  const [zoom, setZoom] = useState(1);
  const [viewport, setViewport] = useState<"responsive" | "phone">("responsive");
  const [capture, setCapture] = useState<BrowserCapture | null>(null);
  const [capturing, setCapturing] = useState(false);
  const captureId = useRef(0);
  const takeover = useRef(0);
  useEffect(() => {
    if (state.takeover && state.takeover > takeover.current) { takeover.current = state.takeover; void onTakeOver(); }
  }, [state.takeover, onTakeOver]);
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  const canvas = useRef<HTMLCanvasElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const keyboard = useRef<HTMLTextAreaElement>(null);
  const address = useRef<HTMLInputElement>(null);
  const annotateButton = useRef<HTMLButtonElement>(null);
  const seenTabs = useRef(new Set<string>());
  const composing = useRef(false);
  const size = useRef({ width: 1000, height: 700, dpr: window.devicePixelRatio || 1 });
  const send = useCallback((params: Record<string, unknown>) => {
    if (["navigate", "back", "forward", "reload", "activate", "close", "zoom"].includes(String(params.action))) { captureId.current++; setCapture(null); setCapturing(false); }
    void browserRequest(params).then(() => setError("")).catch(e => setError(String(e)));
  }, []);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const measure = () => {
      const rect = surface.current?.getBoundingClientRect();
      size.current = { width: Math.max(1, Math.round(rect?.width || 1000)), height: Math.max(1, Math.round(rect?.height || 700)), dpr: Math.min(4, Math.max(.5, window.devicePixelRatio || 1)) };
    };
    measure();
    void subscribeBrowser(value => { if (!disposed) setState(value); }).then(unsubscribe => {
      if (disposed) { unsubscribe(); return; }
      stop = unsubscribe; send({ action: "attach", ...size.current });
    }).catch(e => { if (!disposed) setError(String(e)); });
    const resize = () => { clearTimeout(timer); timer = setTimeout(() => {
      const previous = size.current; measure();
      if (previous.width !== size.current.width || previous.height !== size.current.height) { captureId.current++; setCapture(null); setCapturing(false); }
      if (backend.current !== "native") send({ action: "resize", ...size.current });
    }, 100); };
    const observer = new ResizeObserver(resize);
    if (surface.current) observer.observe(surface.current);
    window.addEventListener("resize", resize);
    return () => { disposed = true; clearTimeout(timer); observer.disconnect(); window.removeEventListener("resize", resize); stop?.(); };
  }, [send]);
  const active = state.tabs?.find(tab => tab.id === state.active);
  const blank = active?.url === "about:blank";
  const loading = Boolean(active?.loading && !blank);
  useEffect(() => {
    const newBlank = blank && !seenTabs.current.has(state.active);
    seenTabs.current = new Set(state.tabs.map(tab => tab.id));
    if (newBlank) address.current?.focus();
  }, [blank, state.active, state.tabs]);
  useEffect(() => { setConfirmClear(null); }, [state.active, active?.url]);
  const clearSiteTarget = confirmClear?.tabId === state.active && confirmClear.expectedUrl === active?.url ? confirmClear : null;
  useEffect(() => { captureId.current++; setCapture(null); setCapturing(false); }, [state.active, active?.url, active?.loading, viewport, agentActive, state.agent]);
  useEffect(() => () => { captureId.current++; }, []);
  const annotate = async () => {
    if (capture) { setCapture(null); annotateButton.current?.focus(); return; }
    const id = ++captureId.current;
    const rect = surface.current?.getBoundingClientRect();
    if (!active || !rect?.width || !rect.height) return;
    setCapturing(true); setError("");
    try {
      let page: BrowserPageGeometry | undefined;
      if (state.capabilities?.evaluate) {
        // Page metadata can be unavailable even when capture is supported.
        try {
          const result = await browserRequest<BrowserPageGeometry>({ action: "evaluate", tabId: state.active, expectedUrl: active.url, function: "() => ({ width: innerWidth, height: innerHeight, scrollX, scrollY, timeOrigin: performance.timeOrigin })" });
          if (result && [result.width, result.height, result.scrollX, result.scrollY, result.timeOrigin].every(Number.isFinite) && result.width > 0 && result.height > 0) page = result;
        } catch { /* Keep selection disabled for this capture. */ }
      }
      if (id !== captureId.current) return;
      const image = await browserRequest<string>({ action: "screenshot", tabId: state.active, expectedUrl: active.url });
      if (id !== captureId.current) return;
      if (typeof image !== "string" || !/^[A-Za-z0-9+/=\r\n]+$/.test(image)) throw new Error("The browser did not return a screenshot");
      const decoded = new Image(); decoded.src = `data:image/png;base64,${image}`; await decoded.decode();
      if (id !== captureId.current) return;
      if (!decoded.naturalWidth || !decoded.naturalHeight) throw new Error("The screenshot is empty");
      setCapture({ image: decoded.src, width: decoded.naturalWidth, height: decoded.naturalHeight, url: active.url, tab: state.active, page });
    } catch (e) { if (id === captureId.current) setError(String(e)); }
    finally { if (id === captureId.current) setCapturing(false); }
  };
  useEffect(() => { setUrl(active?.url === "about:blank" ? "" : active?.url ?? ""); }, [active?.url, state.active]);
  useEffect(() => {
    const target = canvas.current;
    if (!target) return;
    if (!state.frame || state.frameTarget !== state.active) { target.getContext("2d")?.clearRect(0,0,target.width,target.height); return; }
    let disposed = false;
    const image = new Image();
    image.onload = () => { if (!disposed) drawBrowserFrame(target, image, state.width ?? size.current.width, state.height ?? size.current.height, state.dpr ?? size.current.dpr); };
    image.src = `data:image/jpeg;base64,${state.frame}`;
    return () => { disposed = true; };
  }, [state.frame, state.frameTarget, state.active, state.width, state.height, state.dpr]);
  const mouse = (e: MouseEvent<HTMLCanvasElement>, type: string) => {
    e.preventDefault();
    if (type === "mousePressed") keyboard.current?.focus();
    const rect = e.currentTarget.getBoundingClientRect();
    send({ action: "mouse", type, x: (e.clientX - rect.left) * (state.width ?? size.current.width) / (rect.width || 1), y: (e.clientY - rect.top) * (state.height ?? size.current.height) / (rect.height || 1), button: type === "mouseMoved" && !e.buttons ? -1 : e.button, buttons: e.buttons, clickCount: e.detail, modifiers: modifiers(e) });
  };
  useEffect(() => {
    const target = canvas.current;
    if (!target) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = target.getBoundingClientRect();
      const factor = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? size.current.height : 1;
      send({ action: "mouse", type: "mouseWheel", x: (e.clientX - rect.left) * size.current.width / (rect.width || 1), y: (e.clientY - rect.top) * size.current.height / (rect.height || 1), button: -1, deltaX: e.deltaX * factor, deltaY: e.deltaY * factor, modifiers: modifiers(e) });
    };
    target.addEventListener("wheel", wheel, { passive: false });
    return () => target.removeEventListener("wheel", wheel);
  }, [send, native]);
  const key = (e: KeyboardEvent, type: "keyDown" | "keyUp") => {
    e.stopPropagation();
    if (e.key === "Escape" && e.shiftKey) { e.preventDefault(); address.current?.focus(); return; }
    if (composing.current || e.nativeEvent.isComposing) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "v") return; // Native paste supplies clipboard text.
    e.preventDefault();
    send({ action: "key", type, key: e.key, code: e.code, keyCode: e.keyCode, text: e.key.length === 1 ? e.key : e.key === "Enter" ? "\r" : "", modifiers: modifiers(e), mac: isMacPlatform() });
  };
  const pageError = error || state.error || active?.error;
  return <section className={`browser-pane ds-panel${agentActive || state.agent ? " browser-agent-active" : ""}`} aria-label="Browser">
    {!embedded && <header className="ds-panel-header"><strong>Browser</strong><button className="ghost" onClick={onClose} aria-label="Close browser pane"><X aria-hidden /></button></header>}
    {(agentActive || state.agent) && <div className="browser-agent" role="status"><span>Agent is using the browser</span><button className="secondary" onClick={() => { void Promise.resolve(onTakeOver()).catch(e => setError(String(e))); }}>Take over</button></div>}
    {!embedded && <div className="browser-tabs" role="tablist" aria-label="Browser tabs">
      {state.tabs?.map(tab => <div className={`browser-tab${tab.id === state.active ? " active" : ""}`} key={tab.id}>
        <button role="tab" aria-selected={tab.id === state.active} onClick={() => send({ action: "activate", id: tab.id })}>{tab.favicon && /^(https?:|data:image\/)/.test(tab.favicon) && <img src={tab.favicon} alt="" referrerPolicy="no-referrer" />}<span>{tab.title || "New tab"}</span></button>
        <button aria-label={`Close ${tab.title || "tab"}`} onClick={() => send({ action: "close", id: tab.id })}><X aria-hidden /></button>
      </div>)}
      <button className="ghost" aria-label="New browser tab" onClick={() => send({ action: "new" })}><Plus aria-hidden /></button>
    </div>}
    <form className="browser-toolbar" onSubmit={e => { e.preventDefault(); try { send({ action: "navigate", url: normalizeBrowserUrl(url, searchUrl) }); } catch (err) { setError(String(err)); } }}>
      <button className="ghost" type="button" aria-label="Back" disabled={native && active?.canGoBack === false} onClick={() => send({ action: "back" })}><ArrowLeft aria-hidden /></button>
      <button className="ghost" type="button" aria-label="Forward" disabled={native && active?.canGoForward === false} onClick={() => send({ action: "forward" })}><ArrowRight aria-hidden /></button>
      <button className="ghost" type="button" aria-label={loading ? "Stop loading" : "Reload"} onClick={() => send({ action: loading ? "stop" : "reload" })}>{loading ? <X aria-hidden /> : <RotateCw aria-hidden />}</button>
      <div className="browser-address">{url.startsWith("https:") && <LockKeyhole aria-hidden />}<input ref={address} aria-label="URL or search" placeholder="Search or enter a URL" value={url} onChange={e => setUrl(e.target.value)} onFocus={e => e.target.select()} /></div>
      <button className="ghost" type="button" aria-label="Downloads" onClick={() => { void openBrowserDownloads().catch(e => setError(String(e))); }}><Download aria-hidden /></button>
      {onBrowserContext && state.capabilities?.screenshot && <button ref={annotateButton} className="ghost" type="button" aria-label="Annotate page" title="Select or annotate the page" aria-pressed={Boolean(capture)} disabled={!active || blank || active.loading || capturing || agentActive || state.agent} onClick={() => void annotate()}><Pencil aria-hidden /></button>}
      <RowMenu label="page" size="md" actions={[
        { label: "Open in default browser", onClick: () => { void openBrowserUrl(url).catch(e => setError(String(e))); } },
        { label: "Copy URL", onClick: () => { void navigator.clipboard.writeText(url).catch(e => setError(String(e))); } },
        ...(state.capabilities?.screenshot ? [{ label: "Save screenshot", onClick: () => { void exportBrowserScreenshot(state.active, active?.url).catch(e => setError(String(e))); } }] : []),
        { label: viewport === "phone" ? "Viewport: Responsive" : "Viewport: Phone (390px)", onClick: () => setViewport(old => old === "phone" ? "responsive" : "phone") },
        ...(native ? [
          { label: "Clear site data", disabled: !active, onClick: () => { if (active) setConfirmClear({ tabId: state.active, expectedUrl: active.url }); } },
          { label: "Zoom +", onClick: () => { const value = Math.min(5, zoom + .1); setZoom(value); send({ action: "zoom", value }); } },
          { label: "Zoom −", onClick: () => { const value = Math.max(.25, zoom - .1); setZoom(value); send({ action: "zoom", value }); } },
        ] : []),
      ]}><span /></RowMenu>
    </form>
    <div className="browser-progress" role="progressbar" aria-label="Page loading" aria-valuenow={loading ? Math.round((active?.progress ?? .2) * 100) : 100}><span style={{ width: loading ? `${Math.max(5, (active?.progress ?? .2) * 100)}%` : "0%" }} /></div>
    {clearSiteTarget && <div className="browser-confirm" role="alert">Clear sign-ins and storage for this site?<button onClick={() => { if (clearSiteTarget.tabId === state.active && clearSiteTarget.expectedUrl === active?.url) send({ action: "clearSite", confirmed: true, ...clearSiteTarget }); setConfirmClear(null); }}>Clear data</button><button onClick={() => setConfirmClear(null)}>Cancel</button></div>}
    {state.externalRequest && <div className="browser-confirm" role="alert">Open {state.externalRequest} externally?<button onClick={() => { void openBrowserUrl(state.externalRequest!).catch(e => setError(String(e))); send({ action: "dismiss", key: "externalRequest" }); }}>Open</button><button onClick={() => send({ action: "dismiss", key: "externalRequest" })}>Cancel</button></div>}
    {state.downloadRequest && <div className="browser-confirm" role="alert">Save this download to Downloads?<button onClick={() => send({ action: "download", url: state.downloadRequest, confirmed: true })}>Save download</button><button onClick={() => send({ action: "dismiss", key: "downloadRequest" })}>Cancel</button></div>}
    <div ref={surface} className="browser-surface" data-viewport={viewport}>
      {native ? !blank && <NativeSlot tabId={state.active} ready={active?.ready === true} snapshot={active?.snapshot} blocked={Boolean(capture || pageError || clearSiteTarget || state.externalRequest || state.downloadRequest)} onError={setError} /> : <>
      <canvas ref={canvas} tabIndex={0} aria-label="Browser page; Shift Escape returns to address bar" style={{ cursor: state.cursor || "default" }} onFocus={() => keyboard.current?.focus()} onMouseDown={e => mouse(e,"mousePressed")} onMouseUp={e => mouse(e,"mouseReleased")} onMouseMove={e => mouse(e,"mouseMoved")} onContextMenu={e => e.preventDefault()} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} />
      <textarea ref={keyboard} className="browser-keyboard" aria-label="Browser keyboard input" autoCapitalize="off" autoCorrect="off" spellCheck={false} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={e => { composing.current = false; send({ action: "text", text: e.data }); e.currentTarget.value = ""; }} onPaste={e => { e.preventDefault(); send({ action: "text", text: e.clipboardData.getData("text/plain") }); }} />
      </>}
      {blank && <div className="browser-empty">Enter a URL or search to open a page.</div>}
      {capture && onBrowserContext && <BrowserAnnotations key={`${capture.tab}:${capture.url}`} capture={capture} onAdd={onBrowserContext} onClose={reason => { setCapture(null); if (reason === "dismiss") annotateButton.current?.focus(); }} />}
      {pageError && <div className="browser-error" role="alert"><p>{pageError}</p><button onClick={() => send({ action: native ? "reload" : "attach", ...(!native ? size.current : {}) })}>Retry</button></div>}
      {!blank && !native && !state.frame && !pageError && <div className="browser-empty">{state.tabs?.length ? "Connecting to the page…" : "Open a tab to browse together."}</div>}
    </div>
  </section>;
}
