import type { ProviderModel, ReasoningCapability } from "@/types";
import { reasoningLabel, reasoningLevels } from "@/features/models/reasoning";
import "@/features/composer/components/reasoning.css";

export function ModelReasoningSettings({ model, disabled, onChange, onDetect }: { model: ProviderModel; disabled: boolean; onChange: (model: ProviderModel) => void; onDetect: () => void }) {
  const cap = model.reasoning;
  const override = (supported: boolean, levels: string[]) => {
    supported = supported && levels.length > 0;
    const next: ReasoningCapability = { supported, levels: supported ? levels : [], default: supported ? levels.includes("medium") ? "medium" : levels[0] ?? null : null, control: cap?.control ?? "effort", source: "user", detectedAt: Math.floor(Date.now() / 1000) };
    onChange({ ...model, detectedReasoning: cap?.source === "user" ? model.detectedReasoning : cap, reasoning: next });
  };
  return <fieldset disabled={disabled} aria-label={`Reasoning for ${model.id}`}>
    <legend>Reasoning</legend>
    <div className="reasoning-pills">{cap ? cap.supported ? cap.levels.map(level => <span key={level}>{reasoningLabel(level)}</span>) : <span>Not supported</span> : <span>Unknown</span>}</div>
    {cap && <small>{cap.source === "api" ? "from provider API" : cap.source === "user" ? "set manually" : cap.source === "probe" ? "detected by request" : "estimated"}</small>}
    <label><input type="checkbox" aria-label={`Supports reasoning: ${model.id}`} checked={cap?.supported ?? false} onChange={e => override(e.target.checked, cap?.levels.length ? cap.levels : ["low", "medium", "high"])} />Supports reasoning</label>
    {cap?.supported && <div>{reasoningLevels.map(level => <label key={level}><input type="checkbox" aria-label={`${reasoningLabel(level)}: ${model.id}`} checked={cap.levels.includes(level)} onChange={e => override(true, reasoningLevels.filter(l => l === level ? e.target.checked : cap.levels.includes(l)))} />{reasoningLabel(level)}</label>)}</div>}
    <button type="button" disabled={cap?.source !== "user"} onClick={() => onChange({ ...model, reasoning: model.detectedReasoning ?? null })}>Reset to detected</button>
    <button type="button" onClick={onDetect} title="Sends one minimal request to the provider; usage may be billed">Detect reasoning</button>
    <small>Detection sends one minimal request.</small>
  </fieldset>;
}
