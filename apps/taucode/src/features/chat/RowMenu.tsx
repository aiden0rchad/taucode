import { useEffect, useRef, type ReactNode } from "react";
import MoreHorizontal from "lucide-react/dist/esm/icons/ellipsis";
import { useMenuController } from "@app/hooks/useMenuController";
import { PopoverMenuItem, PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";

type Action = { label: string; disabled?: boolean; onClick: () => void };

export function RowMenu({ label, actions, children }: { label: string; actions: Action[]; children: ReactNode }) {
  const menu = useMenuController();
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (menu.isOpen) menu.containerRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();
  }, [menu.isOpen, menu.containerRef]);
  const anchor = menu.isOpen ? trigger.current?.getBoundingClientRect() : undefined;
  return <div className="mode-row" ref={menu.containerRef} onContextMenu={event => { event.preventDefault(); menu.setOpen(true); }} onKeyDown={event => {
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) { event.preventDefault(); menu.setOpen(true); }
    if (event.key === "Escape") { menu.setOpen(false); trigger.current?.focus(); }
    if (menu.isOpen && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'));
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    }
  }}>
    {children}
    <button data-tauri-drag-region="false" ref={trigger} type="button" className="ghost mode-row-menu-trigger" aria-label={`Actions for ${label}`} aria-haspopup="menu" aria-expanded={menu.isOpen} onClick={menu.toggle}><MoreHorizontal size={14} aria-hidden /></button>
    {menu.isOpen && <PopoverSurface className="mode-row-menu" role="menu" aria-label={`Actions for ${label}`} style={{ position: "fixed", left: Math.max(8, Math.min(anchor?.left ?? 0, window.innerWidth - 160)), right: "auto", top: Math.max(8, Math.min(anchor?.bottom ?? 0, window.innerHeight - actions.length * 36 - 16)) }}>
      {actions.map(action => <PopoverMenuItem role="menuitem" key={action.label} disabled={action.disabled} onClick={() => { menu.setOpen(false); action.onClick(); }}>{action.label}</PopoverMenuItem>)}
    </PopoverSurface>}
  </div>;
}
