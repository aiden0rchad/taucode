import { useState } from "react";
import type { AppSettings } from "@/types";
import { features, type Features } from "@settings/features";
import { SettingsPage, SettingsGroup, SettingsToggleRow, SettingsToggleSwitch } from "@/features/design-system/components/settings/SettingsPrimitives";

export const featureGroups: [string, [keyof Features, string, string, boolean?][]][] = [
  ["Developer", [["diagnostics", "Diagnostics", "Record issues, performance and improvement signals locally for agents to review"]]],
  ["Workspace", [["git", "Git", "Changes, branches, diffs and pull requests. Turning off also disables worktrees."], ["worktrees", "Worktrees & clones", "Create isolated workspaces and clone agents."], ["terminal", "Terminal", "Integrated terminal sessions."], ["checkpoints", "Checkpoints & rewind", "Save and restore files independently of Git."]]],
  ["Agents", [["subagents", "Subagents tree", "Delegate to child agents and display their progress.", true], ["sideChats", "Side chats", "Fork conversations alongside the parent thread."], ["connectors", "Connectors (MCP)", "Connect external tools to engines.", true]]],
  ["Tools for the agent", [["browser", "Browser", "Browse pages with the integrated browser.", true], ["computerUse", "Computer use", "Allow agents to operate desktop apps.", true]]],
  ["Modes", [["chatMode", "Chat", "Conversational mode. Code always remains available."], ["workMode", "Work", "Document workflows. Code always remains available."]]],
  ["Extras", [["importTools", "Import from Claude/Codex", "Import conversations and configuration."], ["dictation", "Dictation", "Speak into the composer."], ["reasoningEffects", "Reasoning effects", "Heat, particles and animation on the reasoning slider."]]],
];
export function SettingsTaucodeFeaturesSection({ appSettings, onUpdateAppSettings }: { appSettings: AppSettings; onUpdateAppSettings: (settings: AppSettings) => Promise<unknown> }) {
  const value = features(appSettings);
  const [error, setError] = useState("");
  const toggle = async (key: keyof Features) => {
    try { await onUpdateAppSettings({ ...appSettings, features: { ...value, [key]: !value[key], ...(key === "git" && value.git ? { worktrees: false } : {}) } }); setError(""); }
    catch (e) { setError(String(e)); }
  };
  return <SettingsPage title="Features" subtitle="Choose what appears and runs in taucode. Code is always available.">
    {featureGroups.map(([group, rows]) => <SettingsGroup key={group} title={group}>{rows.map(([key, title, description, session]) => <SettingsToggleRow key={key} title={title} subtitle={<>{description}{session && <small> Applies to new sessions</small>}</>}><SettingsToggleSwitch pressed={value[key]} disabled={key === "worktrees" && !value.git} onClick={() => void toggle(key)} /></SettingsToggleRow>)}</SettingsGroup>)}
    {error && <p role="alert">{error}</p>}
  </SettingsPage>;
}
