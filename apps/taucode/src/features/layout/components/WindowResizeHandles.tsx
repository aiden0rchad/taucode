import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";
import { isMacPlatform } from "@utils/platformPaths";
import { usePanelResize } from "../hooks/usePanelResize";

const directions = ["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"] as const;

export function WindowResizeHandles() {
  const { startResize } = usePanelResize();
  if (!isTauri()) return null;
  return <div className="window-resize-handles" aria-hidden="true">{directions.map(direction => <div key={direction} className={`window-resize-edge ${direction}`} onPointerDown={event => {
    if (event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    const window = getCurrentWindow();
    if (!isMacPlatform()) { void window.startResizeDragging(direction).catch(() => {}); return; }
    // Tauri's native resize-drag API is unsupported on macOS. Screen coordinates
    // stay anchored while the window moves; convert screen points to physical pixels.
    const origin = { x: event.screenX, y: event.screenY };
    const initial = Promise.all([window.outerPosition(), window.outerSize(), window.scaleFactor(), window.isFullscreen()]);
    void initial.catch(() => {});
    let cursor = origin, pending = false, moving = false;
    const move = (event: PointerEvent) => {
      cursor = { x: event.screenX, y: event.screenY };
      pending = true;
      if (moving) return;
      moving = true;
      void (async () => {
        const [position, size, scale, fullscreen] = await initial;
        while (pending && !fullscreen) {
          pending = false;
          const dx = (cursor.x - origin.x) * scale, dy = (cursor.y - origin.y) * scale;
          const width = Math.max(360 * scale, size.width + (direction.includes("East") ? dx : direction.includes("West") ? -dx : 0));
          const height = Math.max(600 * scale, size.height + (direction.includes("South") ? dy : direction.includes("North") ? -dy : 0));
          await window.setSize(new PhysicalSize(width, height));
          if (direction.includes("West") || direction.includes("North")) {
            await window.setPosition(new PhysicalPosition(position.x + (direction.includes("West") ? size.width - width : 0), position.y + (direction.includes("North") ? size.height - height : 0)));
          }
        }
      })().catch(error => { console.warn("Window resize failed", error); }).finally(() => { moving = false; });
    };
    startResize(event, direction === "North" || direction === "South" ? "row-resize" : "col-resize", move);
  }} />)}</div>;
}
