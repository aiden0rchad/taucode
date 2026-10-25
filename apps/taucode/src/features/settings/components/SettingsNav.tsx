import { Fragment } from "react";
import { KeyRound, Bot, Plug, SquarePen, Palette, Monitor, Keyboard, Mic, GitBranch, AppWindow, Folder, Box, Globe, MousePointer2, Server, Download, Info } from "lucide-react";
import { PanelNavItem, PanelNavList } from "@/features/design-system/components/panel/PanelPrimitives";
import type { CodexSection } from "./settingsTypes";
import { SETTINGS_SECTION_LABELS } from "./settingsViewConstants";
export const SETTINGS_GROUPS: { label: string; sections: CodexSection[] }[] = [
  { label: "Core", sections: ["appearance", "providers", "agents", "connectors", "composer", "display", "shortcuts", "dictation"] },
  { label: "Workspace", sections: ["git", "open-apps", "projects", "environments"] },
  { label: "Tools", sections: ["browser", "computer"] },
  { label: "Advanced", sections: ["server", "import", "about"] },
];
export const SETTINGS_ICONS = { "providers": KeyRound, "agents": Bot, "connectors": Plug, "composer": SquarePen, "appearance": Palette, "display": Monitor, "shortcuts": Keyboard, "dictation": Mic, "git": GitBranch, "open-apps": AppWindow, "projects": Folder, "environments": Box, "browser": Globe, "computer": MousePointer2, "server": Server, "import": Download, "about": Info };
export function SettingsNav({ activeSection, onSelectSection, showDisclosure = false }: { activeSection: CodexSection; onSelectSection: (section: CodexSection) => void; showDisclosure?: boolean }) {
  return <aside className="settings-sidebar"><PanelNavList className="settings-nav-list">{SETTINGS_GROUPS.map(group => <Fragment key={group.label}><h2 className="settings-nav-group">{group.label}</h2>{group.sections.map(section => { const Icon = SETTINGS_ICONS[section as keyof typeof SETTINGS_ICONS]; return <PanelNavItem key={section} className="settings-nav" title={SETTINGS_SECTION_LABELS[section]} icon={<Icon aria-hidden />} active={activeSection === section} aria-current={activeSection === section ? "page" : undefined} showDisclosure={showDisclosure} onClick={() => onSelectSection(section)}>{SETTINGS_SECTION_LABELS[section]}</PanelNavItem>; })}</Fragment>)}</PanelNavList></aside>;
}
