import { useEffect, useRef } from "react";
import { BrainCog } from "lucide-react";
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
  return <div className="reasoning-control" ref={menu.containerRef}>
    <button ref={trigger} type="button" className="ghost composer-chip" aria-label={`Reasoning: ${label}${adjusted ? " (adjusted for model)" : ""}`} aria-haspopup="dialog" aria-expanded={menu.isOpen} disabled={disabled} onClick={menu.toggle} title={adjusted ? `${reasoningLabel(value!)} is unavailable; using ${label}` : "Reasoning effort"}><BrainCog size={14} aria-hidden />{label}{adjusted && <small> · adjusted</small>}</button>
    {menu.isOpen && <AnchoredPopover anchorRef={trigger} surfaceRef={popover} className="reasoning-popover" role="dialog" aria-label="Reasoning effort">
      {unknown ? <><p>Reasoning capabilities are unknown.</p><button type="button" onClick={onOpenSettings ?? (() => window.dispatchEvent(new Event("taucode:provider-settings")))}>Detect in Settings</button></> : <>
        <label>Reasoning effort<input type="range" aria-label="Reasoning level" min={0} max={capability.levels.length - 1} step={1} value={capability.levels.indexOf(level)} aria-valuetext={label} onChange={event => onChange(capability.levels[Number(event.target.value)])} onKeyDown={event => {
          const index = capability.levels.indexOf(level);
          const next = { ArrowRight: index + 1, ArrowUp: index + 1, ArrowLeft: index - 1, ArrowDown: index - 1, Home: 0, End: capability.levels.length - 1 }[event.key];
          if (next !== undefined) { event.preventDefault(); onChange(capability.levels[Math.max(0, Math.min(capability.levels.length - 1, next))]); }
        }} /></label>
        <div className="reasoning-stops">{capability.levels.map(stop => <button type="button" key={stop} aria-pressed={stop === level} onClick={() => onChange(stop)}>{reasoningLabel(stop)}</button>)}</div>
        <small>{({ api: "from provider API", probe: "detected by request", heuristic: "estimated", user: "set manually" })[capability.source]}</small>
        {adjusted && <p role="status">{reasoningLabel(value!)} is unavailable; using {label}.</p>}
      </>}
    </AnchoredPopover>}
  </div>;
}
