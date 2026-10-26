import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { registerNativeOverlay } from "@/features/design-system/nativeOverlays";

export function usePanelResize() {
  const cleanup = useRef<(() => void) | null>(null);
  const [isResizing, setIsResizing] = useState(false);
  useEffect(() => () => cleanup.current?.(), []);

  const startResize = useCallback((event: ReactPointerEvent, cursor: "col-resize" | "row-resize", onMove: (event: PointerEvent) => void, onEnd: () => void = () => {}) => {
    if (event.button !== 0 || event.isPrimary === false) return;
    event.preventDefault();
    cleanup.current?.();
    const target = event.currentTarget as HTMLElement;
    const pointerId = event.pointerId;
    target.setPointerCapture(pointerId);
    const overlay = document.createElement("div");
    overlay.className = "panel-resize-overlay";
    overlay.style.cssText = `position:fixed;inset:0;z-index:2147483647;cursor:${cursor};touch-action:none`;
    document.body.append(overlay);
    const unregister = registerNativeOverlay(overlay);
    const { cursor: previousCursor, userSelect } = document.body.style;
    document.body.style.cursor = cursor;
    document.body.style.userSelect = "none";
    const move = (e: PointerEvent) => { if (e.pointerId === pointerId) onMove(e); };
    const end = (e: PointerEvent) => { if (e.pointerId === pointerId) stop(); };
    const stop = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", end);
      target.removeEventListener("pointercancel", end);
      target.removeEventListener("lostpointercapture", end);
      window.removeEventListener("blur", stop);
      if (target.hasPointerCapture(pointerId)) target.releasePointerCapture(pointerId);
      unregister();
      overlay.remove();
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = userSelect;
      cleanup.current = null;
      setIsResizing(false);
      onEnd();
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", end);
    target.addEventListener("pointercancel", end);
    target.addEventListener("lostpointercapture", end);
    window.addEventListener("blur", stop);
    cleanup.current = stop;
    setIsResizing(true);
  }, []);

  return { isResizing, startResize };
}
