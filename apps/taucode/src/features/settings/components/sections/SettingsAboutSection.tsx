import { SettingsButton, SettingsRow, SettingsSection } from "@/features/design-system/components/settings/SettingsPrimitives";
import { useEffect, useState } from "react";
import type { AppSettings } from "@/types";
import {
  getAppBuildType,
  type AppBuildType,
} from "@services/tauri";

type SettingsAboutSectionProps = {
  appSettings: AppSettings;
  onToggleAutomaticAppUpdateChecks?: () => void;
};

export function SettingsAboutSection(_props: SettingsAboutSectionProps) {
  const [appBuildType, setAppBuildType] = useState<AppBuildType | "unknown">("unknown");

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


  const buildDateValue = __APP_BUILD_DATE__.trim();
  const parsedBuildDate = Date.parse(buildDateValue);
  const buildDateLabel = Number.isNaN(parsedBuildDate)
    ? buildDateValue || "unknown"
    : new Date(parsedBuildDate).toLocaleString();

  return (
    <SettingsSection title="About" subtitle="App version and build metadata.">
      <SettingsButton onClick={() => window.dispatchEvent(new Event("taucode:setup-checklist"))}>Setup checklist</SettingsButton>
      <dl className="settings-build-info">
        {Object.entries({ Version: __APP_VERSION__, "Build type": appBuildType, Branch: __APP_GIT_BRANCH__ || "unknown", Commit: __APP_COMMIT_HASH__ || "unknown", "Build date": buildDateLabel }).map(([label, value]) =>
          <SettingsRow key={label}><dt>{label}:</dt><dd><code>{value}</code></dd></SettingsRow>)}
      </dl>
    </SettingsSection>
  );
}
