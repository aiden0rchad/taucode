import { SettingsRow, SettingsTextArea, SettingsButton, SettingsSelect, SettingsSection, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import type { AppSettings, ModelOption } from "@/types";

type SettingsGitSectionProps = {
  appSettings: AppSettings;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  models: ModelOption[];
  commitMessagePromptDraft: string;
  commitMessagePromptDirty: boolean;
  commitMessagePromptSaving: boolean;
  onSetCommitMessagePromptDraft: (value: string) => void;
  onSaveCommitMessagePrompt: () => Promise<void>;
  onResetCommitMessagePrompt: () => Promise<void>;
};

export function SettingsGitSection({
  appSettings,
  onUpdateAppSettings,
  models,
  commitMessagePromptDraft,
  commitMessagePromptDirty,
  commitMessagePromptSaving,
  onSetCommitMessagePromptDraft,
  onSaveCommitMessagePrompt,
  onResetCommitMessagePrompt,
}: SettingsGitSectionProps) {
  return (
    <SettingsSection
      title="Git"
      subtitle="Manage how diffs are loaded in the Git sidebar."
    >
      <SettingsToggleRow
        title="Preload git diffs"
        subtitle="Make viewing git diff faster."
      >
        <SettingsToggleSwitch
          pressed={appSettings.preloadGitDiffs}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              preloadGitDiffs: !appSettings.preloadGitDiffs,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Ignore whitespace changes"
        subtitle="Hides whitespace-only changes in local and commit diffs."
      >
        <SettingsToggleSwitch
          pressed={appSettings.gitDiffIgnoreWhitespaceChanges}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              gitDiffIgnoreWhitespaceChanges: !appSettings.gitDiffIgnoreWhitespaceChanges,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsRow className="settings-field">
        <div className="settings-field-label">Commit message prompt</div>
        <div className="settings-help">
          Used when generating commit messages. Include <code>{"{diff}"}</code> to insert the
          git diff.
        </div>
        <SettingsTextArea
          className="settings-agents-textarea"
          value={commitMessagePromptDraft}
          onChange={(event) => onSetCommitMessagePromptDraft(event.target.value)}
          spellCheck={false}
          disabled={commitMessagePromptSaving}
        />
        <div className="settings-field-actions">
          <SettingsButton
            type="button"
            className="ghost settings-button-compact"
            onClick={() => {
              void onResetCommitMessagePrompt();
            }}
            disabled={commitMessagePromptSaving || !commitMessagePromptDirty}
          >
            Reset
          </SettingsButton>
          <SettingsButton
            type="button"
            className="primary settings-button-compact"
            onClick={() => {
              void onSaveCommitMessagePrompt();
            }}
            disabled={commitMessagePromptSaving || !commitMessagePromptDirty}
          >
            {commitMessagePromptSaving ? "Saving..." : "Save"}
          </SettingsButton>
        </div>
      </SettingsRow>
      {models.length > 0 && (
        <SettingsRow className="settings-field">
          <label className="settings-field-label" htmlFor="commit-message-model-select">
            Commit message model
          </label>
          <div className="settings-help">
            The model used when generating commit messages. Leave on default to use the
            workspace model.
          </div>
          <SettingsSelect
            id="commit-message-model-select"
            className="settings-select"
            value={appSettings.commitMessageModelId ?? ""}
            onChange={(event) => {
              const value = event.target.value || null;
              void onUpdateAppSettings({
                ...appSettings,
                commitMessageModelId: value,
              });
            }}
          >
            <option value="">Default</option>
            {models.map((model) => (
              <option key={model.id} value={model.model}>
                {model.displayName?.trim() || model.model}
              </option>
            ))}
          </SettingsSelect>
        </SettingsRow>
      )}
    </SettingsSection>
  );
}
