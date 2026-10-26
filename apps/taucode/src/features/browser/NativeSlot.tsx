import { useEffect, useRef, useState } from "react";
import { browserNativeBounds } from "@services/tauri";
import { nativeOverlayIntersects, trackNativeDragOverlay } from "@/features/design-system/nativeOverlays";

export function roundedRect(rect: DOMRect) {
  return { x: Math.max(0, Math.round(rect.x)), y: Math.max(0, Math.round(rect.y)), w: Math.max(1, Math.round(rect.width)), h: Math.max(1, Math.round(rect.height)) };
}
export function NativeSlot({ tabId, snapshot, ready, blocked, onError }: { tabId: string; snapshot?: string; ready: boolean; blocked: boolean; onError: (error: string) => void }) {
  const slot = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState(true);
  useEffect(() => {
    const element = slot.current;
    if (!element || !tabId) return;
    const stopDrag = trackNativeDragOverlay();
    let frame = 0, disposed = false, pending = false, last = "";
    let rect = roundedRect(element.getBoundingClientRect());
    // One measurement per frame covers resizer drags, ancestor scrolls, transitions,
    // mode changes and overlays. IPC is sent only when rounded bounds/visibility change.
    const measure = () => {
      if (disposed) return;
      const bounds = element.getBoundingClientRect();
      rect = roundedRect(bounds);
      const visible = ready && !blocked && !document.hidden && bounds.width > 0 && bounds.height > 0 && bounds.x >= 0 && bounds.y >= 0 && !nativeOverlayIntersects(bounds);
      setHidden(old => old === !visible ? old : !visible);
      const key = JSON.stringify([rect, visible]);
      if (!pending && key !== last) {
        pending = true; last = key;
        void browserNativeBounds(tabId, rect, visible).catch(e => { if (!disposed) onError(String(e)); }).finally(() => { pending = false; });
      }
      frame = requestAnimationFrame(measure);
    };
    frame = requestAnimationFrame(measure);
    const observer = new ResizeObserver(() => { /* next frame measures the final layout */ });
    observer.observe(element);
    return () => { disposed = true; stopDrag(); cancelAnimationFrame(frame); observer.disconnect(); void browserNativeBounds(tabId, rect, false).catch(() => {}); };
  }, [tabId, ready, blocked, onError]);
  return <div ref={slot} className="browser-native-slot" data-native-hidden={hidden} aria-label="Native browser page">
    {snapshot && <img className="browser-native-snapshot" src={snapshot.startsWith("data:") ? snapshot : `data:image/png;base64,${snapshot}`} alt="Last page snapshot" />}
    {!ready && <div className="browser-empty">Loading page…</div>}
  </div>;
}
