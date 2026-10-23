import ScrollText from "lucide-react/dist/esm/icons/scroll-text";
import Settings from "lucide-react/dist/esm/icons/settings";
type SidebarBottomRailProps = {
  onOpenSettings: () => void;
  onOpenDebug: () => void;
  showDebugButton: boolean;
};
export function SidebarBottomRail({ onOpenSettings, onOpenDebug, showDebugButton }: SidebarBottomRailProps) {
  return <div className="sidebar-bottom-rail"><div className="sidebar-bottom-actions is-compact">
    <button className="ghost sidebar-labeled-button sidebar-utility-button" type="button" onClick={onOpenSettings} aria-label="Open settings"><Settings size={14} aria-hidden /><span>Settings</span></button>
    {showDebugButton && <button className="ghost sidebar-labeled-button sidebar-utility-button" type="button" onClick={onOpenDebug} aria-label="Open debug log"><ScrollText size={14} aria-hidden /></button>}
  </div></div>;
}
