import { useFeatures } from "@settings/features";
import { useEffect } from "react";
import { matchesShortcut } from "../../../utils/shortcuts";

type UseBranchSwitcherShortcutOptions = {
  shortcut: string | null;
  isEnabled: boolean;
  onTrigger: () => void;
};

export function useBranchSwitcherShortcut({
  shortcut,
  isEnabled,
  onTrigger,
}: UseBranchSwitcherShortcutOptions) {
  const git = useFeatures().git;
  isEnabled = isEnabled && git;
  useEffect(() => {
    if (!isEnabled || !shortcut) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (matchesShortcut(event, shortcut)) {
        event.preventDefault();
        onTrigger();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isEnabled, onTrigger, shortcut]);
}
