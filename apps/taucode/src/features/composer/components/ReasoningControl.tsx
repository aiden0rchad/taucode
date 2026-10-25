import { useFeatures } from "@settings/features";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type PointerEvent } from "react";
import { ChevronDown, ChevronLeft, ChevronRight, Flame, RotateCcw, Zap } from "lucide-react";
import type { ReasoningCapability } from "@/types";
import { useMenuController } from "@app/hooks/useMenuController";
import { AnchoredPopover } from "@/features/design-system/components/popover/AnchoredPopover";
import { clampReasoning, reasoningLabel } from "@/features/models/reasoning";
import { useReasoningAnimationSpeed } from "./useReasoningAnimationSpeed";
import { ComposerModelList, type ComposerModel } from "./ComposerModelList";
import "./reasoning.css";

export type ReasoningControlProps = {
  capability?: ReasoningCapability | null;
  value?: string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
  unavailable?: string;
  onOpenSettings?: () => void;
  modelName?: string;
  models?: ComposerModel[];
  selectedModelId?: string | null;
  defaultModelId?: string | null;
  onSelectModel?: (id: string) => void | boolean | Promise<void | boolean>;
};
export function ReasoningControl({ capability, value, onChange, disabled, unavailable, onOpenSettings, modelName = "Model", models = [], selectedModelId, defaultModelId, onSelectModel }: ReasoningControlProps) {
  const effects = useFeatures().reasoningEffects;
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const [view, setView] = useState<"slider" | "models">("slider");
  const [outgoing, setOutgoing] = useState<"slider" | "models" | null>(null);
  const previousHeight = useRef(0);
  const returning = useRef(false);
  const switchView = (next: "slider" | "models") => {
    if (next === view) return;
    previousHeight.current = popover.current?.offsetHeight ?? 0;
    returning.current = next === "slider";
    setOutgoing(!effects || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ? null : view);
    setView(next);
  };
  const [selecting, setSelecting] = useState(false);
  const [modelError, setModelError] = useState("");
  const menu = useMenuController({ closeOnEscape: view === "slider", popoverRef: popover, onDismiss: () => trigger.current?.focus() });
  useLayoutEffect(() => {
    if (!menu.isOpen || !outgoing) return;
    const surface = popover.current!;
    const incoming = surface.querySelector<HTMLElement>(".reasoning-view.incoming")!;
    const animation = effects && surface.animate?.(
      [{ height: previousHeight.current + "px" }, { height: incoming.offsetHeight + 26 + "px" }],
      { duration: 220, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
    );
    const timer = window.setTimeout(() => setOutgoing(null), 220);
    return () => { window.clearTimeout(timer); animation && animation.cancel(); };
  }, [menu.isOpen, view, outgoing, effects]);
  useEffect(() => {
    if (menu.isOpen) {
      const surface = popover.current?.querySelector<HTMLElement>(".reasoning-view:not(.outgoing)");
      const target = view === "models" ? surface?.querySelector<HTMLElement>('input[type="search"]') ?? surface?.querySelector<HTMLElement>('[role="option"]:not(:disabled)') : (returning.current ? surface?.querySelector<HTMLElement>(".reasoning-model") : surface?.querySelector<HTMLElement>('input[type="range"]')) ?? surface?.querySelector<HTMLElement>('.reasoning-model');
      target?.focus();
    }
    else { setDrag(null); setOutgoing(null); }
  }, [menu.isOpen, view]);
  const level = clampReasoning(capability, value);
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
  const heat = effects ? (last > 0 ? position / last : 1) : 0;
  useReasoningAnimationSpeed(popover, effects && menu.isOpen && view === "slider" && !!level && !unavailable, heat);
  const noReasoning = capability?.supported === false || !!unavailable;
  const burning = effects && !unknown && position === last;
  const warm = effects && !unknown && !burning && Math.round(position) === last - 1;
  const sliderStyle = {
    "--reasoning-position": `${last > 0 ? position / last * 100 : 0}%`,
    "--reasoning-particle-color": `color-mix(in srgb, white, #ffbd64 ${heat * 15}%)`,
    "--reasoning-color": heat < 0.5 ? `color-mix(in srgb, #8998ef, #e9ac38 ${heat * 200}%)` : `color-mix(in srgb, #e9ac38, #f06436 ${(heat - 0.5) * 200}%)`,
  } as CSSProperties;
  const pointerPosition = (event: PointerEvent<HTMLInputElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return Math.max(0, Math.min(1, (event.clientX - rect.left - 18) / Math.max(1, rect.width - 36))) * last;
  };
  const select = (next: number) => { setDrag(null); onChange(levels[Math.max(0, Math.min(last, next))]); };
  const selectModel = async (id: string) => {
    if (!onSelectModel) return;
    if (id === selectedModelId) { switchView("slider"); return; }
    setSelecting(true); setModelError("");
    try { if (await onSelectModel(id) !== false) { switchView("slider"); setDrag(null); } }
    catch (error) { setModelError(String(error)); }
    finally { setSelecting(false); }
  };
  return <div className="reasoning-control" ref={menu.containerRef}>
    <button ref={trigger} type="button" className="ghost composer-chip reasoning-chip" style={sliderStyle} aria-label={`${modelName}${noReasoning ? "" : ` · ${chipLabel}`}${adjusted && !noReasoning ? " (adjusted for model)" : ""}`} aria-haspopup="dialog" aria-expanded={menu.isOpen} disabled={disabled} onClick={() => { if (!menu.isOpen) { returning.current = false; setOutgoing(null); setView(noReasoning ? "models" : "slider"); setModelError(""); } menu.toggle(); }} title={unavailable || (adjusted ? `${reasoningLabel(value!)} is unavailable; using ${chipLabel}` : "Model and reasoning")}>
      <span className="reasoning-chip-model">{modelName}</span>{!noReasoning && <span className="reasoning-chip-dot" aria-hidden />}
      {!noReasoning && <span className={`reasoning-chip-label${unknown ? " unknown" : ""}`} style={{ minWidth: `${Math.max(...labels.map(text => text.length))}ch` }}>{labels.map(text => <span className="reasoning-chip-sizer" aria-hidden="true" key={text}>{text}</span>)}<span>{chipLabel}</span></span>}<ChevronDown size={12} aria-hidden />
    </button>
    {menu.isOpen && <AnchoredPopover anchorRef={trigger} surfaceRef={popover} className={`reasoning-popover${effects ? "" : " effects-off"}${burning ? " burning" : warm ? " warm" : ""}${drag !== null ? " dragging" : ""}`} style={sliderStyle} role="dialog" aria-label={view === "models" ? "Select model" : "Model and reasoning"} onKeyDown={event => { if (event.key === "Escape" && view === "models") { event.preventDefault(); event.stopPropagation(); switchView("slider"); } }}>
      {(["slider", "models"] as const).filter(panel => panel === view || panel === outgoing).map(panel => <div key={panel} className={`reasoning-view${outgoing ? ` ${view === "models" ? "forward" : "back"} ${panel === outgoing ? "outgoing" : "incoming"}` : ""}`} aria-hidden={panel === outgoing || undefined} inert={panel === outgoing ? true : undefined}>
      {panel === "models" ? <><div className="reasoning-model-heading"><button type="button" aria-label="Back to reasoning" onClick={() => switchView("slider")}><ChevronLeft size={16} aria-hidden /></button><strong>Select model</strong></div><ComposerModelList models={models} selectedId={selectedModelId} defaultId={defaultModelId} disabled={disabled || selecting} onSelect={selectModel} />{modelError && <p role="alert">{modelError}</p>}</> : <>
        <div className="reasoning-heading">
          <span className="reasoning-heading-icon" aria-hidden="true">{!noReasoning && (burning ? <Flame className="reasoning-flame" size={22} /> : <Zap size={22} />)}</span>
          <div className="reasoning-heading-center">
            <span className="reasoning-level">{noReasoning ? "Model" : label}</span>
            <button type="button" className="reasoning-model" title={modelName} disabled={!onSelectModel || disabled} onClick={() => switchView("models")}><span>{modelName}</span><ChevronRight size={12} aria-hidden /></button>
          </div>
          <button type="button" className="reasoning-reset" disabled={unknown || noReasoning} title="Reset to model default" aria-label="Reset to model default" onClick={() => { setDrag(null); onChange(clampReasoning(capability, capability?.default)!); }}><RotateCcw size={16} aria-hidden /></button>
        </div>
        {noReasoning ? <p className="reasoning-empty">{unavailable || "This model does not support reasoning."}</p> : unknown ? <div className="reasoning-empty"><p>Reasoning capabilities are unknown.</p><button type="button" onClick={onOpenSettings ?? (() => window.dispatchEvent(new Event("taucode:provider-settings")))}>Detect in Settings</button></div> : <div className="reasoning-slider">
          <div className="reasoning-rail" aria-hidden="true">
            <span className="reasoning-fill">{effects && Array.from({ length: 10 }, (_, dot) => <span className="reasoning-particle" key={dot} style={{ left: `${dot * 8}%`, top: `${20 + (dot % 3) * 25}%`, width: `${2 + dot % 2}px`, height: `${2 + dot % 2}px`, "--particle-drift": `${24 + (dot % 3) * 10}px`, "--particle-opacity": 0.35 + (dot % 3) * 0.125, animationDelay: `${dot * -0.43}s` } as CSSProperties} />)}</span>
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
        </div>}
        <p className="reasoning-notice" role="status">{adjusted ? `${reasoningLabel(value!)} is unavailable; using ${chipLabel}.` : ""}</p>
      </>}
      </div>)}
    </AnchoredPopover>}
  </div>;
}
