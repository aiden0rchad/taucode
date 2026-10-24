import { useEffect, useRef, type ReactNode } from "react";
import Ellipsis from "lucide-react/dist/esm/icons/ellipsis";
import { useMenuController } from "@app/hooks/useMenuController";
import { MenuTrigger } from "@/features/design-system/components/popover/PopoverPrimitives";

export function HeaderActions({ children }: { children: ReactNode }) {
  const menu = useMenuController();
  const trigger = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (menu.isOpen) menu.containerRef.current?.querySelector<HTMLButtonElement>(".main-header-actions-content button:not(:disabled)")?.focus();
  }, [menu.isOpen, menu.containerRef]);
  return <div className="main-header-actions" ref={menu.containerRef} onKeyDown={event => {
    if (event.key === "Escape") { menu.close(); trigger.current?.querySelector("button")?.focus(); }
  }}>
    <div className="main-header-overflow-trigger" ref={trigger}><MenuTrigger className="ghost main-header-action" isOpen={menu.isOpen} popupRole="dialog" aria-label="Header actions" title="More actions" onClick={menu.toggle}><Ellipsis size={14} aria-hidden /></MenuTrigger></div>
    <div className={`main-header-actions-content${menu.isOpen ? " is-open" : ""}`} role={menu.isOpen ? "dialog" : undefined} aria-label={menu.isOpen ? "Header actions" : undefined}>{children}</div>
  </div>;
}
