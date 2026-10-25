import { useEffect, useRef, type CSSProperties } from "react";
import { BrainCog, Flame } from "lucide-react";
import type { ReasoningCapability } from "@/types";
import { useMenuController } from "@app/hooks/useMenuController";
import { AnchoredPopover } from "@/features/design-system/components/popover/AnchoredPopover";
import { clampReasoning, reasoningLabel } from "@/features/models/reasoning";
import "./reasoning.css";

export type ReasoningControlProps = { capability?: ReasoningCapability | null; value?: string | null; onChange: (value: string) => void; disabled?: boolean; unavailable?: string; onOpenSettings?: () => void };
export function ReasoningControl({ capability, value, onChange, disabled, unavailable, onOpenSettings }: ReasoningControlProps) {
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const menu = useMenuController({ popoverRef: popover, onDismiss: () => trigger.current?.focus() });
  useEffect(() => { if (menu.isOpen) popover.current?.querySelector<HTMLInputElement>('input')?.focus(); }, [menu.isOpen]);
  const level = clampReasoning(capability, value);
  if (unavailable) return <span className="reasoning-unavailable" title={unavailable} aria-label={unavailable}><BrainCog size={14} aria-hidden /></span>;
  if (capability?.supported === false) return null;
  const adjusted = !!value && !!level && value !== level;
  const unknown = !capability || !level;
  const label = unknown ? "?" : reasoningLabel(level);
  const index = capability && level ? capability.levels.indexOf(level) : 0;
  const last = (capability?.levels.length ?? 1) - 1;
  const heat = last > 0 ? index / last : 1;
  const burning = !unknown && index === last;
  const warm = !unknown && !burning && index === last - 1;
  const sliderStyle = { "--reasoning-position": `${heat * 100}%`, "--reasoning-color": heat < 0.5 ? `color-mix(in srgb, #849bb6, #e9ac38 ${heat * 200}%)` : `color-mix(in srgb, #e9ac38, #f06436 ${(heat - 0.5) * 200}%)` } as CSSProperties;
  return <div className="reasoning-control" ref={menu.containerRef}>
    <button ref={trigger} type="button" className="ghost composer-chip" aria-label={`Reasoning: ${label}${adjusted ? " (adjusted for model)" : ""}`} aria-haspopup="dialog" aria-expanded={menu.isOpen} disabled={disabled} onClick={menu.toggle} title={adjusted ? `${reasoningLabel(value!)} is unavailable; using ${label}` : "Reasoning effort"}>{burning ? <Flame className="reasoning-flame" size={14} aria-hidden /> : <BrainCog size={14} aria-hidden />}{label}{adjusted && <small> · adjusted</small>}</button>
    {menu.isOpen && <AnchoredPopover anchorRef={trigger} surfaceRef={popover} className={`reasoning-popover${burning ? " burning" : warm ? " warm" : ""}`} style={sliderStyle} role="dialog" aria-label="Reasoning effort">
      {unknown ? <><p>Reasoning capabilities are unknown.</p><button type="button" onClick={onOpenSettings ?? (() => window.dispatchEvent(new Event("taucode:provider-settings")))}>Detect in Settings</button></> : <>
        <label><span className="reasoning-heading">Reasoning effort{burning && <Flame className="reasoning-flame" size={16} aria-hidden />}</span><span className="reasoning-slider">
          <span className="reasoning-rail" aria-hidden="true"><span className="reasoning-fill" /><span className="reasoning-thumb">{burning && Array.from({ length: 6 }, (_, ember) => <span className="reasoning-ember" key={ember} />)}</span></span>
          <input type="range" aria-label="Reasoning level" min={0} max={capability.levels.length - 1} step={1} value={index} aria-valuetext={label} onChange={event => onChange(capability.levels[Number(event.target.value)])} onKeyDown={event => {
          const index = capability.levels.indexOf(level);
          const next = { ArrowRight: index + 1, ArrowUp: index + 1, ArrowLeft: index - 1, ArrowDown: index - 1, Home: 0, End: capability.levels.length - 1 }[event.key];
          if (next !== undefined) { event.preventDefault(); onChange(capability.levels[Math.max(0, Math.min(capability.levels.length - 1, next))]); }
        }} /></span></label>
        <div className="reasoning-stops">{capability.levels.map(stop => <button type="button" key={stop} aria-pressed={stop === level} onClick={() => onChange(stop)}>{reasoningLabel(stop)}</button>)}</div>
        {adjusted && <p role="status">{reasoningLabel(value!)} is unavailable; using {label}.</p>}
      </>}
    </AnchoredPopover>}
  </div>;
}
