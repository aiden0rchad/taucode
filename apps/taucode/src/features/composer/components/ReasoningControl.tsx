import { useEffect, useRef, useState, type CSSProperties, type PointerEvent, type RefObject } from "react";
import { BrainCog, ChevronDown, Flame, RotateCcw, Zap } from "lucide-react";
import type { ReasoningCapability } from "@/types";
import { useMenuController } from "@app/hooks/useMenuController";
import { AnchoredPopover } from "@/features/design-system/components/popover/AnchoredPopover";
import { clampReasoning, reasoningLabel } from "@/features/models/reasoning";
import "./reasoning.css";

export type ReasoningControlProps = {
  capability?: ReasoningCapability | null;
  value?: string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
  unavailable?: string;
  onOpenSettings?: () => void;
  modelName?: string;
  modelPickerRef?: RefObject<HTMLSelectElement | null>;
};
export function ReasoningControl({ capability, value, onChange, disabled, unavailable, onOpenSettings, modelName = "Model", modelPickerRef }: ReasoningControlProps) {
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const menu = useMenuController({ popoverRef: popover, onDismiss: () => trigger.current?.focus() });
  useEffect(() => {
    if (menu.isOpen) popover.current?.querySelector<HTMLInputElement>("input")?.focus();
    else setDrag(null);
  }, [menu.isOpen]);
  const level = clampReasoning(capability, value);
  if (unavailable) return <span className="reasoning-unavailable" title={unavailable} aria-label={unavailable}><BrainCog size={14} aria-hidden /></span>;
  if (capability?.supported === false) return null;
  const adjusted = !!value && !!level && value !== level;
  const unknown = !capability || !level;
  const levels = capability?.levels ?? [];
  const index = level ? levels.indexOf(level) : 0;
  const last = Math.max(0, levels.length - 1);
  const position = drag ?? index;
  const shownLevel = levels[Math.round(position)] ?? level;
  const label = unknown ? "?" : reasoningLabel(shownLevel!);
  const chipLabel = level ? reasoningLabel(level) : "?";
  const labels = levels.length ? levels.map(reasoningLabel) : ["?"];
  const heat = last > 0 ? position / last : 1;
  const burning = !unknown && position === last;
  const warm = !unknown && !burning && Math.round(position) === last - 1;
  const sliderStyle = {
    "--reasoning-position": `${last > 0 ? heat * 100 : 0}%`,
    "--reasoning-color": heat < 0.5 ? `color-mix(in srgb, #8998ef, #e9ac38 ${heat * 200}%)` : `color-mix(in srgb, #e9ac38, #f06436 ${(heat - 0.5) * 200}%)`,
  } as CSSProperties;
  const pointerPosition = (event: PointerEvent<HTMLInputElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return Math.max(0, Math.min(1, (event.clientX - rect.left - 18) / Math.max(1, rect.width - 36))) * last;
  };
  const select = (next: number) => { setDrag(null); onChange(levels[Math.max(0, Math.min(last, next))]); };
  return <div className="reasoning-control" ref={menu.containerRef}>
    <button ref={trigger} type="button" className="ghost composer-chip reasoning-chip" style={{ minWidth: `calc(${Math.max(...labels.map(text => text.length))}ch + 30px)` }} aria-label={`Reasoning: ${chipLabel}${adjusted ? " (adjusted for model)" : ""}`} aria-haspopup="dialog" aria-expanded={menu.isOpen} disabled={disabled} onClick={menu.toggle} title={adjusted ? `${reasoningLabel(value!)} is unavailable; using ${chipLabel}` : "Reasoning effort"}>
      {!unknown && index === last ? <Flame className="reasoning-flame" size={14} aria-hidden /> : <BrainCog size={14} aria-hidden />}
      <span className="reasoning-chip-label">{labels.map(text => <span className="reasoning-chip-sizer" aria-hidden="true" key={text}>{text}</span>)}<span>{chipLabel}</span></span>
    </button>
    {menu.isOpen && <AnchoredPopover anchorRef={trigger} surfaceRef={popover} className={`reasoning-popover${burning ? " burning" : warm ? " warm" : ""}${drag !== null ? " dragging" : ""}`} style={sliderStyle} role="dialog" aria-label="Reasoning effort">
      {unknown ? <><p>Reasoning capabilities are unknown.</p><button type="button" onClick={onOpenSettings ?? (() => window.dispatchEvent(new Event("taucode:provider-settings")))}>Detect in Settings</button></> : <>
        <div className="reasoning-heading">
          <span className="reasoning-heading-icon" aria-hidden="true">{burning ? <Flame className="reasoning-flame" size={22} /> : <Zap size={22} />}</span>
          <div className="reasoning-heading-center">
            <span className="reasoning-level">{label}</span>
            <button type="button" className="reasoning-model" title={modelName} disabled={!modelPickerRef || disabled} onClick={() => {
              const picker = modelPickerRef?.current;
              if (!picker || picker.disabled) return;
              menu.close();
              picker.focus();
              if (picker.showPicker) picker.showPicker();
              else picker.click();
            }}><span>{modelName}</span><ChevronDown size={12} aria-hidden /></button>
          </div>
          <button type="button" className="reasoning-reset" title="Reset to model default" aria-label="Reset to model default" onClick={() => { setDrag(null); onChange(clampReasoning(capability, capability.default)!); }}><RotateCcw size={16} aria-hidden /></button>
        </div>
        <div className="reasoning-slider">
          <div className="reasoning-rail" aria-hidden="true">
            <span className="reasoning-fill">{!burning && Array.from({ length: 10 }, (_, dot) => <span className="reasoning-sparkle" key={dot} style={{ left: `${7 + dot * 9}%`, top: `${25 + (dot % 3) * 23}%`, animationDelay: `${dot * -0.23}s` }} />)}</span>
            {levels.map((stop, stopIndex) => <span className="reasoning-tick" key={stop} style={{ left: `${last > 0 ? stopIndex / last * 100 : 0}%` }} />)}
            <span className="reasoning-thumb">{burning && Array.from({ length: 6 }, (_, ember) => <span className="reasoning-ember" key={ember} />)}</span>
          </div>
          <input type="range" aria-label="Reasoning level" min={0} max={last} step="any" value={position} aria-valuetext={label} onChange={event => select(Math.round(Number(event.target.value)))}
            onPointerDown={event => { if (event.button !== 0) return; event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId); setDrag(pointerPosition(event)); }}
            onPointerMove={event => { if (event.currentTarget.hasPointerCapture(event.pointerId)) setDrag(pointerPosition(event)); }}
            onPointerUp={event => { if (!event.currentTarget.hasPointerCapture(event.pointerId)) return; select(Math.round(pointerPosition(event))); event.currentTarget.releasePointerCapture(event.pointerId); }}
            onPointerCancel={() => setDrag(null)} onLostPointerCapture={() => setDrag(null)}
            onKeyDown={event => {
              const next = { ArrowRight: index + 1, ArrowUp: index + 1, ArrowLeft: index - 1, ArrowDown: index - 1, Home: 0, End: last }[event.key];
              if (next !== undefined) { event.preventDefault(); select(next); }
            }} />
        </div>
        <p className="reasoning-notice" role="status">{adjusted ? `${reasoningLabel(value!)} is unavailable; using ${chipLabel}.` : ""}</p>
      </>}
    </AnchoredPopover>}
  </div>;
}
