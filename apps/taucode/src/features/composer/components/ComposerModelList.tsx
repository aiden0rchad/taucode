import { useState } from "react";
import { Check } from "lucide-react";

export type ComposerModel = { id: string; name: string; provider?: string; disabled?: boolean; disabledReason?: string };

export function ComposerModelList({ models, selectedId, defaultId, disabled, onSelect }: {
  models: ComposerModel[]; selectedId?: string | null; defaultId?: string | null; disabled?: boolean;
  onSelect: (id: string) => Promise<void>;
}) {
  const [search, setSearch] = useState("");
  const filtered = models.filter(model => `${model.name} ${model.provider ?? ""}`.toLowerCase().includes(search.toLowerCase()));
  const providers = [...new Set(models.map(model => model.provider ?? "Models"))];
  const recommended = models.find(model => model.id === defaultId && !model.disabled);
  return <div className="composer-model-list" onKeyDown={event => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || (event.target instanceof HTMLInputElement && ["Home", "End"].includes(event.key))) return;
    const options = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const index = options.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? options.length - 1 : event.key === "ArrowDown" ? (index + 1) % options.length : (index - 1 + options.length) % options.length;
    event.preventDefault(); options[next]?.focus();
  }}>
    {models.length > 8 && <input type="search" aria-label="Search models" placeholder="Search models" value={search} onChange={event => setSearch(event.target.value)} />}
    <div className="composer-model-options" role="listbox" aria-label="Models">
      {!search && <button type="button" role="option" aria-selected={false} disabled={disabled || !recommended} onClick={() => { if (recommended) void onSelect(recommended.id); }}><span>Default<small>Recommended for this engine</small></span></button>}
      {providers.map(provider => {
        const entries = filtered.filter(model => (model.provider ?? "Models") === provider);
        return entries.length > 0 && <div key={provider} role="group" aria-label={provider}>
          {providers.length > 1 && <div className="composer-model-provider">{provider}</div>}
          {entries.map(model => <button type="button" role="option" aria-selected={model.id === selectedId} key={model.id} title={model.disabledReason} disabled={disabled || model.disabled} onClick={() => { void onSelect(model.id); }}><span>{model.name}</span>{model.id === selectedId && <Check size={14} aria-hidden />}</button>)}
        </div>;
      })}
      {!filtered.length && <p>No models found.</p>}
    </div>
  </div>;
}
