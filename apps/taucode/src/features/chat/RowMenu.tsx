import { IconButton } from "@/features/design-system/components/IconButton";
import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import MoreHorizontal from "lucide-react/dist/esm/icons/ellipsis";
import { useMenuController } from "@app/hooks/useMenuController";
import { PopoverMenuItem, PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";

type Action = { label: string; icon?: ReactNode; disabled?: boolean; onClick: () => void };

export function RowMenu({ label, actions, children, portal = false, size = "sm" }: { label: string; actions: Action[]; children: ReactNode; portal?: boolean; size?: "sm" | "md" }) {
  const surface = useRef<HTMLDivElement>(null);
  const menu = useMenuController({ popoverRef: surface });
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (menu.isOpen) surface.current?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();
  }, [menu.isOpen]);
  const anchor = menu.isOpen ? trigger.current?.getBoundingClientRect() : undefined;
  const popover = menu.isOpen && <PopoverSurface ref={surface} className="mode-row-menu" role="menu" aria-label={`Actions for ${label}`} style={{ position: "fixed", ...(portal ? { zIndex: "var(--z-tooltip)", minWidth: 190 } : {}), left: Math.max(8, Math.min(anchor?.left ?? 0, window.innerWidth - 200)), right: "auto", top: Math.max(8, Math.min(anchor?.bottom ?? 0, window.innerHeight - actions.length * 36 - 16)) }}>
      {actions.map(action => <PopoverMenuItem role="menuitem" icon={action.icon} key={action.label} disabled={action.disabled} onClick={() => { menu.setOpen(false); trigger.current?.focus(); action.onClick(); }}>{action.label}</PopoverMenuItem>)}
    </PopoverSurface>;
  return <div className="mode-row" ref={menu.containerRef} onContextMenu={event => { event.preventDefault(); menu.setOpen(true); }} onKeyDown={event => {
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) { event.preventDefault(); menu.setOpen(true); }
    if (menu.isOpen && event.key === "Escape") { event.preventDefault(); event.stopPropagation(); menu.setOpen(false); trigger.current?.focus(); }
    if (menu.isOpen && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const items = Array.from(surface.current!.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'));
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    }
  }}>
    {children}
    <IconButton size={size} data-tauri-drag-region="false" ref={trigger} type="button" className="ghost mode-row-menu-trigger" aria-label={`Actions for ${label}`} aria-haspopup="menu" aria-expanded={menu.isOpen} onClick={menu.toggle}><MoreHorizontal size={14} aria-hidden /></IconButton>
    {portal && popover ? createPortal(popover, document.body) : popover}
  </div>;
}
