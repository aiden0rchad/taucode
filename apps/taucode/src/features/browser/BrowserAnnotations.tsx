import { useEffect, useRef, useState } from "react";
import { MousePointer2, Pencil, Square, Type, Undo2, Redo2, X } from "lucide-react";
import { browserRequest } from "@services/tauri";
import type { BrowserContext } from "./BrowserPane";

type Point = { x: number; y: number };
type Mark = { tool: "pen" | "rectangle" | "text" | "select"; points: Point[]; color: string; width: number; text?: string };
export type BrowserPageGeometry = { width: number; height: number; scrollX: number; scrollY: number; timeOrigin: number };
export type BrowserCapture = { image: string; width: number; height: number; url: string; tab: string; page?: BrowserPageGeometry };

export function annotationPoint(x: number, y: number, rect: Pick<DOMRect, "left" | "top" | "width" | "height">, size: Pick<BrowserCapture, "width" | "height">): Point {
  const scale = Math.max(.001, Math.min(rect.width / size.width, rect.height / size.height));
  return { x: Math.max(0, Math.min(size.width, (x - rect.left - (rect.width - size.width * scale) / 2) / scale)), y: Math.max(0, Math.min(size.height, (y - rect.top - (rect.height - size.height * scale) / 2) / scale)) };
}

// Read only visible element metadata. Never include field values or page HTML.
export function selectionScript(point: Point, capture: BrowserCapture): string {
  if (!capture.page) throw new Error("Element selection is unavailable for this capture");
  return `() => {
    if (location.href !== ${JSON.stringify(capture.url)}) throw new Error("Page changed. Close annotations and capture it again.");
    const previous = ${JSON.stringify(capture.page)};
    if (performance.timeOrigin !== previous.timeOrigin) throw new Error("Page reloaded. Close annotations and capture it again.");
    if (Math.abs(innerWidth - previous.width) > 1 || Math.abs(innerHeight - previous.height) > 1) throw new Error("Page size changed. Close annotations and capture it again.");
    if (Math.abs(scrollX - previous.scrollX) > 1 || Math.abs(scrollY - previous.scrollY) > 1) throw new Error("Page moved. Close annotations and capture it again.");
    const element = document.elementFromPoint(${point.x / capture.width} * innerWidth, ${point.y / capture.height} * innerHeight);
    if (!element) throw new Error("No element at this point");
    const rect = element.getBoundingClientRect();
    const tag = element.tagName.toLowerCase();
    const text = /^(input|textarea)$/.test(tag) || element.isContentEditable || element.querySelector("input,textarea,[contenteditable]") ? "" : (element.innerText || "").trim().slice(0, 240);
    return { x: rect.x / innerWidth, y: rect.y / innerHeight, width: rect.width / innerWidth, height: rect.height / innerHeight,
      label: [tag, element.id ? "#" + element.id : "", element.getAttribute("aria-label") || text].filter(Boolean).join(" ").slice(0, 320) };
  }`;
}

export function BrowserAnnotations({ capture, onAdd, onClose }: { capture: BrowserCapture; onAdd: (context: BrowserContext) => void; onClose: (reason: "dismiss" | "add") => void }) {
  const [tool, setTool] = useState<Mark["tool"]>("pen");
  const [color, setColor] = useState("#ef4444");
  const [width, setWidth] = useState(3);
  const [marks, setMarks] = useState<Mark[]>([]);
  const [redo, setRedo] = useState<Mark[]>([]);
  const [draft, setDraft] = useState<Mark | null>(null);
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const gesture = useRef<Mark | null>(null);
  const live = useRef(true);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => { live.current = true; root.current?.focus(); return () => { live.current = false; }; }, []);
  const add = (mark: Mark) => { setMarks(old => [...old, mark]); setRedo([]); setError(""); };
  const close = (reason: "dismiss" | "add" = "dismiss") => { live.current = false; onClose(reason); };
  const select = async (point: Point) => {
    setBusy(true); setError("");
    try {
      const result = await browserRequest<{ x: number; y: number; width: number; height: number; label: string }>({ action: "evaluate", tabId: capture.tab, expectedUrl: capture.url, function: selectionScript(point, capture) });
      if (!live.current) return;
      if (!result || ![result.x, result.y, result.width, result.height].every(Number.isFinite) || typeof result.label !== "string") throw new Error("The page returned an invalid element selection");
      add({ tool: "select", color, width, text: result.label, points: [{ x: result.x * capture.width, y: result.y * capture.height }, { x: (result.x + result.width) * capture.width, y: (result.y + result.height) * capture.height }] });
    } catch (e) { if (live.current) setError(String(e)); }
    finally { if (live.current) setBusy(false); }
  };
  const exportImage = async () => {
    setBusy(true); setError("");
    try {
      const image = new Image();
      image.src = capture.image;
      await image.decode();
      if (!live.current) return;
      const canvas = document.createElement("canvas");
      canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Screenshot export is unavailable");
      context.drawImage(image, 0, 0);
      context.scale(canvas.width / capture.width, canvas.height / capture.height);
      context.lineCap = "round"; context.lineJoin = "round";
      for (const mark of marks) {
        context.strokeStyle = mark.color; context.fillStyle = mark.color; context.lineWidth = mark.width;
        const [start, end = start] = mark.points;
        if (mark.tool === "text") { context.font = "600 18px system-ui"; context.fillText(mark.text ?? "", start.x, start.y); }
        else if (mark.tool === "pen") { context.beginPath(); context.moveTo(start.x, start.y); for (const point of mark.points.slice(1)) context.lineTo(point.x, point.y); context.stroke(); }
        else context.strokeRect(start.x, start.y, end.x - start.x, end.y - start.y);
      }
      onAdd({ image: canvas.toDataURL("image/png"), text: [`Browser: ${capture.url}`, ...marks.filter(mark => mark.tool === "select").map(mark => `Selected element: ${mark.text}`)].join("\n") });
      close("add");
    } catch (e) { if (live.current) { setError(String(e)); setBusy(false); } }
  };
  return <div ref={root} tabIndex={-1} className="browser-annotations" role="group" aria-label="Browser annotations" onKeyDown={e => {
    if (e.key === "Escape") { e.stopPropagation(); close(); }
  }}>
    <svg className="browser-annotation-image" role="img" aria-label="Page screenshot; choose a tool and mark the page" viewBox={`0 0 ${capture.width} ${capture.height}`} onPointerDown={e => {
      if (busy || e.button !== 0) return;
      const point = annotationPoint(e.clientX, e.clientY, e.currentTarget.getBoundingClientRect(), capture);
      if (tool === "select") { void select(point); return; }
      if (tool === "text") { if (label.trim()) add({ tool, color, width, points: [point], text: label.trim() }); return; }
      e.currentTarget.setPointerCapture(e.pointerId);
      gesture.current = { tool, color, width, points: [point, point] }; setDraft(gesture.current);
    }} onPointerMove={e => {
      const mark = gesture.current;
      if (!mark) return;
      const point = annotationPoint(e.clientX, e.clientY, e.currentTarget.getBoundingClientRect(), capture);
      gesture.current = { ...mark, points: mark.tool === "pen" ? [...mark.points, point] : [mark.points[0], point] };
      setDraft(gesture.current);
    }} onPointerUp={e => {
      if (gesture.current) { add(gesture.current); gesture.current = null; setDraft(null); }
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    }} onPointerCancel={() => { gesture.current = null; setDraft(null); }}>
      <image href={capture.image} width={capture.width} height={capture.height} />
      {[...marks, ...(draft ? [draft] : [])].map((mark, index) => {
        const [a, b = a] = mark.points;
        return mark.tool === "text" ? <text key={index} x={a.x} y={a.y} fill={mark.color} fontSize="18" fontWeight="600">{mark.text}</text> : mark.tool === "pen" ? <polyline key={index} points={mark.points.map(p => `${p.x},${p.y}`).join(" ")} fill="none" stroke={mark.color} strokeWidth={mark.width} strokeLinecap="round" strokeLinejoin="round" /> : <rect key={index} x={Math.min(a.x, b.x)} y={Math.min(a.y, b.y)} width={Math.abs(b.x - a.x)} height={Math.abs(b.y - a.y)} fill="none" stroke={mark.color} strokeWidth={mark.width}><title>{mark.text}</title></rect>;
      })}
    </svg>
    <div className="browser-annotation-controls">
      {error && <div className="browser-annotation-error" role="alert">{error}</div>}
      {tool === "text" && <input className="browser-annotation-label" aria-label="Annotation text" placeholder="Text to place on the page" value={label} maxLength={240} onChange={e => setLabel(e.target.value)} />}
      <div className="browser-annotation-toolbar" role="toolbar" aria-label="Annotation tools">
        {([["select", "Select element", MousePointer2], ["pen", "Pen", Pencil], ["rectangle", "Rectangle", Square], ["text", "Text", Type]] as const).map(([value, title, Icon]) => <button key={value} className="ghost" title={value === "select" && !capture.page ? "Element selection is unavailable for this page" : title} aria-label={title} aria-pressed={tool === value} disabled={busy || (value === "select" && !capture.page)} onClick={() => setTool(value)}><Icon aria-hidden /></button>)}
        <input type="color" aria-label="Ink color" title="Ink color" value={color} onChange={e => setColor(e.target.value)} />
        <select aria-label="Line width" value={width} onChange={e => setWidth(Number(e.target.value))}><option value={2}>Thin</option><option value={3}>Medium</option><option value={6}>Thick</option></select>
        <button className="ghost" aria-label="Undo annotation" title="Undo" disabled={busy || !marks.length} onClick={() => { setRedo(old => [...old, marks[marks.length - 1]]); setMarks(old => old.slice(0, -1)); }}><Undo2 aria-hidden /></button>
        <button className="ghost" aria-label="Redo annotation" title="Redo" disabled={busy || !redo.length} onClick={() => { setMarks(old => [...old, redo[redo.length - 1]]); setRedo(old => old.slice(0, -1)); }}><Redo2 aria-hidden /></button>
        <button className="secondary browser-annotation-add" disabled={busy} onClick={() => void exportImage()}>{busy ? "Working…" : "Add to chat"}</button>
        <button className="ghost" aria-label="Close annotations" title="Close annotations" onClick={() => close()}><X aria-hidden /></button>
      </div>
      <span className="browser-annotation-hint">{tool === "select" ? "Click an element to include its description." : tool === "text" ? "Enter text, then click where it belongs." : "Mark the captured page, then add it to your draft."}</span>
    </div>
  </div>;
}
