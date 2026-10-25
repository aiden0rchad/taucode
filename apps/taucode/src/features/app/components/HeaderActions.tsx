import { useEffect, useRef, useState, type ReactNode } from "react";
import { headerActionsOverflow } from "./headerLayout";
import Ellipsis from "lucide-react/dist/esm/icons/ellipsis";
import { useMenuController } from "@app/hooks/useMenuController";
import { MenuTrigger } from "@/features/design-system/components/popover/PopoverPrimitives";

export function HeaderActions({ children }: { children: ReactNode }) {
  const menu = useMenuController();
  const [overflow, setOverflow] = useState(false);
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const header = content.current?.closest<HTMLElement>(".main-header");
    if (!header || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const element = content.current;
      if (!element) return;
      // Measure the natural row even when its actions are currently in the menu.
      const previous = element.style.cssText;
      element.style.cssText += ";display:flex;position:absolute;visibility:hidden;width:max-content;flex-direction:row";
      const widths = Array.from(element.children).map(child => Math.max(32, (child as HTMLElement).scrollWidth));
      element.style.cssText = previous;
      setOverflow(headerActionsOverflow(header.getBoundingClientRect().width, widths));
    };
    const observer = new ResizeObserver(measure); observer.observe(header); measure();
    return () => observer.disconnect();
  }, [children]);
  const trigger = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (menu.isOpen) menu.containerRef.current?.querySelector<HTMLButtonElement>(".main-header-actions-content button:not(:disabled)")?.focus();
  }, [menu.isOpen, menu.containerRef]);
  return <div className={`main-header-actions${overflow ? " has-overflow" : ""}`} ref={menu.containerRef} onKeyDown={event => {
    if (event.key === "Escape") { menu.close(); trigger.current?.querySelector("button")?.focus(); }
  }}>
    <div className="main-header-overflow-trigger" ref={trigger}><MenuTrigger className="ghost main-header-action" isOpen={menu.isOpen} popupRole="dialog" aria-label="Header actions" title="More actions" onClick={menu.toggle}><Ellipsis size={14} aria-hidden /></MenuTrigger></div>
    <div ref={content} className={`main-header-actions-content${menu.isOpen ? " is-open" : ""}`} role={menu.isOpen ? "dialog" : undefined} aria-label={menu.isOpen ? "Header actions" : undefined}>{children}</div>
  </div>;
}
