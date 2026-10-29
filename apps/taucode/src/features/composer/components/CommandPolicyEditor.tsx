import { useState } from "react";
import { ChevronRight, GitBranch, ShieldCheck, Star, Wrench } from "lucide-react";
import type { AgentPolicy, ProviderEntry } from "@/types";
import { modelKey, modelRef } from "@settings/registry";
import { SettingsFieldRow, SettingsNumberStepper, SettingsSegmented, SettingsSelect, SettingsTextArea, SettingsTextField } from "@/features/design-system/components/settings/SettingsPrimitives";

const ranks = [
  { name: "colonel", title: "Colonel", job: "Delegation", description: "Turns the goal into focused assignments.", Icon: GitBranch },
  { name: "major", title: "Major", job: "Quality assurance", description: "Checks the work against your requirements.", Icon: ShieldCheck },
  { name: "enlisted", title: "Enlisted", job: "Implementation", description: "Builds, tests, and reports the result.", Icon: Wrench },
];
const limits = [
  { field: "maxConcurrent", label: "Max concurrent", max: 16, hint: "Active agents per delegator" },
  { field: "maxPerTurn", label: "Max per turn", max: 32, hint: "Total agents across the team" },
  { field: "maxDepth", label: "Max depth", max: 3, hint: "Levels below the commander" },
] as const;

export default function CommandPolicyEditor({ policy, providers, tab, onChange }: {
  policy: AgentPolicy; providers: ProviderEntry[]; tab: string; onChange: (policy: AgentPolicy) => void;
}) {
  const [selectedRole, setSelectedRole] = useState("colonel");
  const sub = policy.subagents;
  const models = providers.filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ value: `${p.id}/${m.id}`, label: `${p.name} / ${m.displayName || m.id}` })));
  const change = (patch: Partial<typeof sub>) => onChange({ ...policy, subagents: { ...sub, ...patch } });
  const changeRole = (name: string, patch: Partial<typeof sub.roles[number]>) => change({ roles: sub.roles.map(role => role.name === name ? { ...role, ...patch } : role) });
  const options = models.map(model => <option key={model.value} value={model.value}>{model.label}</option>);
  const modelSelect = (label: string, value: string, select: (value: string) => void) => <SettingsSelect aria-label={label} value={value} onChange={event => select(event.target.value)}><option value="">Inherit</option>{options}</SettingsSelect>;

  if (tab === "defaults") return <div className="command-team">
    <div className="command-commander">
      <span className="command-rank-icon"><Star size={20} aria-hidden /></span>
      <div><h3>Commander</h3><p>Sets the goal and brings the work together.</p></div>
      <span className="command-model-source">Your conversation model</span>
    </div>
    <div className="command-team-list">
      {ranks.map(({ name, title, job, Icon }) => {
        const role = sub.roles.find(role => role.name === name);
        return role && <div key={name} className="command-role-card" data-rank={name}>
          <span className="command-rank-icon"><Icon size={18} aria-hidden /></span>
          <div className="command-role-copy"><h3>{title}{" "}<span>{job}</span></h3></div>
          {modelSelect(`${title} model`, modelKey(role.model), value => changeRole(name, { model: modelRef(value) }))}
        </div>;
      })}
    </div>
    <p className="command-hint">Mix cloud and local models. Inherit uses the default subagent model, then the parent’s model.</p>
  </div>;

  if (tab === "subagents") return <div className="command-limits">
    <div className="command-section-heading"><h3>Keep the team focused</h3><p>Set boundaries for how much work the team can start.</p></div>
    <div className="command-limit-grid">{limits.map(({ field, label, max, hint }) => <div className="command-limit-card" key={field}>
      <SettingsFieldRow title={label}><SettingsNumberStepper min={1} max={max} value={sub[field]} onChange={event => change({ [field]: Number(event.target.value) })} /></SettingsFieldRow>
      <p>{hint}</p>
    </div>)}</div>
    <details className="command-restrictions">
      <summary><ChevronRight size={16} aria-hidden /><span>Model defaults & restrictions</span><small>{sub.allowedModels.length ? `${sub.allowedModels.length} allowed` : "All enabled models"}</small></summary>
      <div>
        <SettingsFieldRow title="Default subagent model" subtitle="Used by roles set to Inherit.">{modelSelect("Default subagent model", modelKey(sub.defaultModel), value => change({ defaultModel: modelRef(value) }))}</SettingsFieldRow>
        <SettingsFieldRow title="Allowed models" subtitle="Leave empty to allow any enabled model." className="settings-field-row-long"><SettingsSelect multiple value={sub.allowedModels.map(modelKey)} onChange={event => change({ allowedModels: Array.from(event.target.selectedOptions).flatMap(option => { const model = modelRef(option.value); return model ? [model] : []; }) })}>{options}</SettingsSelect></SettingsFieldRow>
      </div>
    </details>
  </div>;

  const rank = ranks.find(rank => rank.name === selectedRole)!;
  const role = sub.roles.find(role => role.name === selectedRole);
  return <div className="command-roles">
    <SettingsSegmented tabs label="Agent role" value={selectedRole} options={ranks.map(rank => ({ value: rank.name, label: rank.title }))} onChange={setSelectedRole} />
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
