import { useCallback, useEffect, useState } from "react";

export type SettingsSection = import("@settings/components/settingsTypes").CodexSection;

export function useSettingsModalState() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<SettingsSection | null>(
    null,
  );

  const openSettings = useCallback((section?: SettingsSection) => {
    setSettingsSection(section ?? null);
    setSettingsOpen(true);
  }, []);

  useEffect(() => {
    const navigate = (event: Event) => openSettings((event as CustomEvent<SettingsSection>).detail);
    window.addEventListener("taucode:setup-settings", navigate);
    return () => window.removeEventListener("taucode:setup-settings", navigate);
  }, [openSettings]);

  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
    setSettingsSection(null);
  }, []);
  useEffect(() => {
    window.addEventListener("taucode:setup-finished", closeSettings);
    return () => window.removeEventListener("taucode:setup-finished", closeSettings);
  }, [closeSettings]);

  return {
    settingsOpen,
    settingsSection,
    openSettings,
    closeSettings,
    setSettingsOpen,
    setSettingsSection,
  };
}
