import { createContext, useContext, type ReactNode } from "react";
import Globe from "lucide-react/dist/esm/icons/globe";
import { Button } from "@/features/design-system/components/Primitives";
import { useBrowserController } from "@app/hooks/useBrowserController";
import { BrowserPane } from "./BrowserPane";

const BrowserContext = createContext<ReturnType<typeof useBrowserController> | null>(null);
export function BrowserToggle() {
  const browser = useContext(BrowserContext);
  return <Button variant="ghost" className="browser-header-toggle" title="Browser" aria-label="Browser" aria-pressed={browser?.open ?? false} onClick={browser?.toggle}><Globe size={16} aria-hidden /></Button>;
}

export function BrowserDock({ children, searchUrl, onInterrupt }: { children: ReactNode; searchUrl?: string; onInterrupt?: () => void | Promise<void> }) {
  const browser = useBrowserController();
  return <BrowserContext.Provider value={browser}><div className="browser-workspace">
    <div className="browser-main">{children}</div>
    {browser.open ? <aside className="browser-dock" style={{ width: browser.width }}>
      <div className="browser-resize" role="separator" aria-label="Resize browser pane" aria-orientation="vertical" aria-valuenow={browser.width} aria-valuemin={320} aria-valuemax={900} tabIndex={0} onKeyDown={e => { if (["ArrowLeft","ArrowRight"].includes(e.key)) { e.preventDefault(); browser.setWidth(w => Math.min(900,Math.max(320,w + (e.key === "ArrowLeft" ? 20 : -20)))); } }} onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); }} onPointerMove={e => { if (e.currentTarget.hasPointerCapture(e.pointerId)) browser.setWidth(Math.min(900,Math.max(320,window.innerWidth-e.clientX))); }} onPointerUp={e => e.currentTarget.releasePointerCapture(e.pointerId)} />
      <BrowserPane searchUrl={searchUrl} agentActive={browser.agentActive} onTakeOver={browser.agentActive ? browser.takeOver : onInterrupt ?? browser.takeOver} onClose={() => browser.setOpen(false)} />
    </aside> : null}
  </div></BrowserContext.Provider>;
}
