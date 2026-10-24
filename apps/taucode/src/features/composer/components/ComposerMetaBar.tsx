import type { CSSProperties } from "react";
import { useEffect, useState } from "react";
import { engineRuntime } from "@services/tauri";
import { BrainCog, SlidersHorizontal, Zap, ListChecks, Shield } from "lucide-react";
import type { AccessMode, ServiceTier, ThreadTokenUsage } from "../../../types";
import type { CodexArgsOption } from "../../threads/utils/codexArgsProfiles";

type ComposerMetaBarProps = {
  disabled: boolean;
  collaborationModes: { id: string; label: string }[];
  selectedCollaborationModeId: string | null;
  onSelectCollaborationMode: (id: string | null) => void;
  engine?: import("@/types").EngineKind;
  onSelectEngine?: (engine: import("@/types").EngineKind) => void;
  models: (Pick<import("@/types").ModelOption, "id" | "displayName" | "model" | "providerId" | "providerName" | "engines">)[];
  selectedModelId: string | null;
  onSelectModel: (id: string) => void;
  reasoningOptions: string[];
  selectedEffort: string | null;
  onSelectEffort: (effort: string) => void;
  selectedServiceTier: ServiceTier | null;
  reasoningSupported: boolean;
  accessMode: AccessMode;
  onSelectAccessMode: (mode: AccessMode) => void;
  codexArgsOptions?: CodexArgsOption[];
  selectedCodexArgsOverride?: string | null;
  onSelectCodexArgsOverride?: (value: string | null) => void;
  contextUsage?: ThreadTokenUsage | null;
};

export function ComposerMetaBar({
  disabled,
  collaborationModes,
  selectedCollaborationModeId,
  onSelectCollaborationMode,
  engine = "codex", onSelectEngine,
  models,
  selectedModelId,
  onSelectModel,
  reasoningOptions,
  selectedEffort,
  onSelectEffort,
  selectedServiceTier,
  reasoningSupported,
  accessMode,
  onSelectAccessMode,
  codexArgsOptions = [],
  selectedCodexArgsOverride = null,
  onSelectCodexArgsOverride,
  contextUsage = null,
}: ComposerMetaBarProps) {
  const [claudeRuntimeReason, setClaudeRuntimeReason] = useState("");
  const hasClaudeModels = models.some(m => m.engines?.includes("claude"));
  useEffect(() => {
    if (!hasClaudeModels && engine !== "claude") return;
    let active = true;
    void engineRuntime("status").then(runtimes => {
      const runtime = runtimes.find(r => r.engine === "claude-acp");
      if (active) setClaudeRuntimeReason(runtime && ["error", "installing", "preparing"].includes(runtime.state) ? runtime.message : "");
    }).catch(() => {});
    return () => { active = false; };
  }, [engine, hasClaudeModels]);
  const claudeReason = hasClaudeModels ? claudeRuntimeReason : "Claude Code needs an Anthropic or Anthropic-compatible provider and model";
  const selectEngine = (next: import("@/types").EngineKind) => {
    if (!models.find(m => m.id === selectedModelId)?.engines?.includes(next)) {
      const model = models.find(m => m.engines?.includes(next));
      if (model) onSelectModel(model.id);
    }
    onSelectEngine?.(next);
  };
  const selectedModel =
    models.find((model) => model.id === selectedModelId) ?? null;
  const selectedModelLabel =
    selectedModel?.displayName || selectedModel?.model || "No models";
  const modelSelectStyle = {
    "--composer-model-select-width": `${Math.max(selectedModelLabel.length + 2, 8)}ch`,
  } as CSSProperties;
  const contextWindow = contextUsage?.modelContextWindow ?? null;
  const lastTokens = contextUsage?.last.totalTokens ?? 0;
  const totalTokens = contextUsage?.total.totalTokens ?? 0;
  const usedTokens = lastTokens > 0 ? lastTokens : totalTokens;
  const contextFreePercent =
    contextWindow && contextWindow > 0 && usedTokens > 0
      ? Math.max(
          0,
          100 -
            Math.min(Math.max((usedTokens / contextWindow) * 100, 0), 100),
        )
      : null;
  const planMode =
    collaborationModes.find((mode) => mode.id === "plan") ?? null;
  const defaultMode =
    collaborationModes.find((mode) => mode.id === "default") ?? null;
  const canUsePlanToggle =
    Boolean(planMode) &&
    collaborationModes.every(
      (mode) => mode.id === "default" || mode.id === "plan",
    );
  const planSelected = selectedCollaborationModeId === (planMode?.id ?? "");

  return (
    <div className="composer-bar">
      <div className="composer-meta">
        {collaborationModes.length > 0 && (
          canUsePlanToggle ? (
            <div className="composer-select-wrap composer-plan-toggle-wrap">
              <label className="composer-plan-toggle" aria-label="Plan mode">
                <input
                  className="composer-plan-toggle-input"
                  type="checkbox"
                  checked={planSelected}
                  disabled={disabled}
                  onChange={(event) =>
                    onSelectCollaborationMode(
                      event.target.checked
                        ? planMode?.id ?? "plan"
                        : (defaultMode?.id ?? null),
                    )
                  }
                />
                <span className="composer-plan-toggle-icon" aria-hidden>
                  <ListChecks aria-hidden />
                </span>
                <span className="composer-plan-toggle-label">
                  {planMode?.label || "Plan"}
                </span>
              </label>
            </div>
          ) : (
            <div className="composer-select-wrap">
            <span className="composer-icon" aria-hidden>
              <ListChecks aria-hidden />
            </span>
              <select
                className="composer-select composer-select--model composer-select--collab"
                aria-label="Collaboration mode"
                value={selectedCollaborationModeId ?? ""}
                onChange={(event) =>
                  onSelectCollaborationMode(event.target.value || null)
                }
                disabled={disabled}
              >
                {collaborationModes.map((mode) => (
                  <option key={mode.id} value={mode.id}>
                    {mode.label || mode.id}
                  </option>
                ))}
              </select>
            </div>
          )
        )}
        <div className="composer-select-wrap composer-select-wrap--model">
          <span className="composer-icon composer-icon--model" aria-hidden>
            <span className="provider-dot">{(selectedModel?.providerName || selectedModel?.providerId || selectedModelLabel)[0]?.toUpperCase()}</span>
          </span>
          {onSelectEngine ? <select aria-label="Engine" value={engine} disabled={disabled} onChange={e => selectEngine(e.target.value as import("@/types").EngineKind)}><option value="pi">pi</option><option value="codex">codex</option><option value="claude" disabled={!!claudeReason} title={claudeReason || undefined}>Claude Code (ACP){claudeReason ? " — " + claudeReason : ""}</option></select> : <span className="engine-badge" aria-label="Thread engine">{engine === "claude" ? "Claude Code (ACP)" : engine}</span>}
          <select
            className="composer-select composer-select--model"
            aria-label="Model"
            value={selectedModelId ?? ""}
            onChange={(event) => onSelectModel(event.target.value)}
            disabled={disabled}
            style={modelSelectStyle}
          >
            {models.length === 0 && <option value="">No models</option>}
            {[...new Set(models.map(m => m.providerName ?? m.providerId ?? "Models"))].map(provider => <optgroup key={provider} label={provider}>
              {models.filter(m => (m.providerName ?? m.providerId ?? "Models") === provider).map(model => {
                const unavailable = model.engines && !model.engines.includes(engine);
                return <option key={model.id} value={model.id} disabled={unavailable} title={unavailable ? "Unavailable for " + engine + ". Continue in a new pi thread (coming later)." : undefined}>{model.displayName || model.model}</option>;
              })}
            </optgroup>)}
          </select>
          {selectedServiceTier === "fast" && (
            <span
              className="composer-fast-indicator"
              role="status"
              aria-label="Fast mode enabled"
              title="Fast mode enabled"
            >
              <Zap size={12} strokeWidth={1.8} />
            </span>
          )}
        </div>
        <div className="composer-select-wrap composer-select-wrap--effort">
          <span className="composer-icon composer-icon--effort" aria-hidden>
            <BrainCog size={14} strokeWidth={1.8} />
          </span>
          <select
            className="composer-select composer-select--effort"
            aria-label="Thinking mode"
            title={!reasoningSupported ? "This model does not support reasoning effort" : "Reasoning effort"}
            value={selectedEffort ?? ""}
            onChange={(event) => onSelectEffort(event.target.value)}
            disabled={disabled || !reasoningSupported}
          >
            {reasoningOptions.length === 0 && <option value="">Default</option>}
            {reasoningOptions.map((effort) => (
              <option key={effort} value={effort}>
                {effort}
              </option>
            ))}
          </select>
        </div>
        {codexArgsOptions.length > 1 && onSelectCodexArgsOverride && (
          <div className="composer-select-wrap">
            <span className="composer-icon" aria-hidden>
              <SlidersHorizontal size={14} strokeWidth={1.8} />
            </span>
            <select
              className="composer-select composer-select--approval"
              aria-label="Codex args profile"
              disabled={disabled}
              value={selectedCodexArgsOverride ?? ""}
              onChange={(event) =>
                onSelectCodexArgsOverride(event.target.value || null)
              }
            >
              {codexArgsOptions.map((option) => (
                <option key={option.value || "default"} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        )}
        <div className="composer-select-wrap composer-access-chip">
          <span className="composer-icon" aria-hidden>
            <Shield aria-hidden />
          </span>
          <select
            className="composer-select composer-select--approval"
            aria-label="Agent access"
            disabled={disabled}
            value={accessMode}
            onChange={(event) =>
              onSelectAccessMode(event.target.value as AccessMode)
            }
          >
            <option value="read-only">Read-only</option>
            <option value="current">Ask</option>
            <option value="full-access">Full access</option>
          </select>
        </div>
      </div>
      <div className="composer-context">
        <div
          className="composer-context-ring"
          data-tooltip={
            contextFreePercent === null
              ? "Context free --"
              : `Context free ${Math.round(contextFreePercent)}%`
          }
          aria-label={
            contextFreePercent === null
              ? "Context free --"
              : `Context free ${Math.round(contextFreePercent)}%`
          }
          style={
            {
              "--context-free": contextFreePercent ?? 0,
            } as CSSProperties
          }
        >
          <span className="composer-context-value" />
        </div>
      </div>
    </div>
  );
}
