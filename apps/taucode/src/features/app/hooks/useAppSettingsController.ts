import { useThemePreference } from "../../layout/hooks/useThemePreference";
import { useUiScaleShortcuts } from "../../layout/hooks/useUiScaleShortcuts";
import { useAppSettings } from "../../settings/hooks/useAppSettings";
import { runCodexUpdate } from "../../../services/tauri";

export function useAppSettingsController() {
  const {
    settings: appSettings,
    setSettings: setAppSettings,
    saveSettings,
    doctor,
    isLoading: appSettingsLoading,
  } = useAppSettings();

  useThemePreference(appSettings.theme);

  const {
    uiScale,
    scaleShortcutTitle,
    scaleShortcutText,
    queueSaveSettings,
  } = useUiScaleShortcuts({
    settings: appSettings,
    setSettings: setAppSettings,
    saveSettings,
  });

  return {
    appSettings,
    setAppSettings,
    saveSettings,
    queueSaveSettings,
    doctor,
    codexUpdate: (codexBin: string | null, codexArgs: string | null) =>
      runCodexUpdate(codexBin, codexArgs),
    appSettingsLoading,
    uiScale,
    scaleShortcutTitle,
    scaleShortcutText,
  };
}
