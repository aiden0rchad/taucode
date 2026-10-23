import type { WorkspaceInfo, WorkspaceSettings } from "@/types";
import { SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useCodegraphStatus } from "@settings/hooks/useCodegraphStatus";

type Props = {
  workspace: WorkspaceInfo;
  globalEnabled: boolean;
  onUpdate: (id: string, settings: Partial<WorkspaceSettings>) => Promise<void>;
};

export function CodegraphProjectSettings({ workspace, globalEnabled, onUpdate }: Props) {
  const override = workspace.settings.codegraphEnabled;
  const enabled = globalEnabled && (override ?? true);
  const { status, error, busy, run, rebuild } = useCodegraphStatus(workspace.id, globalEnabled, override);
  return <div>
    <SettingsToggleRow title="CodeGraph" subtitle="Changes apply on the next app-server spawn.">
      <SettingsToggleSwitch aria-label={`CodeGraph for ${workspace.name}`} pressed={enabled}
        disabled={!globalEnabled || busy}
        onClick={() => void run(() => onUpdate(workspace.id, { codegraphEnabled: !enabled }))} />
    </SettingsToggleRow>
    <div className="settings-help" role="status">
      {!enabled ? "disabled" : status ? `${status.state}: ${status.detail}` : "Loading CodeGraph status…"}
    </div>
    {error && <div role="alert" className="settings-error">{error}</div>}
    <button type="button" className="ghost settings-button-compact" disabled={busy || !enabled || status?.state === "indexing"}
      onClick={() => void rebuild()}>Rebuild index</button>
    {override != null && <button type="button" className="ghost settings-button-compact" disabled={busy}
      onClick={() => void run(() => onUpdate(workspace.id, { codegraphEnabled: null }))}>Use app setting</button>}
  </div>;
}
