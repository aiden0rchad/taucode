import ScrollText from "lucide-react/dist/esm/icons/scroll-text";
import { SidebarFooter } from "./AppShell";
import { Button } from "@/features/design-system/components/Primitives";
type SidebarBottomRailProps = { onOpenSettings: () => void; onOpenDebug: () => void; showDebugButton: boolean };
export function SidebarBottomRail({ onOpenSettings, onOpenDebug, showDebugButton }: SidebarBottomRailProps) {
  return <SidebarFooter onOpenSettings={onOpenSettings}>{showDebugButton && <Button variant="ghost" onClick={onOpenDebug} aria-label="Open debug log"><ScrollText size={14} aria-hidden /></Button>}</SidebarFooter>;
}
