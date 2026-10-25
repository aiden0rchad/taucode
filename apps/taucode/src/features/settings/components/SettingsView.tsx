import { useState } from "react";
import { SettingsPage, SettingsSegmented } from "@/features/design-system/components/settings/SettingsPrimitives";
import { SettingsProvidersSection, SettingsAgentPolicySection, ProjectAgentPolicy } from "./sections/SettingsRegistrySections";
import { SettingsConnectorsSection } from "./sections/SettingsConnectorsSection";
import { SettingsImportSection } from "./sections/SettingsImportSection";
import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left";
import X from "lucide-react/dist/esm/icons/x";
import type {
  AppSettings,
  CodexDoctorResult,
  CodexUpdateResult,
  DictationModelStatus,
  WorkspaceSettings,
  WorkspaceGroup,
  WorkspaceInfo,
} from "@/types";
import { useSettingsViewCloseShortcuts } from "@settings/hooks/useSettingsViewCloseShortcuts";
import { useSettingsViewNavigation } from "@settings/hooks/useSettingsViewNavigation";
import { useSettingsViewOrchestration } from "@settings/hooks/useSettingsViewOrchestration";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsNav } from "./SettingsNav";
import type { CodexSection } from "./settingsTypes";
import { SETTINGS_SECTION_LABELS } from "./settingsViewConstants";
import { SettingsSectionContainers } from "./sections/SettingsSectionContainers";
import { SettingsComputerSection } from "./sections/SettingsComputerSection";
import { SettingsBrowserSection } from "./sections/SettingsBrowserSection";
import { SettingsRuntimeSetup } from "./sections/SettingsRuntimeSetup";

export type SettingsViewProps = {
  activeWorkspace?: WorkspaceInfo | null;
  workspaceGroups: WorkspaceGroup[];
  groupedWorkspaces: Array<{
    id: string | null;
    name: string;
    workspaces: WorkspaceInfo[];
  }>;
  ungroupedLabel: string;
  onClose: () => void;
  onMoveWorkspace: (id: string, direction: "up" | "down") => void;
  onDeleteWorkspace: (id: string) => void;
  onCreateWorkspaceGroup: (name: string) => Promise<WorkspaceGroup | null>;
  onRenameWorkspaceGroup: (id: string, name: string) => Promise<boolean | null>;
  onMoveWorkspaceGroup: (id: string, direction: "up" | "down") => Promise<boolean | null>;
  onDeleteWorkspaceGroup: (id: string) => Promise<boolean | null>;
  onAssignWorkspaceGroup: (
    workspaceId: string,
    groupId: string | null,
  ) => Promise<boolean | null>;
  appSettings: AppSettings;
  openAppIconById: Record<string, string>;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  onToggleAutomaticAppUpdateChecks?: () => void;
  onRunDoctor: (
    codexBin: string | null,
    codexArgs: string | null,
  ) => Promise<CodexDoctorResult>;
  onRunCodexUpdate?: (
    codexBin: string | null,
    codexArgs: string | null,
  ) => Promise<CodexUpdateResult>;
  onUpdateWorkspaceSettings: (
    id: string,
    settings: Partial<WorkspaceSettings>,
  ) => Promise<void>;
  scaleShortcutTitle: string;
  scaleShortcutText: string;
  onTestNotificationSound: () => void;
  onTestSystemNotification: () => void;
  onMobileConnectSuccess?: () => Promise<void> | void;
  dictationModelStatus?: DictationModelStatus | null;
  onDownloadDictationModel?: () => void;
  onCancelDictationDownload?: () => void;
  onRemoveDictationModel?: () => void;
  initialSection?: CodexSection;
};

export function SettingsView({
  activeWorkspace,
  workspaceGroups,
  groupedWorkspaces,
  ungroupedLabel,
  onClose,
  onMoveWorkspace,
  onDeleteWorkspace,
  onCreateWorkspaceGroup,
  onRenameWorkspaceGroup,
  onMoveWorkspaceGroup,
  onDeleteWorkspaceGroup,
  onAssignWorkspaceGroup,
  appSettings,
  openAppIconById,
  onUpdateAppSettings,
  onToggleAutomaticAppUpdateChecks,
  onRunDoctor,
  onRunCodexUpdate,
  onUpdateWorkspaceSettings,
  scaleShortcutTitle,
  scaleShortcutText,
  onTestNotificationSound,
  onTestSystemNotification,
  onMobileConnectSuccess,
  dictationModelStatus,
  onDownloadDictationModel,
  onCancelDictationDownload,
  onRemoveDictationModel,
  initialSection,
}: SettingsViewProps) {
  const {
    activeSection,
    showMobileDetail,
    setShowMobileDetail,
    useMobileMasterDetail,
    handleSelectSection,
  } = useSettingsViewNavigation({ initialSection });

  const orchestration = useSettingsViewOrchestration({
    workspaceGroups,
    groupedWorkspaces,
    ungroupedLabel,
    appSettings,
    openAppIconById,
    onUpdateAppSettings,
    onToggleAutomaticAppUpdateChecks,
    onRunDoctor,
    onRunCodexUpdate,
    onUpdateWorkspaceSettings,
    scaleShortcutTitle,
    scaleShortcutText,
    onTestNotificationSound,
    onTestSystemNotification,
    onMoveWorkspace,
    onDeleteWorkspace,
    onCreateWorkspaceGroup,
    onRenameWorkspaceGroup,
    onMoveWorkspaceGroup,
    onDeleteWorkspaceGroup,
    onAssignWorkspaceGroup,
    onMobileConnectSuccess,
    dictationModelStatus,
    onDownloadDictationModel,
    onCancelDictationDownload,
    onRemoveDictationModel,
  });

  useSettingsViewCloseShortcuts(onClose);

  const [agentTab, setAgentTab] = useState("defaults");
  const manageEngines = () => { setAgentTab("engines"); handleSelectSection("agents"); };
  const activeSectionLabel = SETTINGS_SECTION_LABELS[activeSection];
  const settingsBodyClassName = `settings-body${
    useMobileMasterDetail ? " settings-body-mobile-master-detail" : ""
  }${useMobileMasterDetail && showMobileDetail ? " is-detail-visible" : ""}`;

  return (
    <ModalShell
      className="settings-overlay"
      cardClassName="settings-window"
      onBackdropClick={onClose}
      ariaLabelledBy="settings-modal-title"
    >
      <div className="settings-titlebar">
        <div className="settings-title" id="settings-modal-title">
          Settings
        </div>
        <button
          type="button"
          className="ghost icon-button settings-close"
          onClick={onClose}
          aria-label="Close settings"
        >
          <X aria-hidden />
        </button>
      </div>
      <div className={settingsBodyClassName}>
        {(!useMobileMasterDetail || !showMobileDetail) && (
          <div className="settings-master">
            <SettingsNav
              activeSection={activeSection}
              onSelectSection={handleSelectSection}
              showDisclosure={useMobileMasterDetail}
            />
          </div>
        )}
        {(!useMobileMasterDetail || showMobileDetail) && (
          <div className="settings-detail">
            {useMobileMasterDetail && (
              <div className="settings-mobile-detail-header">
                <button
                  type="button"
                  className="settings-mobile-back"
                  onClick={() => setShowMobileDetail(false)}
                  aria-label="Back to settings sections"
                >
                  <ChevronLeft aria-hidden />
                  Sections
                </button>
                <div className="settings-mobile-detail-title">{activeSectionLabel}</div>
              </div>
            )}
            <div className="settings-content">
              {activeSection === "computer" ? <SettingsComputerSection onManageEngines={manageEngines} appSettings={appSettings} onUpdateAppSettings={onUpdateAppSettings} /> : activeSection === "browser" ? <SettingsBrowserSection onManageEngines={manageEngines} appSettings={appSettings} onUpdateAppSettings={onUpdateAppSettings} /> :
               activeSection === "import" ? <SettingsImportSection /> :
               activeSection === "providers" ? <SettingsProvidersSection appSettings={appSettings} onUpdateAppSettings={onUpdateAppSettings} /> :
               activeSection === "connectors" ? <SettingsConnectorsSection appSettings={appSettings} onUpdateAppSettings={onUpdateAppSettings} workspace={activeWorkspace} onUpdateWorkspaceSettings={onUpdateWorkspaceSettings} /> :
               activeSection === "agents" ? <SettingsPage title="Models & Agents" subtitle="Choose defaults, delegate work and manage engine runtimes.">
                 <SettingsSegmented tabs label="Models & Agents tabs" value={agentTab} options={["defaults", "subagents", "roles", "engines"].map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }))} onChange={setAgentTab} />
                 {agentTab === "engines" ? <><SettingsRuntimeSetup /><details><summary>Codex engine</summary><SettingsSectionContainers activeSection="codex" orchestration={orchestration} /></details><details><summary>Engine features</summary><SettingsSectionContainers activeSection="features" orchestration={orchestration} /></details></> :
                 <SettingsAgentPolicySection tab={agentTab as "defaults" | "subagents" | "roles"} appSettings={appSettings} onUpdateAppSettings={onUpdateAppSettings} workspace={activeWorkspace} />}
               </SettingsPage> :
               <SettingsSectionContainers
                activeSection={activeSection}
                orchestration={orchestration}
              />}
              {activeSection === "projects" && groupedWorkspaces.flatMap(g => g.workspaces).map(w => <ProjectAgentPolicy key={w.id} workspace={w} appSettings={appSettings} onUpdate={onUpdateWorkspaceSettings} />)}
            </div>
          </div>
        )}
      </div>
    </ModalShell>
  );
}
