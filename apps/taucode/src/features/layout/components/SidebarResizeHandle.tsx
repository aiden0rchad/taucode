import { IconButton } from "@/features/design-system/components/IconButton";
import type { PointerEventHandler } from "react";
import { PanelLeft } from "lucide-react";
import { useSidebarState } from "../hooks/useSidebarState";

export function SidebarResizeHandle({ onPointerDown }: { onPointerDown?: PointerEventHandler<HTMLDivElement> }) {
  const sidebar = useSidebarState();
  return <div className="sidebar-resizer" role="separator" aria-orientation="vertical" aria-label="Resize sidebar" tabIndex={0}
    aria-valuemin={220} aria-valuemax={420} aria-valuenow={sidebar.sidebarWidth}
    onPointerDown={onPointerDown ?? sidebar.onSidebarResizeStart}
    onKeyDown={event => { if (["ArrowLeft", "ArrowRight"].includes(event.key)) { event.preventDefault(); sidebar.setSidebarWidth(sidebar.sidebarWidth + (event.key === "ArrowLeft" ? -10 : 10)); } }} />;
}
export function SidebarCollapseButton() {
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  return <IconButton data-tauri-drag-region="false" className="ghost mode-sidebar-toggle" aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"} onClick={() => setSidebarCollapsed(!sidebarCollapsed)}><PanelLeft size={16} aria-hidden /></IconButton>;
}
