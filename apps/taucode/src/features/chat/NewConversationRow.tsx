import { IconButton } from "@/features/design-system/components/IconButton";
import { useEffect } from "react";
import { Search, SquarePen } from "lucide-react";
import { formatShortcut, matchesShortcut } from "@utils/shortcuts";

export function NewConversationRow({ label, onNew, onSearch, shortcut, handleShortcut = false, disabled = false, searchLabel = "Search conversations", searchExpanded }: {
  label: string; onNew: () => void; onSearch: () => void; shortcut?: string | null; handleShortcut?: boolean; disabled?: boolean; searchLabel?: string; searchExpanded?: boolean;
}) {
  useEffect(() => {
    if (!handleShortcut || !shortcut || disabled) return;
    const keydown = (event: KeyboardEvent) => {
      if (!event.defaultPrevented && matchesShortcut(event, shortcut)) { event.preventDefault(); onNew(); }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [disabled, handleShortcut, onNew, shortcut]);
  return <div className="sidebar-new-row">
    <button type="button" className="ghost sidebar-labeled-button mode-new" aria-label={label} title={disabled ? "Please wait for the current operation" : label} disabled={disabled} onClick={onNew}><SquarePen aria-hidden />{label}{shortcut && <kbd>{formatShortcut(shortcut)}</kbd>}</button>
    <IconButton type="button" className="ghost" aria-label={searchLabel} title={searchLabel} aria-expanded={searchExpanded} onClick={onSearch}><Search aria-hidden /></IconButton>
  </div>;
}
