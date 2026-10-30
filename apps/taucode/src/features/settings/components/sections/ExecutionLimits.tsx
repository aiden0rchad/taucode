import type { AgentPolicy } from "@/types";
import { SettingsFieldRow, SettingsGroup, SettingsNumberStepper } from "@/features/design-system/components/settings/SettingsPrimitives";
import "./executionLimits.css";

export function ExecutionLimits({ policy, onChange }: { policy: AgentPolicy; onChange: (policy: AgentPolicy) => void }) {
  const sub = policy.subagents;
  return <SettingsGroup className="execution-limits" title="Run budgets" subtitle="Applies to new Chain of Command runs in this project. Set a limit to 0 to turn it off.">
    <SettingsFieldRow title="Run time (minutes)" subtitle="Stops the commander, its agents, and active tools when time runs out."><SettingsNumberStepper min={0} max={1440} value={sub.maxRunMinutes ?? 0} onChange={event => onChange({ ...policy, subagents: { ...sub, maxRunMinutes: Number(event.target.value) } })} /></SettingsFieldRow>
    <SettingsFieldRow title="Daily team tokens" subtitle="Shared across conversations and Pi/OMP agents. Usage survives restarts and resets at 00:00 UTC."><SettingsNumberStepper min={0} max={1_000_000_000} step={1000} value={sub.dailyTokenLimit ?? 0} onChange={event => onChange({ ...policy, subagents: { ...sub, dailyTokenLimit: Number(event.target.value) } })} /></SettingsFieldRow>
    <p>Stops when reported usage reaches the allowance. Requests already running can exceed it. Missing usage stops a capped run.</p>
  </SettingsGroup>;
}
