import { SettingsButton, SettingsGroup, SettingsRow, SettingsSection, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEffect, useState } from "react";
import type { AppSettings } from "@/types";
import {
  checkAppUpdate,
  getAppBuildType,
  openBrowserUrl,
  type AppBuildType,
  type AppUpdateCheck,
} from "@services/tauri";
import { readLastUpdateCheck, recordUpdateCheck } from "@/features/update/hooks/useUpdater";
import { ReleaseNotes } from "@/features/update/components/ReleaseNotes";
import { IssueReportDialog } from "@/features/report/IssueReportDialog";

type SettingsAboutSectionProps = {
  appSettings: AppSettings;
  onToggleAutomaticAppUpdateChecks?: () => void;
};

export function updateCheckSummary(result: AppUpdateCheck) {
  switch (result.status) {
    case "available": return `taucode ${result.latestVersion} is available. You have ${result.currentVersion}.`;
    case "latest": return "You're up to date.";
    case "noReleases": return "No releases published yet.";
    default: return result.message ?? "Couldn't check for updates. Try again later.";
  }
}

export function SettingsAboutSection({ appSettings, onToggleAutomaticAppUpdateChecks }: SettingsAboutSectionProps) {
  const [appBuildType, setAppBuildType] = useState<AppBuildType | "unknown">("unknown");
  const [update, setUpdate] = useState<AppUpdateCheck | null>(() => readLastUpdateCheck()?.result ?? null);
  const [checking, setChecking] = useState(false);
  const [reporting, setReporting] = useState(false);

  useEffect(() => {
    let active = true;
    const loadBuildType = async () => {
      try {
        const value = await getAppBuildType();
        if (active) {
          setAppBuildType(value);
        }
      } catch {
        if (active) {
          setAppBuildType("unknown");
        }
      }
    };
    void loadBuildType();
    return () => {
      active = false;
    };
  }, []);

  const checkNow = async () => {
    setChecking(true);
    try {
      const result = await checkAppUpdate();
      recordUpdateCheck(result);
      setUpdate(result);
    } catch {
      setUpdate({ status: "unavailable", currentVersion: __APP_VERSION__, message: "Couldn't check for updates. Try again later." });
    } finally {
      setChecking(false);
    }
  };

  const buildDateValue = __APP_BUILD_DATE__.trim();
  const parsedBuildDate = Date.parse(buildDateValue);
  const buildDateLabel = Number.isNaN(parsedBuildDate)
    ? buildDateValue || "unknown"
    : new Date(parsedBuildDate).toLocaleString();
  const downloadUrl = update?.downloadUrl ?? update?.htmlUrl;

  return (
    <SettingsSection title="About" subtitle="App version and build metadata.">
      <SettingsButton onClick={() => window.dispatchEvent(new Event("taucode:run-setup"))}>Run setup</SettingsButton>
      <dl className="settings-build-info">
        {Object.entries({ Version: __APP_VERSION__, "Build type": appBuildType, Branch: __APP_GIT_BRANCH__ || "unknown", Commit: __APP_COMMIT_HASH__ || "unknown", "Build date": buildDateLabel }).map(([label, value]) =>
          <SettingsRow key={label}><dt>{label}:</dt><dd><code>{value}</code></dd></SettingsRow>)}
      </dl>
      <SettingsGroup title="App updates" subtitle="taucode checks GitHub Releases and links to the download. It never installs anything by itself.">
        <SettingsToggleRow title="Check for updates automatically" subtitle="At most once a day, when taucode starts.">
          <SettingsToggleSwitch pressed={appSettings.automaticAppUpdateChecksEnabled} onClick={onToggleAutomaticAppUpdateChecks} />
        </SettingsToggleRow>
        <SettingsRow className="settings-update-row">
          <span role="status">{checking ? "Checking GitHub…" : update ? updateCheckSummary(update) : "Not checked yet."}</span>
          <span>
            {update?.status === "available" && downloadUrl && <SettingsButton className="primary" onClick={() => void openBrowserUrl(downloadUrl)}>Download {update.latestVersion}</SettingsButton>}
            <SettingsButton disabled={checking} onClick={() => void checkNow()}>Check for updates</SettingsButton>
          </span>
        </SettingsRow>
        {update?.status === "available" && update.notes && <ReleaseNotes className="update-toast-notes" notes={update.notes} />}
      </SettingsGroup>
      <SettingsGroup title="Feedback" subtitle="Found a bug? Open a prefilled GitHub issue you can review and edit before posting.">
        <SettingsButton onClick={() => setReporting(true)}>Report an issue…</SettingsButton>
      </SettingsGroup>
      {reporting && <IssueReportDialog onClose={() => setReporting(false)} />}
    </SettingsSection>
  );
}
