import { SettingsIconButton } from "@/features/design-system/components/settings/SettingsPrimitives";
import { SettingsRow, SettingsTextField, SettingsSelect, SettingsButton, SettingsSection } from "@/features/design-system/components/settings/SettingsPrimitives";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronUp from "lucide-react/dist/esm/icons/chevron-up";
import Trash2 from "lucide-react/dist/esm/icons/trash-2";
import type { OpenAppTarget } from "@/types";
import {
  fileManagerName,
  isMacPlatform,
} from "@utils/platformPaths";
import {
  GENERIC_APP_ICON,
  getKnownOpenAppIcon,
} from "@app/utils/openAppIcons";
import type { OpenAppDraft } from "@settings/components/settingsTypes";

type SettingsOpenAppsSectionProps = {
  openAppDrafts: OpenAppDraft[];
  openAppSelectedId: string;
  openAppIconById: Record<string, string>;
  onOpenAppDraftChange: (index: number, updates: Partial<OpenAppDraft>) => void;
  onOpenAppKindChange: (index: number, kind: OpenAppTarget["kind"]) => void;
  onCommitOpenApps: () => void;
  onMoveOpenApp: (index: number, direction: "up" | "down") => void;
  onDeleteOpenApp: (index: number) => void;
  onAddOpenApp: () => void;
  onSelectOpenAppDefault: (id: string) => void;
};

const isOpenAppLabelValid = (label: string) => label.trim().length > 0;

export function SettingsOpenAppsSection({
  openAppDrafts,
  openAppSelectedId,
  openAppIconById,
  onOpenAppDraftChange,
  onOpenAppKindChange,
  onCommitOpenApps,
  onMoveOpenApp,
  onDeleteOpenApp,
  onAddOpenApp,
  onSelectOpenAppDefault,
}: SettingsOpenAppsSectionProps) {
  return (
    <SettingsSection
      title="Open in"
      subtitle="Customize the Open in menu shown in the title bar and file previews."
    >
      <div className="settings-open-apps">
        {openAppDrafts.map((target, index) => {
          const iconSrc =
            getKnownOpenAppIcon(target.id) ?? openAppIconById[target.id] ?? GENERIC_APP_ICON;
          const labelValid = isOpenAppLabelValid(target.label);
          const appNameValid = target.kind !== "app" || Boolean(target.appName?.trim());
          const commandValid =
            target.kind !== "command" || Boolean(target.command?.trim());
          const isComplete = labelValid && appNameValid && commandValid;
          const incompleteHint = !labelValid
            ? "Label required"
            : target.kind === "app"
              ? "App name required"
              : target.kind === "command"
                ? "Command required"
                : "Complete required fields";

          return (
            <SettingsRow
              key={target.id}
              className={`settings-open-app-row${isComplete ? "" : " is-incomplete"}`}
            >
              <div className="settings-open-app-icon-wrap" aria-hidden>
                <img
                  className="settings-open-app-icon"
                  src={iconSrc}
                  alt=""
                  width={18}
                  height={18}
                />
              </div>
              <div className="settings-open-app-fields">
                <label className="settings-open-app-field settings-open-app-field--label">
                  <span className="settings-visually-hidden">Label</span>
                  <SettingsTextField
                    className="settings-input settings-input--compact settings-open-app-input settings-open-app-input--label"
                    value={target.label}
                    placeholder="Label"
                    onChange={(event) =>
                      onOpenAppDraftChange(index, {
                        label: event.target.value,
                      })
                    }
                    onBlur={onCommitOpenApps}
                    aria-label={`Open app label ${index + 1}`}
                    data-invalid={!labelValid || undefined}
                  />
                </label>
                <label className="settings-open-app-field settings-open-app-field--type">
                  <span className="settings-visually-hidden">Type</span>
                  <SettingsSelect
                    className="settings-select settings-select--compact settings-open-app-kind"
                    value={target.kind}
                    onChange={(event) =>
                      onOpenAppKindChange(index, event.target.value as OpenAppTarget["kind"])
                    }
                    aria-label={`Open app type ${index + 1}`}
                  >
                    <option value="app">App</option>
                    <option value="command">Command</option>
                    <option value="finder">{fileManagerName()}</option>
                  </SettingsSelect>
                </label>
                {target.kind === "app" && (
                  <label className="settings-open-app-field settings-open-app-field--appname">
                    <span className="settings-visually-hidden">App name</span>
                    <SettingsTextField
                      className="settings-input settings-input--compact settings-open-app-input settings-open-app-input--appname"
                      value={target.appName ?? ""}
                      placeholder="App name"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          appName: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app name ${index + 1}`}
                      data-invalid={!appNameValid || undefined}
                    />
                  </label>
                )}
                {target.kind === "command" && (
                  <label className="settings-open-app-field settings-open-app-field--command">
                    <span className="settings-visually-hidden">Command</span>
                    <SettingsTextField
                      className="settings-input settings-input--compact settings-open-app-input settings-open-app-input--command"
                      value={target.command ?? ""}
                      placeholder="Command"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          command: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app command ${index + 1}`}
                      data-invalid={!commandValid || undefined}
                    />
                  </label>
                )}
                {target.kind !== "finder" && (
                  <label className="settings-open-app-field settings-open-app-field--args">
                    <span className="settings-visually-hidden">Args</span>
                    <SettingsTextField
                      className="settings-input settings-input--compact settings-open-app-input settings-open-app-input--args"
                      value={target.argsText}
                      placeholder="Args"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          argsText: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app args ${index + 1}`}
                    />
                  </label>
                )}
              </div>
              <div className="settings-open-app-actions">
                {!isComplete && (
                  <span
                    className="settings-open-app-status"
                    title={incompleteHint}
                    aria-label={incompleteHint}
                  >
                    Incomplete
                  </span>
                )}
                <SettingsButton className="settings-open-app-default" aria-pressed={target.id === openAppSelectedId}
                  onClick={() => onSelectOpenAppDefault(target.id)} disabled={!isComplete}>Default</SettingsButton>
                <div className="settings-open-app-order">
                  <SettingsIconButton
                    type="button"
                    className="ghost"
                    onClick={() => onMoveOpenApp(index, "up")}
                    disabled={index === 0}
                    aria-label="Move up"
                  >
                    <ChevronUp aria-hidden />
                  </SettingsIconButton>
                  <SettingsIconButton
                    type="button"
                    className="ghost"
                    onClick={() => onMoveOpenApp(index, "down")}
                    disabled={index === openAppDrafts.length - 1}
                    aria-label="Move down"
                  >
                    <ChevronDown aria-hidden />
                  </SettingsIconButton>
                </div>
                <SettingsIconButton
                  type="button"
                  className="ghost"
                  onClick={() => onDeleteOpenApp(index)}
                  disabled={openAppDrafts.length <= 1}
                  aria-label="Remove app"
                  title="Remove app"
                >
                  <Trash2 aria-hidden />
                </SettingsIconButton>
              </div>
            </SettingsRow>
          );
        })}
      </div>
      <div className="settings-open-app-footer">
        <SettingsButton type="button" className="ghost" onClick={onAddOpenApp}>
          Add app
        </SettingsButton>
        <div className="settings-help">
          Commands receive the selected path as the final argument.{" "}
          {isMacPlatform()
            ? "Apps open via `open -a` with optional args."
            : "Apps run as an executable with optional args."}
        </div>
      </div>
    </SettingsSection>
  );
}
