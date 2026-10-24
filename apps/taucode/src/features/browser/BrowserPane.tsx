import { ArrowLeft, ArrowRight, Plus, RotateCw, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { browserRequest, subscribeBrowser } from "@services/tauri";
import { isMacPlatform } from "@utils/platformPaths";
import { defaultBrowserSettings, drawBrowserFrame, modifiers, normalizeBrowserUrl } from "./browser";
import type { BrowserState } from "./browser";
import "./browser.css";

type Props = { searchUrl?: string; agentActive?: boolean; onTakeOver: () => void | Promise<void>; onClose: () => void };
export function BrowserPane({ searchUrl = defaultBrowserSettings.searchUrl, agentActive = false, onTakeOver, onClose }: Props) {
  const [state, setState] = useState<BrowserState>({ tabs: [], active: "" });
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
    const resize = () => { clearTimeout(timer); timer = setTimeout(() => { measure(); send({ action: "resize", ...size.current }); }, 100); };
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
  }, [send]);
  const key = (e: KeyboardEvent, type: "keyDown" | "keyUp") => {
    e.stopPropagation();
    if (e.key === "Escape" && e.shiftKey) { e.preventDefault(); address.current?.focus(); return; }
    if (composing.current || e.nativeEvent.isComposing) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "v") return; // Native paste supplies clipboard text.
    e.preventDefault();
    send({ action: "key", type, key: e.key, code: e.code, keyCode: e.keyCode, text: e.key.length === 1 ? e.key : e.key === "Enter" ? "\r" : "", modifiers: modifiers(e), mac: isMacPlatform() });
  };
  return <section className="browser-pane ds-panel" aria-label="Browser">
    <header className="ds-panel-header"><strong>Browser</strong><button className="ghost" onClick={onClose} aria-label="Close browser pane"><X aria-hidden /></button></header>
    {(agentActive || state.agent) && <div className="browser-agent" role="status"><span>Agent is using the browser</span><button className="secondary" onClick={() => { void Promise.resolve(onTakeOver()).catch(e => setError(String(e))); }}>Take over</button></div>}
    <div className="browser-tabs" role="tablist" aria-label="Browser tabs">
      {state.tabs?.map(tab => <div className="browser-tab" key={tab.id}>
        <button role="tab" aria-selected={tab.id === state.active} onClick={() => send({ action: "activate", id: tab.id })}>{tab.favicon && /^(https?:|data:image\/)/.test(tab.favicon) && <img src={tab.favicon} alt="" referrerPolicy="no-referrer" />}<span>{tab.title || "New tab"}</span></button>
        <button aria-label={`Close ${tab.title || "tab"}`} onClick={() => send({ action: "close", id: tab.id })}><X aria-hidden /></button>
      </div>)}
      <button className="ghost" aria-label="New browser tab" onClick={() => send({ action: "new" })}><Plus aria-hidden /></button>
    </div>
    <form className="browser-toolbar" onSubmit={e => { e.preventDefault(); try { send({ action: "navigate", url: normalizeBrowserUrl(url, searchUrl) }); } catch (err) { setError(String(err)); } }}>
      <button className="ghost" type="button" aria-label="Back" onClick={() => send({ action: "back" })}><ArrowLeft aria-hidden /></button>
      <button className="ghost" type="button" aria-label="Forward" onClick={() => send({ action: "forward" })}><ArrowRight aria-hidden /></button>
      <button className="ghost" type="button" aria-label={active?.loading ? "Stop loading" : "Reload"} onClick={() => send({ action: active?.loading ? "stop" : "reload" })}>{active?.loading ? <X aria-hidden /> : <RotateCw aria-hidden />}</button>
      <input ref={address} aria-label="URL or search" placeholder="Search or enter a URL" value={url} onChange={e => setUrl(e.target.value)} onFocus={e => e.target.select()} />
    </form>
    {(error || state.error) && <div className="browser-error" role="alert">{error || state.error}<button onClick={() => send({ action: "attach", ...size.current })}>Retry</button></div>}
    <div ref={surface} className="browser-surface">
      <canvas ref={canvas} tabIndex={0} aria-label="Browser page; Shift Escape returns to address bar" style={{ cursor: state.cursor || "default" }} onFocus={() => keyboard.current?.focus()} onMouseDown={e => mouse(e,"mousePressed")} onMouseUp={e => mouse(e,"mouseReleased")} onMouseMove={e => mouse(e,"mouseMoved")} onContextMenu={e => e.preventDefault()} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} />
      <textarea ref={keyboard} className="browser-keyboard" aria-label="Browser keyboard input" autoCapitalize="off" autoCorrect="off" spellCheck={false} onKeyDown={e => key(e,"keyDown")} onKeyUp={e => key(e,"keyUp")} onCompositionStart={() => { composing.current = true; }} onCompositionEnd={e => { composing.current = false; send({ action: "text", text: e.data }); e.currentTarget.value = ""; }} onPaste={e => { e.preventDefault(); send({ action: "text", text: e.clipboardData.getData("text/plain") }); }} />
      {!state.frame && !error && !state.error && <div className="browser-empty">{state.tabs?.length ? "Connecting to the page…" : "Open a tab to browse together."}</div>}
    </div>
  </section>;
}
