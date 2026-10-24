import type { ReactNode } from "react";
import { BrowserToggle } from "@/features/browser/BrowserDock";
import { HeaderActions } from "@app/components/HeaderActions";

export function ModeHeader({ title, children, navigation }: { title: string; children?: ReactNode; navigation: ReactNode }) {
  return <div className="main-topbar"><header className="main-header">
    {navigation}<div className="workspace-header"><strong className="workspace-title">{title}</strong></div>
    <HeaderActions>{children}<BrowserToggle /></HeaderActions>
  </header></div>;
}
