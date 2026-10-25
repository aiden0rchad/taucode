import { SettingsButton, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import type { WorkspaceInfo, WorkspaceSettings } from "@/types";
import { useCodegraphStatus } from "@settings/hooks/useCodegraphStatus";

type Props = {
  workspace: WorkspaceInfo;
  globalEnabled: boolean;
  onUpdate: (id: string, settings: Partial<WorkspaceSettings>) => Promise<void>;
};

export function CodegraphProjectSettings({ workspace, globalEnabled, onUpdate }: Props) {
  const registryOverride = workspace.settings.mcpServers?.find(s => s.id === "codegraph");
  const override = registryOverride?.enabled ?? workspace.settings.codegraphEnabled;
  const enabled = registryOverride?.enabled ?? (globalEnabled && (override ?? true));
  const update = (enabled: boolean | null) => onUpdate(workspace.id, { codegraphEnabled: enabled, ...(registryOverride ? { mcpServers: [...(workspace.settings.mcpServers ?? []).filter(s => s.id !== "codegraph"), ...(enabled === null ? [] : [{ id: "codegraph", enabled }])] } : {}) });
  const { status, error, busy, run, rebuild } = useCodegraphStatus(workspace.id, globalEnabled, override);
  return <div>
    <SettingsToggleRow title="CodeGraph" subtitle="Changes apply on the next app-server spawn.">
      <SettingsToggleSwitch aria-label={`CodeGraph for ${workspace.name}`} pressed={enabled}
        disabled={(!globalEnabled && !registryOverride) || busy}
        onClick={() => void run(() => update(!enabled))} />
    </SettingsToggleRow>
    <div className="settings-help" role="status">
      {!enabled ? "disabled" : status ? `${status.state}: ${status.detail}` : "Loading CodeGraph status…"}
    </div>
    {error && <div role="alert" className="settings-error">{error}</div>}
    <SettingsButton type="button" className="ghost settings-button-compact" disabled={busy || !enabled || status?.state === "indexing"}
      onClick={() => void rebuild()}>Rebuild index</SettingsButton>
    {override != null && <SettingsButton type="button" className="ghost settings-button-compact" disabled={busy}
      onClick={() => void run(() => update(null))}>Use app setting</SettingsButton>}
  </div>;
}
