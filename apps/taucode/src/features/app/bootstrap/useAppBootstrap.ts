import { useDebugLog } from "@/features/debug/hooks/useDebugLog";
import { useAppSettingsController } from "@app/hooks/useAppSettingsController";
import { useCodeCssVars } from "@app/hooks/useCodeCssVars";
import { useDictationController } from "@app/hooks/useDictationController";

export function useAppBootstrap() {
  const appSettingsState = useAppSettingsController();
  useCodeCssVars(appSettingsState.appSettings);

  const dictationState = useDictationController(appSettingsState.appSettings);
  const debugState = useDebugLog();

  return {
    ...appSettingsState,
    ...dictationState,
    ...debugState,
  };
}
