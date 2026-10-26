import { SettingsIconButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { SettingsFieldRow, SettingsTextField, SettingsButton, SettingsSelect, SettingsPage, SettingsGroup } from "@/features/design-system/components/settings/SettingsPrimitives";
import { CodegraphProjectSettings } from "./CodegraphProjectSettings";
import type { WorkspaceSettings } from "@/types";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronUp from "lucide-react/dist/esm/icons/chevron-up";
import Trash2 from "lucide-react/dist/esm/icons/trash-2";
import type { Dispatch, SetStateAction } from "react";
import type { WorkspaceGroup, WorkspaceInfo } from "@/types";

type GroupedWorkspaces = Array<{
  id: string | null;
  name: string;
  workspaces: WorkspaceInfo[];
}>;

type SettingsProjectsSectionProps = {
  codegraphEnabled: boolean;
  onUpdateWorkspaceSettings: (id: string, settings: Partial<WorkspaceSettings>) => Promise<void>;
  workspaceGroups: WorkspaceGroup[];
  groupedWorkspaces: GroupedWorkspaces;
  ungroupedLabel: string;
  groupDrafts: Record<string, string>;
  newGroupName: string;
  groupError: string | null;
  projects: WorkspaceInfo[];
  canCreateGroup: boolean;
  onSetNewGroupName: Dispatch<SetStateAction<string>>;
  onSetGroupDrafts: Dispatch<SetStateAction<Record<string, string>>>;
  onCreateGroup: () => Promise<void>;
  onRenameGroup: (group: WorkspaceGroup) => Promise<void>;
  onMoveWorkspaceGroup: (id: string, direction: "up" | "down") => Promise<boolean | null>;
  onDeleteGroup: (group: WorkspaceGroup) => Promise<void>;
  onChooseGroupCopiesFolder: (group: WorkspaceGroup) => Promise<void>;
  onClearGroupCopiesFolder: (group: WorkspaceGroup) => Promise<void>;
  onAssignWorkspaceGroup: (workspaceId: string, groupId: string | null) => Promise<boolean | null>;
  onMoveWorkspace: (id: string, direction: "up" | "down") => void;
  onDeleteWorkspace: (id: string) => void;
};

export function SettingsProjectsSection({
  codegraphEnabled,
  onUpdateWorkspaceSettings,
  workspaceGroups,
  groupedWorkspaces,
  ungroupedLabel,
  groupDrafts,
  newGroupName,
  groupError,
  projects,
  canCreateGroup,
  onSetNewGroupName,
  onSetGroupDrafts,
  onCreateGroup,
  onRenameGroup,
  onMoveWorkspaceGroup,
  onDeleteGroup,
  onChooseGroupCopiesFolder,
  onClearGroupCopiesFolder,
  onAssignWorkspaceGroup,
  onMoveWorkspace,
  onDeleteWorkspace,
}: SettingsProjectsSectionProps) {
  return (
    <SettingsPage
      title="Projects"
      subtitle="Group related workspaces and reorder projects within each group."
    >
      <SettingsGroup
        title="Groups"
        subtitle="Create group labels for related repositories."
      >
      <div className="settings-groups">
        <div className="settings-group-create">
          <SettingsTextField
            className="settings-input settings-input--compact"
            value={newGroupName}
            placeholder="New group name"
            onChange={(event) => onSetNewGroupName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && canCreateGroup) {
                event.preventDefault();
                void onCreateGroup();
              }
            }}
          />
          <SettingsButton
            type="button"
            className="ghost settings-button-compact"
            onClick={() => {
              void onCreateGroup();
            }}
            disabled={!canCreateGroup}
          >
            Add group
          </SettingsButton>
        </div>
        {groupError && <div className="settings-group-error">{groupError}</div>}
        {workspaceGroups.length > 0 ? (
          <div className="settings-group-list">
            {workspaceGroups.map((group, index) => (
              <div key={group.id} className="settings-group-row">
                <div className="settings-group-fields">
                  <SettingsTextField
                    className="settings-input settings-input--compact"
                    value={groupDrafts[group.id] ?? group.name}
                    onChange={(event) =>
                      onSetGroupDrafts((prev) => ({
                        ...prev,
                        [group.id]: event.target.value,
                      }))
                    }
                    onBlur={() => {
                      void onRenameGroup(group);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        void onRenameGroup(group);
                      }
                    }}
                  />
                  <div className="settings-group-copies">
                    <div className="settings-group-copies-label">Copies folder</div>
                    <div className="settings-group-copies-row">
                      <div
                        className={`settings-group-copies-path${group.copiesFolder ? "" : " empty"}`}
                        title={group.copiesFolder ?? ""}
                      >
                        {group.copiesFolder ?? "Not set"}
                      </div>
                      <SettingsButton
                        type="button"
                        className="ghost settings-button-compact"
                        onClick={() => {
                          void onChooseGroupCopiesFolder(group);
                        }}
                      >
                        Choose…
                      </SettingsButton>
                      <SettingsButton
                        type="button"
                        className="ghost settings-button-compact"
                        onClick={() => {
                          void onClearGroupCopiesFolder(group);
                        }}
                        disabled={!group.copiesFolder}
                      >
                        Clear
                      </SettingsButton>
                    </div>
                  </div>
                </div>
                <div className="settings-group-actions">
                  <SettingsIconButton
                    type="button"
                    className="ghost"
                    onClick={() => {
                      void onMoveWorkspaceGroup(group.id, "up");
                    }}
                    disabled={index === 0}
                    aria-label="Move group up"
                  >
                    <ChevronUp aria-hidden />
                  </SettingsIconButton>
                  <SettingsIconButton
                    type="button"
                    className="ghost"
                    onClick={() => {
                      void onMoveWorkspaceGroup(group.id, "down");
                    }}
                    disabled={index === workspaceGroups.length - 1}
                    aria-label="Move group down"
                  >
                    <ChevronDown aria-hidden />
                  </SettingsIconButton>
                  <SettingsIconButton
                    type="button"
                    className="ghost"
                    onClick={() => {
                      void onDeleteGroup(group);
                    }}
                    aria-label="Delete group"
                  >
                    <Trash2 aria-hidden />
                  </SettingsIconButton>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="settings-empty">No groups yet.</div>
        )}
      </div>
      </SettingsGroup><SettingsGroup
        title="Projects"
        subtitle="Assign projects to groups and adjust their order."
      >
      <div className="settings-projects">
        {groupedWorkspaces.map((group) => (
          <div key={group.id ?? "ungrouped"} className="settings-project-group">
            <div className="settings-project-group-label">{group.name}</div>
            {group.workspaces.map((workspace, index) => {
              const groupValue = workspaceGroups.some(
                (entry) => entry.id === workspace.settings.groupId,
              )
                ? workspace.settings.groupId ?? ""
                : "";
              return (
                <details key={workspace.id} className="settings-project-row">
                  <summary><strong>{workspace.name}</strong><span>{workspace.path}</span></summary>
                  <div className="settings-project-info">
                    <div className="settings-project-name">{workspace.name}</div>
                    <div className="settings-project-path">{workspace.path}</div>
                    <CodegraphProjectSettings workspace={workspace} globalEnabled={codegraphEnabled} onUpdate={onUpdateWorkspaceSettings} />
                  </div>
                  <SettingsFieldRow title="Group">
                    <SettingsSelect
                      className="settings-select settings-select--compact"
                      aria-label={"Group for " + workspace.name}
                      value={groupValue}
                      onChange={(event) => {
                        const nextGroupId = event.target.value || null;
                        void onAssignWorkspaceGroup(workspace.id, nextGroupId);
                      }}
                    >
                      <option value="">{ungroupedLabel}</option>
                      {workspaceGroups.map((entry) => (
                        <option key={entry.id} value={entry.id}>
                          {entry.name}
                        </option>
                      ))}
                    </SettingsSelect>
                  </SettingsFieldRow>
                  <div className="settings-project-actions">
                    <SettingsIconButton
                      type="button"
                      className="ghost"
                      onClick={() => onMoveWorkspace(workspace.id, "up")}
                      disabled={index === 0}
                      aria-label="Move project up"
                    >
                      <ChevronUp aria-hidden />
                    </SettingsIconButton>
                    <SettingsIconButton
                      type="button"
                      className="ghost"
                      onClick={() => onMoveWorkspace(workspace.id, "down")}
                      disabled={index === group.workspaces.length - 1}
                      aria-label="Move project down"
                    >
                      <ChevronDown aria-hidden />
                    </SettingsIconButton>
                    <SettingsIconButton
                      type="button"
                      className="ghost"
                      onClick={() => onDeleteWorkspace(workspace.id)}
                      aria-label="Delete project"
                    >
                      <Trash2 aria-hidden />
                    </SettingsIconButton>
                  </div>
                </details>
              );
            })}
          </div>
        ))}
        {projects.length === 0 && <div className="settings-empty">No projects yet.</div>}
      </div>
    </SettingsGroup></SettingsPage>
  );
}
