import { useRef } from "react";
import { createPortal } from "react-dom";
import { Settings2 } from "lucide-react";
import { useMenuController } from "@app/hooks/useMenuController";
import { PopoverSurface } from "@/features/design-system/components/popover/PopoverPrimitives";
export function WorkConcurrency({ value, disabled, onChange }: { value: number; disabled: boolean; onChange: (value: number) => void }) {
  const popover = useRef<HTMLDivElement>(null);
  const menu = useMenuController({ popoverRef: popover });
  const rect = menu.containerRef.current?.getBoundingClientRect();
  return <div ref={menu.containerRef}><button className="ghost icon-button" aria-label="Space options" aria-haspopup="dialog" aria-expanded={menu.isOpen} onClick={menu.toggle}><Settings2 aria-hidden /></button>{menu.isOpen && createPortal(<PopoverSurface ref={popover} role="dialog" aria-label="Space options" className="work-concurrency-popover" style={{ position: "fixed", top: Math.min(rect?.bottom ?? 0, window.innerHeight - 80), left: Math.max(8, Math.min(rect?.left ?? 0, window.innerWidth - 240)), width: 230 }}><label className="work-concurrency">Concurrent tasks <select autoFocus value={value} disabled={disabled} onChange={e => onChange(Number(e.target.value))}>{[1,2,3,4,5,6,7,8].map(n => <option key={n}>{n}</option>)}</select></label></PopoverSurface>, document.body)}</div>;
}
