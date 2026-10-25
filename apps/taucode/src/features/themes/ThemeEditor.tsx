import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { RotateCcw, Search } from 'lucide-react';
import { ModalShell } from '@/features/design-system/components/modal/ModalShell';
import { Squircle } from '@/features/design-system/components/squircle/Squircle';
import { SettingsButton, SettingsTextField, SettingsTextArea, SettingsSelect, SettingsSegmented, SettingsRow, SettingsGroup, SettingsToggleRow, SettingsToggleSwitch } from '@/features/design-system/components/settings/SettingsPrimitives';
import { applyTheme } from './applyTheme';
import { builtins, resolveTheme, themeTokenOverrides } from './builtins';
import { themeTokens } from './tokens';
import { validateTheme, type ThemeManifest } from './validate';
import { rgba } from './contrast';
import { ThemePreview } from './ThemePreview';

export function ThemeDialog({ title, close, children, editor = false }: { title: string; close: () => void; children: ReactNode; editor?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('input,textarea,button,select')?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopImmediatePropagation(); e.preventDefault(); closeRef.current(); }
      if (e.key !== 'Tab') return;
      const all = [...(ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),textarea,select,summary,[tabindex="0"]') ?? [])].filter(e => !e.closest('[hidden]'));
      const first = all[0], last = all[all.length-1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    };
    document.addEventListener('keydown', key, true);
    return () => { document.removeEventListener('keydown',key,true); previous?.focus(); };
  }, []);
  return <ModalShell ariaLabel={title} squircle={editor} cardClassName={editor ? 'theme-editor-card squircle-clip' : undefined} className={'theme-dialog'+(editor ? ' theme-editor' : '')} onBackdropClick={close}><div ref={ref} className={editor ? 'theme-editor-content settings-page' : undefined}>{!editor && <h2>{title}</h2>}{children}</div></ModalShell>;
}
const stacks = { System: 'system-ui, sans-serif', Mono: 'ui-monospace, "SF Mono", "Menlo", monospace', Serif: 'ui-serif, "New York", "Iowan Old Style", "Georgia", serif' };
const descriptions: Record<string,string> = { Surfaces:'Background fill for this surface.', Text:'Foreground color for this text style.', Accent:'Emphasis for links and selected controls.', Borders:'Outline separating surfaces and controls.', Status:'Color communicating a status.', Tree:'Line connecting items in the project tree.', Code:'Color used in code content.', Terminal:'Color used in terminal output.', Radii:'Corner rounding for this shape.' };
export function ThemeEditor({ initial, restore, save, close }: { initial: ThemeManifest; restore: () => void; save: (theme: ThemeManifest) => Promise<void>; close: () => void }) {
  const [draft,setDraft] = useState(initial), [search,setSearch] = useState(''), [error,setError] = useState(''), [saving,setSaving] = useState(false);
  const [customFonts,setCustomFonts] = useState<Record<string,boolean>>({});
  const errors = validateTheme(draft), resolved = resolveTheme(draft), base = resolveTheme({ ...draft,tokens:{} });
  const preview = useRef(initial);
  if (!errors.length) preview.current = draft;
  const committed = useRef(false);
  const restoreRef = useRef(restore);
  restoreRef.current = restore;
  useEffect(() => () => { if (!committed.current) restoreRef.current(); }, []);
  useEffect(() => { if (!validateTheme(draft).length) applyTheme(draft); }, [draft]);
  const patch = (next: Partial<ThemeManifest>) => setDraft(t => ({ ...t, ...next }));
  const token = (key: string, value: string) => setDraft(t => {
    const tokens = { ...t.tokens };
    if (value && value !== base.tokens[key]) tokens[key] = value; else delete tokens[key];
    return { ...t,tokens };
  });
  const commit = async () => {
    setSaving(true);
    try { await save({ ...draft,tokens:themeTokenOverrides(draft) }); committed.current = true; close(); } catch (e) { setError(String(e)); } finally { setSaving(false); }
  };
  const cancel = () => { if (!saving) close(); };
  const title = 'Edit theme — '+(draft.name || 'Untitled');
  return <ThemeDialog title={title} close={cancel} editor>
    <header className="theme-editor-header"><div><h2>{title}</h2><p>Changes preview live. Cancel restores your selected theme.</p></div><label className="theme-token-search"><Search size={16} aria-hidden /><SettingsTextField aria-label="Search tokens" placeholder="Search tokens" value={search} onChange={e => setSearch(e.target.value)} /></label></header>
    <div className="theme-editor-body">
      <div className="theme-editor-controls">
        <SettingsGroup title="Details">
          <SettingsRow><label htmlFor="theme-name">Name</label><SettingsTextField id="theme-name" value={draft.name} onChange={e => patch({name:e.target.value})} /></SettingsRow>
          <SettingsRow><label htmlFor="theme-author">Author</label><SettingsTextField id="theme-author" value={draft.author ?? ''} onChange={e => patch({author:e.target.value})} /></SettingsRow>
          <SettingsRow><label htmlFor="theme-description">Description</label><SettingsTextArea id="theme-description" rows={2} value={draft.description ?? ''} onChange={e => patch({description:e.target.value})} /></SettingsRow>
          <SettingsRow><span>Appearance</span><SettingsSegmented label="Appearance" value={draft.appearance} options={[{value:'light',label:'Light'},{value:'dark',label:'Dark'}]} onChange={value => patch({appearance:value as 'light'|'dark'})} /></SettingsRow>
          <SettingsRow><label htmlFor="theme-extends">Based on</label><SettingsSelect id="theme-extends" value={draft.extends ?? draft.appearance} onChange={e => patch({ extends:e.target.value })}>{builtins.map(t => <option key={t.id} value={t.id}>{t.name}</option>)}</SettingsSelect></SettingsRow>
        </SettingsGroup>
        <h3 className="theme-colors-title">Colors</h3>
        {[...new Set(Object.values(themeTokens).map(t => t.group))].map(group => {
          const entries = Object.entries(themeTokens).filter(([key,t]) => t.group === group && (key+' '+t.label).includes(search.toLowerCase()));
          if (!entries.length) return null;
          return <SettingsGroup key={group} title={group} collapsible={!search}>{entries.map(([key,t]) => {
            const label = t.label[0].toUpperCase()+t.label.slice(1), color = resolveTheme(preview.current).tokens[key] ?? 'transparent', rgb = rgba(color);
            const swatch = rgb ? '#'+rgb.slice(0,3).map(c => Math.max(0,Math.min(255,Math.round(c))).toString(16).padStart(2,'0')).join('') : '#000000';
            const overridden = draft.tokens[key] !== undefined && draft.tokens[key] !== base.tokens[key];
            return <SettingsRow key={key} className="theme-token-row"><div className="theme-token-label"><label htmlFor={'token-'+key}>{label}</label><small title={descriptions[group]}>{descriptions[group]}</small></div><div className="theme-token-controls">
              {t.group !== 'Radii' && <Squircle className="theme-color-swatch squircle-clip" style={{'--radius-window':'6px','--theme-swatch-color':color} as CSSProperties}><input type="color" aria-label={label+' color'} value={swatch} onChange={e => token(key,e.target.value)} /></Squircle>}
              <SettingsTextField id={'token-'+key} className="theme-hex" value={draft.tokens[key] ?? ''} placeholder={base.tokens[key]} onChange={e => token(key,e.target.value)} spellCheck={false} />
              {overridden && <SettingsButton className="theme-token-reset" aria-label={'Reset '+label} title={'Reset '+label} onClick={() => token(key,'')}><RotateCcw size={14} aria-hidden /></SettingsButton>}
            </div></SettingsRow>;
          })}</SettingsGroup>;
        })}
        <SettingsGroup title="Fonts">
          {(['ui','mono','prose'] as const).map(key => {
            const label = {ui:'UI',mono:'Code',prose:'Prose'}[key];
            const preset = customFonts[key] ? 'Custom' : draft.fonts?.[key] === undefined ? 'Inherited' : Object.entries(stacks).find(([,v]) => v === draft.fonts?.[key])?.[0] ?? 'Custom';
            return <SettingsRow key={key} className="theme-font-row"><label htmlFor={'font-preset-'+key}>{label}</label><div>
              <SettingsSelect id={'font-preset-'+key} value={preset} onChange={e => {
                const value = e.target.value; setCustomFonts(v => ({...v,[key]:value === 'Custom'}));
                const fonts = {...draft.fonts};
                if (value === 'Inherited') delete fonts[key]; else if (value !== 'Custom') fonts[key] = stacks[value as keyof typeof stacks];
                patch({fonts});
              }}><option>Inherited</option>{Object.keys(stacks).map(k => <option key={k}>{k}</option>)}<option>Custom</option></SettingsSelect>
              {preset === 'Custom' && <SettingsTextField aria-label={label+' font stack'} placeholder="Quoted local names, serif" value={draft.fonts?.[key] ?? ''} onChange={e => patch({fonts:{...draft.fonts,[key]:e.target.value}})} />}
              <span className="theme-font-sample" style={{fontFamily:resolveTheme(preview.current).fonts?.[key] ?? (key === 'mono' ? 'monospace' : 'system-ui')}}>Aa Bb 0123 — The quick brown fox</span>
            </div></SettingsRow>;
          })}
        </SettingsGroup>
        <SettingsGroup title="Window & effects">
          <SettingsRow><div><label htmlFor="theme-material">Material</label><small className="theme-field-note">Window translucency is available on macOS only.</small></div><SettingsSelect id="theme-material" value={resolved.window?.translucent ? resolved.window.material : 'none'} onChange={e => patch({window:{translucent:e.target.value !== 'none',material:e.target.value as NonNullable<ThemeManifest['window']>['material']}})}>{Object.entries({none:'Opaque',sidebar:'Sidebar',underWindow:'Under window',hudWindow:'HUD window'}).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</SettingsSelect></SettingsRow>
          <SettingsToggleRow title="Animated background" subtitle="Digital rain in empty conversations. Respects reduced motion."><SettingsToggleSwitch pressed={!!resolved.effects?.digitalRain} onClick={() => patch({effects:{...resolved.effects,digitalRain:!resolved.effects?.digitalRain}})} /></SettingsToggleRow>
          <SettingsToggleRow title="Glow effects" subtitle="Static accent and focus glows."><SettingsToggleSwitch pressed={!!resolved.effects?.neonGlow} onClick={() => patch({effects:{...resolved.effects,neonGlow:!resolved.effects?.neonGlow}})} /></SettingsToggleRow>
        </SettingsGroup>
        {!!errors.length && <div role="alert" className="theme-editor-errors">{errors.map(e => <p key={e}>{e}</p>)}</div>}
        {error && <p role="alert">{error}</p>}
      </div>
      <aside className="theme-editor-preview"><ThemePreview theme={preview.current} /></aside>
    </div>
    <footer className="theme-editor-footer"><span>Saved locally · Share as a theme file or JSON</span><SettingsButton className="secondary" disabled={saving} onClick={cancel}>Cancel</SettingsButton><SettingsButton className="primary" disabled={!!errors.length || saving} onClick={() => void commit()}>{saving ? 'Saving…' : 'Save theme'}</SettingsButton></footer>
  </ThemeDialog>;
}
