import { isWindowsPlatform } from "@utils/platformPaths";
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type MouseEvent } from "react";

const widthKey = "codexmonitor.sidebarWidth";
const collapsedKey = "codexmonitor.sidebarCollapsed";
const changed = "taucode:sidebar";
const clamp = (width: number) => Math.min(420, Math.max(220, width));
function readWidth() {
  const raw = localStorage.getItem(widthKey);
  const width = raw ? Number(raw) : 280;
  return Number.isFinite(width) ? clamp(width) : 280;
}
function subscribe(listener: () => void) {
  window.addEventListener(changed, listener);
  window.addEventListener("storage", listener);
  return () => { window.removeEventListener(changed, listener); window.removeEventListener("storage", listener); };
}
function write(key: string, value: string) {
  localStorage.setItem(key, value);
  window.dispatchEvent(new Event(changed));
}
export function useSidebarState() {
  const sidebarWidth = useSyncExternalStore(subscribe, readWidth);
  const sidebarCollapsed = useSyncExternalStore(subscribe, () => localStorage.getItem(collapsedKey) === "true");
  const [isResizing, setIsResizing] = useState(false);
  const cleanup = useRef<(() => void) | null>(null);
  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--sidebar-top-padding", isWindowsPlatform() ? "10px" : "36px");
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
    localStorage.setItem(widthKey, String(sidebarWidth));
  }, [sidebarWidth]);
  useEffect(() => () => cleanup.current?.(), []);
  const setSidebarWidth = (width: number) => write(widthKey, String(clamp(width)));
  const onSidebarResizeStart = (event: MouseEvent) => {
    event.preventDefault();
    cleanup.current?.();
    const start = event.clientX;
    const move = (e: globalThis.MouseEvent) => setSidebarWidth(sidebarWidth + e.clientX - start);
    const stop = () => {
      window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", stop);
      document.body.style.cursor = ""; document.body.style.userSelect = "";
      setIsResizing(false); cleanup.current = null;
    };
    cleanup.current = stop;
    window.addEventListener("mousemove", move); window.addEventListener("mouseup", stop);
    document.body.style.cursor = "col-resize"; document.body.style.userSelect = "none";
    setIsResizing(true);
  };
  return { sidebarWidth, sidebarCollapsed, setSidebarWidth, setSidebarCollapsed: (value: boolean) => write(collapsedKey, String(value)), onSidebarResizeStart, isResizing };
}
