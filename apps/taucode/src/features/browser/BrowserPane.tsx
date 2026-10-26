import { NativeSlot } from "./NativeSlot";
import { RowMenu } from "@/features/chat/RowMenu";
import { ArrowLeft, ArrowRight, Download, LockKeyhole, Plus, RotateCw, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { browserRequest, subscribeBrowser, openBrowserDownloads, openBrowserUrl } from "@services/tauri";
import { isMacPlatform } from "@utils/platformPaths";
import { defaultBrowserSettings, drawBrowserFrame, modifiers, normalizeBrowserUrl } from "./browser";
import type { BrowserState } from "./browser";
import "./browser.css";

type Props = { embedded?: boolean; searchUrl?: string; agentActive?: boolean; onTakeOver: () => void | Promise<void>; onClose: () => void };
export function BrowserPane({ embedded = false, searchUrl = defaultBrowserSettings.searchUrl, agentActive = false, onTakeOver, onClose }: Props) {
  const [state, setState] = useState<BrowserState>({ tabs: [], active: "" });
  const native = state.backend === "native";
  const backend = useRef(state.backend); backend.current = state.backend;
  const [confirmClear, setConfirmClear] = useState(false);
  const [zoom, setZoom] = useState(1);
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
  const composing = useRef(false);
  const size = useRef({ width: 1000, height: 700, dpr: window.devicePixelRatio || 1 });
  const send = useCallback((params: Record<string, unknown>) => {
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
    const resize = () => { clearTimeout(timer); timer = setTimeout(() => { measure(); if (backend.current !== "native") send({ action: "resize", ...size.current }); }, 100); };
    const observer = new ResizeObserver(resize);
    if (surface.current) observer.observe(surface.current);
    window.addEventListener("resize", resize);
    return () => { disposed = true; clearTimeout(timer); observer.disconnect(); window.removeEventListener("resize", resize); stop?.(); };
  }, [send]);
  const active = state.tabs?.find(tab => tab.id === state.active);
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
      <button className="ghost" type="button" aria-label={active?.loading ? "Stop loading" : "Reload"} onClick={() => send({ action: active?.loading ? "stop" : "reload" })}>{active?.loading ? <X aria-hidden /> : <RotateCw aria-hidden />}</button>
      <div className="browser-address">{url.startsWith("https:") && <LockKeyhole aria-hidden />}<input ref={address} aria-label="URL or search" placeholder="Search or enter a URL" value={url} onChange={e => setUrl(e.target.value)} onFocus={e => e.target.select()} /></div>
      <button className="ghost" type="button" aria-label="Downloads" onClick={() => { void openBrowserDownloads().catch(e => setError(String(e))); }}><Download aria-hidden /></button>
      <RowMenu label="page" size="md" actions={[
        { label: "Open in default browser", onClick: () => { void openBrowserUrl(url).catch(e => setError(String(e))); } },
        { label: "Copy URL", onClick: () => { void navigator.clipboard.writeText(url).catch(e => setError(String(e))); } },
        ...(native ? [
          { label: "Clear site data", onClick: () => setConfirmClear(true) },
          { label: "Zoom +", onClick: () => { const value = Math.min(5, zoom + .1); setZoom(value); send({ action: "zoom", value }); } },
          { label: "Zoom −", onClick: () => { const value = Math.max(.25, zoom - .1); setZoom(value); send({ action: "zoom", value }); } },
        ] : []),
      ]}><span /></RowMenu>
    </form>
    <div className="browser-progress" role="progressbar" aria-label="Page loading" aria-valuenow={active?.loading ? Math.round((active.progress ?? .2) * 100) : 100}><span style={{ width: active?.loading ? `${Math.max(5, (active.progress ?? .2) * 100)}%` : "0%" }} /></div>
    {confirmClear && <div className="browser-confirm" role="alert">Clear sign-ins and storage for this site?<button onClick={() => { send({ action: "clearSite", confirmed: true }); setConfirmClear(false); }}>Clear data</button><button onClick={() => setConfirmClear(false)}>Cancel</button></div>}
    {state.externalRequest && <div className="browser-confirm" role="alert">Open {state.externalRequest} externally?<button onClick={() => { void openBrowserUrl(state.externalRequest!).catch(e => setError(String(e))); send({ action: "dismiss", key: "externalRequest" }); }}>Open</button><button onClick={() => send({ action: "dismiss", key: "externalRequest" })}>Cancel</button></div>}
    {state.downloadRequest && <div className="browser-confirm" role="alert">Save this download to Downloads?<button onClick={() => send({ action: "download", url: state.downloadRequest, confirmed: true })}>Save download</button><button onClick={() => send({ action: "dismiss", key: "downloadRequest" })}>Cancel</button></div>}
    <div ref={surface} className="browser-surface">
      {native ? <NativeSlot tabId={state.active} ready={active?.ready === true} snapshot={active?.snapshot} blocked={Boolean(pageError || confirmClear || state.externalRequest || state.downloadRequest)} onError={setError} /> : <>
      <canvas ref={canvas} tabIndex={0} aria-label="Browser page; Shift Escape returns to address bar" style={{ cursor: state.cursor || "default" }} onFocus={() => keyboard.current?.focus()} onMouseDown={e => mouse(e,"mousePressed")} onMouseUp={e => mouse(e,"mouseReleased")} onMouseMove={e => mouse(e,"mouseMoved")} onContextMenu={e => e.preventDefault()} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} />
      <textarea ref={keyboard} className="browser-keyboard" aria-label="Browser keyboard input" autoCapitalize="off" autoCorrect="off" spellCheck={false} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={e => { composing.current = false; send({ action: "text", text: e.data }); e.currentTarget.value = ""; }} onPaste={e => { e.preventDefault(); send({ action: "text", text: e.clipboardData.getData("text/plain") }); }} />
      </>}
      {pageError && <div className="browser-error" role="alert"><p>{pageError}</p><button onClick={() => send({ action: native ? "reload" : "attach", ...(!native ? size.current : {}) })}>Retry</button></div>}
      {!native && !state.frame && !pageError && <div className="browser-empty">{state.tabs?.length ? "Connecting to the page…" : "Open a tab to browse together."}</div>}
    </div>
  </section>;
}
