import { SettingsGroup, SettingsButton, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
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
  return <SettingsGroup disabled={disabled} aria-label={`Reasoning for ${model.id}`} title={<>Reasoning</>}>

    <div className="reasoning-pills">{cap ? cap.supported ? cap.levels.map(level => <span key={level}>{reasoningLabel(level)}</span>) : <span>Not supported</span> : <span>Unknown</span>}</div>
    {cap && <small>{cap.source === "api" ? "from provider API" : cap.source === "user" ? "set manually" : cap.source === "probe" ? "detected by request" : "estimated"}</small>}
    <SettingsToggleRow  title={<>Supports reasoning</>}><SettingsToggleSwitch aria-label={`Supports reasoning: ${model.id}`} pressed={cap?.supported ?? false} onClick={() => override(!(cap?.supported ?? false), cap?.levels.length ? cap.levels : ["low", "medium", "high"])} /></SettingsToggleRow>
    {cap?.supported && <div>{reasoningLevels.map(level => <SettingsToggleRow key={level} title={<>{reasoningLabel(level)}</>}><SettingsToggleSwitch aria-label={`${reasoningLabel(level)}: ${model.id}`} pressed={cap.levels.includes(level)} onClick={() => override(true, reasoningLevels.filter(l => l === level ? !(cap.levels.includes(level)) : cap.levels.includes(l)))} /></SettingsToggleRow>)}</div>}
    <SettingsButton type="button" disabled={cap?.source !== "user"} onClick={() => onChange({ ...model, reasoning: model.detectedReasoning ?? null })}>Reset to detected</SettingsButton>
    <SettingsButton type="button" onClick={onDetect} title="Sends one minimal request to the provider; usage may be billed">Detect reasoning</SettingsButton>
    <small>Detection sends one minimal request.</small>
  </SettingsGroup>;
}
