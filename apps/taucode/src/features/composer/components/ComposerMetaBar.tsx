import { useEngines, resolveEngine } from "@settings/engines";
import { useFeatureSettings } from "@settings/features";
import { ReasoningControl } from "./ReasoningControl";
import { useEffect, useState } from "react";
import { engineRuntime } from "@services/tauri";

type ComposerMetaBarProps = {
  disabled: boolean;
  engine?: import("@/types").EngineKind;
  onSelectEngine?: (engine: import("@/types").EngineKind) => void;
  models: (Pick<import("@/types").ModelOption, "id" | "displayName" | "model" | "providerId" | "providerName" | "engines" | "reasoning" | "acpReasoning">)[];
  selectedModelId: string | null;
  defaultModelId?: string | null;
  onSelectModel: (id: string) => void;
  selectedEffort: string | null;
  onSelectEffort: (effort: string) => void;
};

export function ComposerMetaBar({ disabled, engine = "codex", models, selectedModelId, defaultModelId, onSelectModel, selectedEffort, onSelectEffort }: ComposerMetaBarProps) {
  const protocol = useEngines().find(e => e.id === engine)?.protocol;
  const modelEngine = protocol === "pi-rpc" || (protocol === "acp" && engine !== "claude") ? "pi" : engine;
  const selected = models.find(model => model.id === selectedModelId);
  return <ReasoningControl
    modelName={selected?.displayName || selected?.model || "Select model"}
    models={models.map(model => ({ id: model.id, name: model.displayName || model.model, provider: model.providerName || model.providerId, disabled: !!model.engines && !model.engines.includes(modelEngine), disabledReason: model.engines && !model.engines.includes(modelEngine) ? "Unavailable for " + engine : undefined }))}
    selectedModelId={selectedModelId} defaultModelId={defaultModelId} onSelectModel={onSelectModel}
    capability={engine === "claude" ? selected?.acpReasoning : selected?.reasoning}
    value={selectedEffort} onChange={onSelectEffort} disabled={disabled}
    unavailable={engine === "claude" && !selected?.acpReasoning ? "This Claude session does not advertise a reasoning option" : undefined}
  />;
}

export function ComposerEngineControl({ engine = "codex", models, selectedModelId, onSelectModel, onSelectEngine, disabled }: Pick<ComposerMetaBarProps, "engine" | "models" | "selectedModelId" | "onSelectModel" | "onSelectEngine" | "disabled">) {
  const engines = useEngines();
  const settings = useFeatureSettings();
  const resolved = resolveEngine(engine, settings);
  const [runtimeReason, setRuntimeReason] = useState("");
  const hasClaude = models.some(model => model.engines?.includes("claude"));
  useEffect(() => {
    if (!hasClaude && engine !== "claude") return;
    let active = true;
    void engineRuntime("status").then(runtimes => {
      const runtime = runtimes.find(item => item.engine === "claude-acp");
      if (active) setRuntimeReason(runtime && ["error", "installing", "preparing"].includes(runtime.state) ? runtime.message : "");
    }).catch(() => {});
    return () => { active = false; };
  }, [engine, hasClaude]);
  const reason = hasClaude ? runtimeReason : "Claude Code needs an Anthropic or Anthropic-compatible provider and model";
  return onSelectEngine ? <><select aria-label="Engine" value={resolved.engine.id} disabled={disabled} onChange={event => {
    const next = event.target.value as import("@/types").EngineKind;
    if (!models.find(model => model.id === selectedModelId)?.engines?.includes(next)) {
      const model = models.find(item => item.engines?.includes(next));
      if (model) onSelectModel(model.id);
    }
    onSelectEngine(next);
  }}>{engines.filter(e => e.enabled).map(e => <option key={e.id} value={e.id} disabled={e.id === "claude" && !!reason}>{e.name}{e.id === "claude" && reason ? " — " + reason : ""}</option>)}</select>{resolved.notice && <span role="status">{resolved.notice}</span>}</> : <span aria-label="Thread engine">{resolved.engine.name}{resolved.notice && <span role="status">{resolved.notice}</span>}{!resolved.engine.enabled && <span role="status">Enable {resolved.engine.name} to continue</span>}</span>;
}
