import { isWindowsPlatform } from "@utils/platformPaths";
import { useLayoutEffect, useSyncExternalStore, type PointerEvent } from "react";

import { workspaceLayout } from "@/features/panels/panelState";
import { usePanelResize } from "./usePanelResize";

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
  const { isResizing, startResize } = usePanelResize();
  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--sidebar-top-padding", isWindowsPlatform() ? "10px" : "36px");
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
  }, [sidebarWidth]);
  const setSidebarWidth = (width: number) => write(widthKey, String(clamp(width)));
  const onSidebarResizeStart = (event: PointerEvent) => {
    const start = event.clientX;
    const sidebar = event.currentTarget.parentElement?.querySelector(".sidebar");
    const shell = event.currentTarget.closest<HTMLElement>(".workspace-shell");
    const shellWidth = shell?.getBoundingClientRect().width;
    const startWidth = sidebar?.getBoundingClientRect().width || sidebarWidth;
    let width = startWidth;
    startResize(event, "col-resize", e => {
      width = clamp(startWidth + e.clientX - start);
      document.documentElement.style.setProperty("--sidebar-width", `${width}px`);
      if (shell && shellWidth) shell.style.setProperty("--sidebar-width", `${workspaceLayout(shellWidth, width, false, 0, false).sidebarWidth}px`);
    }, () => setSidebarWidth(width));
  };
  return { sidebarWidth, sidebarCollapsed, setSidebarWidth, setSidebarCollapsed: (value: boolean) => write(collapsedKey, String(value)), onSidebarResizeStart, isResizing };
}
