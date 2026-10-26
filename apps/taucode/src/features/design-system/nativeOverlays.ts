import { useCallback, useRef, type Ref } from "react";

export type OverlayRect = { x: number; y: number; width: number; height: number };
const overlays = new Set<HTMLElement>();
export function intersects(a: OverlayRect, b: OverlayRect) {
  return a.width > 0 && a.height > 0 && b.width > 0 && b.height > 0 && a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}
export function registerNativeOverlay(element: HTMLElement) {
  overlays.add(element);
  return () => { overlays.delete(element); };
}
export function nativeOverlayIntersects(rect: OverlayRect) {
  return [...overlays].some(el => el.isConnected && intersects(rect, el.getBoundingClientRect()));
}
export function useNativeOverlay<T extends HTMLElement>(forwarded?: Ref<T>) {
  const cleanup = useRef<(() => void) | undefined>(undefined);
  return useCallback((element: T | null) => {
    cleanup.current?.();
    cleanup.current = element ? registerNativeOverlay(element) : undefined;
    if (typeof forwarded === "function") forwarded(element);
    else if (forwarded) forwarded.current = element;
  }, [forwarded]);
}

// Native HTML drag images are drawn by the OS; hide the child view while a drag
// is active so it cannot steal the HTML drop target. Keep this scoped to slots.
export function trackNativeDragOverlay() {
  let remove: (() => void) | undefined;
  const start = () => { remove?.(); remove = registerNativeOverlay(document.documentElement); };
  const end = () => { remove?.(); remove = undefined; };
  document.addEventListener("dragstart", start, true);
  document.addEventListener("dragend", end, true);
  document.addEventListener("drop", end, true);
  window.addEventListener("blur", end);
  return () => { end(); document.removeEventListener("dragstart", start, true); document.removeEventListener("dragend", end, true); document.removeEventListener("drop", end, true); window.removeEventListener("blur", end); };
}
