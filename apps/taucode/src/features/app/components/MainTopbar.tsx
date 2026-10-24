import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { BrowserToggle } from "@/features/browser/BrowserDock";

const HeaderSlots = createContext<{ actions: HTMLDivElement | null; panel: HTMLDivElement | null } | null>(null);
export function HeaderActions({ children, panel = false }: { children: ReactNode; panel?: boolean }) {
  const slots = useContext(HeaderSlots);
  const target = panel ? slots?.panel : slots?.actions;
  return target ? createPortal(children, target) : slots ? null : <>{children}</>;
}

type MainTopbarProps = { leftNode: ReactNode; actionsNode?: ReactNode; rightPanelNode?: ReactNode; className?: string };
export function MainTopbar({ leftNode, actionsNode, rightPanelNode, className = "" }: MainTopbarProps) {
  const [actions, setActions] = useState<HTMLDivElement | null>(null);
  const [panel, setPanel] = useState<HTMLDivElement | null>(null);
  const menu = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const media = window.matchMedia("(max-width: 700px)");
    const update = () => { if (menu.current) menu.current.open = !media.matches; };
    update(); media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return <HeaderSlots.Provider value={{ actions, panel }}><header className={`main-topbar ${className}`} data-tauri-drag-region>
    <div className="main-topbar-left">{leftNode}</div>
    <details className="shell-header-menu" open ref={menu} onKeyDown={e => { if (e.key === "Escape" && menu.current && window.matchMedia("(max-width: 700px)").matches) { menu.current.open = false; menu.current.querySelector("summary")?.focus(); } }}>
      <summary aria-label="Header actions" title="Header actions">•••</summary>
      <div className="shell-header-actions"><div className="shell-mode-actions" ref={setActions}>{actionsNode}</div><BrowserToggle /><div className="shell-panel-action" ref={setPanel}>{rightPanelNode}</div></div>
    </details>
  </header></HeaderSlots.Provider>;
}
