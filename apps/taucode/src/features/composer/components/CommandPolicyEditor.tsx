import { useEffect, useId, useRef, useState } from "react";
import { BookOpen, ChevronRight, Code2, GitBranch, Palette, Plus, Search, Server, ShieldCheck, Star, Wrench, X } from "lucide-react";
import type { AgentPolicy, AgentRole, ProviderEntry } from "@/types";
import { commandRolePresets, commandRoleTitle, isCommandRole, modelKey, modelRef } from "@settings/registry";
import { SettingsButton, SettingsFieldRow, SettingsNumberStepper, SettingsSegmented, SettingsSelect, SettingsTextArea, SettingsTextField } from "@/features/design-system/components/settings/SettingsPrimitives";
import { ExecutionLimits } from "@settings/components/sections/ExecutionLimits";

const ranks = [
  { name: "colonel", title: "Colonel", job: "Delegation", description: "Turns the goal into focused assignments.", Icon: GitBranch },
  { name: "major", title: "Major", job: "Quality assurance", description: "Checks the work against your requirements.", Icon: ShieldCheck },
  { name: "enlisted", title: "Enlisted", job: "Implementation", description: "Builds, tests, and reports the result.", Icon: Wrench },
];
const presetIcons: Record<string, typeof Search> = { researcher: Search, designer: Palette, "frontend-engineer": Code2, "backend-engineer": Server, "test-engineer": ShieldCheck, documentation: BookOpen };
function roleInfo(role: AgentRole) {
  return ranks.find(rank => rank.name === role.name) ?? { name: role.commandRank, title: commandRoleTitle(role.name), job: role.description, description: `${commandRoleTitle(role.commandRank ?? "enlisted")} rank · ${role.toolAccess === "read-only" ? "Read only" : "Full tool access"}`, Icon: presetIcons[role.name.replace(/-\d+$/, "")] ?? Wrench };
}
const limits = [
  { field: "maxConcurrent", label: "Max concurrent", max: 16, hint: "Active models across the team" },
  { field: "maxPerTurn", label: "Max per turn", max: 32, hint: "Total agents across the team" },
  { field: "maxDepth", label: "Max depth", max: 3, hint: "Levels below the commander" },
] as const;

export default function CommandPolicyEditor({ policy, providers, tab, onChange }: {
  policy: AgentPolicy; providers: ProviderEntry[]; tab: string; onChange: (policy: AgentPolicy) => void;
}) {
  const [selectedRole, setSelectedRole] = useState("colonel");
  const [showPresets, setShowPresets] = useState(false);
  const presetId = useId();
  const team = useRef<HTMLDivElement>(null);
  const focusRole = useRef<string | null>(null);
  const sub = policy.subagents;
  const teamRoles = sub.roles.filter(isCommandRole);
  useEffect(() => {
    if (!focusRole.current || tab !== "defaults") return;
    const target = team.current?.querySelector<HTMLElement>(focusRole.current === "add" ? ".command-add-role" : `[data-role="${focusRole.current}"] select`);
    target?.focus();
    target?.scrollIntoView?.({ block: "nearest" });
    focusRole.current = null;
  }, [sub.roles, tab]);
  const models = providers.filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ value: `${p.id}/${m.id}`, label: `${p.name} / ${m.displayName || m.id}` })));
  const change = (patch: Partial<typeof sub>) => onChange({ ...policy, subagents: { ...sub, ...patch } });
  const changeRole = (name: string, patch: Partial<typeof sub.roles[number]>) => change({ roles: sub.roles.map(role => role.name === name ? { ...role, ...patch } : role) });
  const options = models.map(model => <option key={model.value} value={model.value}>{model.label}</option>);
  const modelSelect = (label: string, value: string, select: (value: string) => void) => <SettingsSelect aria-label={label} value={value} onChange={event => select(event.target.value)}><option value="">Inherit</option>{options}</SettingsSelect>;
  const addRole = (preset: AgentRole) => {
    let name = preset.name;
    for (let n = 2; sub.roles.some(role => role.name === name); n++) name = `${preset.name}-${n}`;
    focusRole.current = name;
    change({ roles: [...sub.roles, { ...preset, name }] });
    setSelectedRole(name);
    setShowPresets(false);
  };

  if (tab === "defaults") return <div className="command-team" ref={team}>
    <div className="command-commander">
      <span className="command-rank-icon"><Star size={20} aria-hidden /></span>
      <div><h3>Commander</h3><p>Sets the goal and brings the work together.</p></div>
      <span className="command-model-source">Your conversation model</span>
    </div>
    <div className="command-team-toolbar"><span>Team roles <small>{teamRoles.length}</small></span><SettingsButton className="command-add-role" aria-expanded={showPresets} aria-controls={presetId} onClick={() => setShowPresets(!showPresets)}><Plus size={14} aria-hidden />Add role</SettingsButton></div>
    {showPresets && <div className="command-presets" id={presetId} role="group" aria-label="Role presets">
      <p>Start with a role. Choose its model, then fine-tune its instructions in Roles.</p>
      <div>{commandRolePresets.map(preset => {
        const Icon = presetIcons[preset.name];
        return <SettingsButton key={preset.name} className="command-preset" onClick={() => addRole(preset)}><Icon size={18} aria-hidden /><span><strong>{commandRoleTitle(preset.name)}</strong>{" "}<small>{preset.description}</small></span><Plus size={13} aria-hidden /></SettingsButton>;
      })}</div>
    </div>}
    <div className="command-team-list">
      {teamRoles.map(role => {
        const { name, title, job, Icon } = roleInfo(role);
        const removable = !ranks.some(rank => rank.name === role.name);
        return <div key={role.name} className="command-role-card" data-rank={name} data-role={role.name}>
          <span className="command-rank-icon"><Icon size={18} aria-hidden /></span>
          <div className="command-role-copy"><h3>{title}{" "}<span>{job}</span></h3></div>
          <div className="command-role-model">{modelSelect(`${title} model`, modelKey(role.model), value => changeRole(role.name, { model: modelRef(value) }))}
            {removable && <SettingsButton className="command-remove-role" aria-label={`Remove ${title}`} title={`Remove ${title}`} onClick={() => { focusRole.current = "add"; change({ roles: sub.roles.filter(r => r.name !== role.name) }); if (selectedRole === role.name) setSelectedRole("colonel"); }}><X size={14} aria-hidden /></SettingsButton>}
          </div>
        </div>;
      })}
    </div>
    <p className="command-hint">Mix cloud and local models. Inherit uses the default subagent model, then the parent’s model.</p>
  </div>;

  if (tab === "subagents") return <div className="command-limits">
    <div className="command-section-heading"><h3>Keep the team focused</h3><p>Set boundaries for how much work the team can start. Coordinators waiting for their team share their slots with workers.</p></div>
    <div className="command-limit-grid">{limits.map(({ field, label, max, hint }) => <div className="command-limit-card" key={field}>
      <SettingsFieldRow title={label}><SettingsNumberStepper min={1} max={max} value={sub[field]} onChange={event => change({ [field]: Number(event.target.value) })} /></SettingsFieldRow>
      <p>{hint}</p>
    </div>)}</div>
    <ExecutionLimits policy={policy} onChange={onChange} />
    <details className="command-restrictions">
      <summary><ChevronRight size={16} aria-hidden /><span>Model defaults & restrictions</span><small>{sub.allowedModels.length ? `${sub.allowedModels.length} allowed` : "All enabled models"}</small></summary>
      <div>
        <SettingsFieldRow title="Default subagent model" subtitle="Used by roles set to Inherit.">{modelSelect("Default subagent model", modelKey(sub.defaultModel), value => change({ defaultModel: modelRef(value) }))}</SettingsFieldRow>
        <SettingsFieldRow title="Allowed models" subtitle="Leave empty to allow any enabled model." className="settings-field-row-long"><SettingsSelect multiple value={sub.allowedModels.map(modelKey)} onChange={event => change({ allowedModels: Array.from(event.target.selectedOptions).flatMap(option => { const model = modelRef(option.value); return model ? [model] : []; }) })}>{options}</SettingsSelect></SettingsFieldRow>
      </div>
    </details>
  </div>;

  const role = teamRoles.find(role => role.name === selectedRole) ?? teamRoles[0];
  if (!role) return null;
  const rank = roleInfo(role);
  return <div className="command-roles">
    {teamRoles.length <= 3 ? <SettingsSegmented tabs label="Agent role" value={role.name} options={teamRoles.map(role => ({ value: role.name, label: commandRoleTitle(role.name) }))} onChange={setSelectedRole} /> : <div className="command-role-selector"><SettingsFieldRow title="Agent role"><SettingsSelect value={role.name} onChange={event => setSelectedRole(event.target.value)}>{teamRoles.map(role => <option key={role.name} value={role.name}>{commandRoleTitle(role.name)}</option>)}</SettingsSelect></SettingsFieldRow></div>}
    {role && <section className="command-role-detail" data-rank={rank.name} aria-label={`${rank.title} role`}>
      <div className="command-role-heading"><span className="command-rank-icon"><rank.Icon size={20} aria-hidden /></span><div><h3>{rank.title}{" "}<span>{rank.job}</span></h3><p>{rank.description}</p></div></div>
      <div className="command-role-fields">
        <SettingsFieldRow title="Description" className="command-wide-field"><SettingsTextField value={role.description} onChange={event => changeRole(role.name, { description: event.target.value })} /></SettingsFieldRow>
        <SettingsFieldRow title={`${rank.title} model`}>{modelSelect(`${rank.title} model`, modelKey(role.model), value => changeRole(role.name, { model: modelRef(value) }))}</SettingsFieldRow>
        <SettingsFieldRow title="Tool access"><SettingsSelect value={role.toolAccess} onChange={event => changeRole(role.name, { toolAccess: event.target.value as "read-only" | "full" })}><option value="read-only">Read only</option><option value="full">Full</option></SettingsSelect></SettingsFieldRow>
        <SettingsFieldRow title="Instructions" className="command-wide-field"><SettingsTextArea rows={3} placeholder="Add guidance, constraints, or a definition of done…" value={role.instructions} onChange={event => changeRole(role.name, { instructions: event.target.value })} /></SettingsFieldRow>
      </div>
    </section>}
  </div>;
}
