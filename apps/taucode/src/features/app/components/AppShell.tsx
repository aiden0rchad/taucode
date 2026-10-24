import { useWindowDrag } from "@/features/layout/hooks/useWindowDrag";
import { isWindowsPlatform } from "@utils/platformPaths";
import type { ComponentProps, CSSProperties, ReactNode } from "react";
import Settings from "lucide-react/dist/esm/icons/settings";
import { ModeSwitcher, useModeContext } from "@/features/chat/ModeShell";
import { useResizablePanels } from "@/features/layout/hooks/useResizablePanels";
import { Button } from "@/features/design-system/components/Primitives";
import { WindowCaptionControls } from "@/features/layout/components/WindowCaptionControls";

export function AppShell({ className = "", ...props }: ComponentProps<"div">) {
  return <div className={`app-shell ${className}`} {...props} />;
}
export function PrimaryAction({ children, ...props }: ComponentProps<typeof Button>) {
  return <div className="shell-primary-action"><Button {...props}>{children}</Button></div>;
}
export function SidebarFooter({ onOpenSettings, children }: { onOpenSettings?: () => void; children?: ReactNode }) {
  const context = useModeContext();
  return <footer className="shell-footer"><Button variant="ghost" aria-label="Open settings" onClick={onOpenSettings ?? context?.openSettings}><Settings size={14} aria-hidden />Settings</Button>{children}</footer>;
}
export function ModeAppShell({ className, sidebar, footer, children, showList }: { className: string; sidebar: ReactNode; footer?: ReactNode; children: ReactNode; showList: boolean }) {
  useWindowDrag("titlebar");
  const { sidebarWidth, appRef, isResizing, onSidebarResizeStart } = useResizablePanels();
  return <AppShell ref={appRef} className={`chat-layout ${className}${showList ? " chat-list-open" : ""}${isResizing ? " is-resizing" : ""}`} style={{ "--sidebar-width": `${sidebarWidth}px`, "--sidebar-top-padding": isWindowsPlatform() ? "10px" : "36px", "--window-caption-width": isWindowsPlatform() ? "138px" : "0px" } as CSSProperties}>
    <div className="drag-strip" id="titlebar" data-tauri-drag-region />
    <WindowCaptionControls />
    <aside className="chat-sidebar"><ModeSwitcher />{sidebar}<SidebarFooter>{footer}</SidebarFooter></aside>
    <div className="sidebar-resizer" role="separator" aria-label="Resize sidebar" aria-orientation="vertical" onMouseDown={onSidebarResizeStart} />
    {children}
  </AppShell>;
}
