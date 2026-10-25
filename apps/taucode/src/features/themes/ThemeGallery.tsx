import { useEffect, useRef, useState, type RefObject } from 'react';
import type { AppSettings } from '@/types';
import { SettingsPage, SettingsGroup, SettingsRow, SettingsSelect, SettingsButton, SettingsTextArea, SettingsToggleRow, SettingsToggleSwitch } from '@/features/design-system/components/settings/SettingsPrimitives';
import { themesSave, themesDelete, themesImport, themesExport, themesRevealFile, themesOpenFolder, pickThemeFile, pickThemeExport } from '@services/tauri';
import { builtins, resolveTheme, selectTheme, themeTokenOverrides } from './builtins';
import { useThemes, reloadThemes } from './store';
import { applyTheme } from './applyTheme';
import { builtinIds, parseTheme, type ThemeManifest } from './validate';
import { ThemeDialog, ThemeEditor } from './ThemeEditor';
import { Plus, Upload, ClipboardPaste, FolderOpen, MoreHorizontal, Check, Copy, Pencil, Download, Clipboard, Trash2, AlertCircle } from 'lucide-react';
import { AnchoredPopover } from '@/features/design-system/components/popover/AnchoredPopover';
import { MenuTrigger, PopoverMenuItem } from '@/features/design-system/components/popover/PopoverPrimitives';
import { ThemePreview } from './ThemePreview';
import './themes.css';

export function ThemeGallery({ settings, update }: { settings: AppSettings; update: (next: AppSettings) => Promise<void> }) {
  const { files } = useThemes();
  const themes = [...builtins,...files.flatMap(f => f.manifest ? [f.manifest] : [])];
  const [systemDark,setSystemDark] = useState(() => window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
  const [focused,setFocused] = useState(settings.theme);
  const [menu,setMenu] = useState<ThemeManifest|null>(null);
  const anchor = useRef<HTMLButtonElement|null>(null);
  const grid = useRef<HTMLDivElement|null>(null);
  const [editorReplace,setEditorReplace] = useState(false);
  useEffect(() => {
    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    const change = () => setSystemDark(media?.matches ?? false);
    media?.addEventListener('change',change);
    return () => media?.removeEventListener('change',change);
  }, []);
  const [editor,setEditor] = useState<ThemeManifest|null>(null), [paste,setPaste] = useState<string|null>(null), [error,setError] = useState('');
  const [remove,setRemove] = useState<ThemeManifest|null>(null), [conflict,setConflict] = useState<{ manifest: ThemeManifest; path?: string }|null>(null);
  const active = selectTheme(settings.theme,themes,systemDark,settings.themeLight,settings.themeDark);
  const restore = () => applyTheme(active,settings.themeAnimations?.[active.id] !== false);
  const run = async (fn: () => Promise<unknown>) => { setError(''); try { await fn(); } catch (e) { setError(String(e)); } };
  const unique = (id: string) => {
    for (let n=2;;n++) { const suffix = '-'+n, next = id.slice(0,40-suffix.length)+suffix; if (!themes.some(t => t.id === next)) return next; }
  };
  const duplicate = (t: ThemeManifest) => {
    setEditorReplace(false);
    const copy = { ...t, id:unique(t.id), name:t.name+' copy', extends:builtinIds.includes(t.id) ? t.id : t.extends };
    setEditor({ ...copy,tokens:themeTokenOverrides(copy) });
  };
  const save = async (t: ThemeManifest, replace = false) => { await themesSave(t,replace); await reloadThemes(); await update({ ...settings,theme:t.id }); applyTheme(t,settings.themeAnimations?.[t.id] !== false); };
  let parsed: ThemeManifest|null = null, pasteError = '';
  if (paste !== null) { try { parsed = parseTheme(paste); } catch (e) { pasteError = String(e); } }
  const acceptPaste = async () => {
    if (!parsed) return;
    if (themes.some(t => t.id === parsed!.id) || parsed.id === 'system') { setConflict({ manifest:parsed }); return; }
    await save(parsed); setPaste(null);
  };
  const importFile = async () => {
    const path = await pickThemeFile(); if (!path) return;
    const result = await themesImport(path);
    if (result.conflict) setConflict({ manifest:result.manifest,path });
    else { await reloadThemes(); await update({ ...settings,theme:result.manifest.id }); }
  };
  const resolveConflict = async (resolution: 'replace'|'keepBoth') => {
    if (!conflict) return;
    if (conflict.path) {
      const result = await themesImport(conflict.path,resolution);
      await reloadThemes(); await update({ ...settings,theme:result.manifest.id });
    } else await save(resolution === 'keepBoth' ? { ...conflict.manifest,id:unique(conflict.manifest.id) } : conflict.manifest,resolution === 'replace');
    setConflict(null); setPaste(null);
  };
  const cards = [{ ...selectTheme('system',themes,systemDark,settings.themeLight,settings.themeDark), id:'system', name:'System', author:'Follow your device' }, ...themes];
  const closeMenu = () => { setMenu(null); anchor.current?.focus(); };
  const menuAction = (fn: () => void) => { closeMenu(); fn(); };
  return <SettingsPage title="Appearance" subtitle="Choose a palette, or make one your own." className="theme-page"><SettingsGroup>
    <SettingsRow className="theme-toolbar"><SettingsButton onClick={() => duplicate(active)}><Plus aria-hidden />New theme</SettingsButton><SettingsButton onClick={() => void run(importFile)}><Upload aria-hidden />Import…</SettingsButton><SettingsButton onClick={() => setPaste('')}><ClipboardPaste aria-hidden />Paste theme</SettingsButton><SettingsButton onClick={() => void run(themesOpenFolder)}><FolderOpen aria-hidden />Open themes folder</SettingsButton></SettingsRow>
    {settings.theme === 'system' && (['light','dark'] as const).map(a => <SettingsRow key={a}><label htmlFor={'system-'+a}>{a === 'light' ? 'Light theme' : 'Dark theme'}</label><SettingsSelect id={'system-'+a} value={(a === 'light' ? settings.themeLight : settings.themeDark) ?? a} onChange={e => void run(() => update({ ...settings,[a === 'light' ? 'themeLight' : 'themeDark']:e.target.value }))}>{themes.filter(t => t.appearance === a).map(t => <option key={t.id} value={t.id}>{t.name}</option>)}</SettingsSelect></SettingsRow>)}
    <div className="theme-gallery" role="group" aria-label="Themes" ref={grid}>
      {cards.map((t,index) => {
        const custom = t.id !== 'system' && !builtinIds.includes(t.id);
        const preview = t.id === 'system' ? selectTheme('system',themes,systemDark,settings.themeLight,settings.themeDark) : t;
        return <article className="theme-card" key={t.id} data-selected={settings.theme === t.id}>
          <button className="theme-card-select" aria-label={'Apply '+t.name} aria-pressed={settings.theme === t.id} tabIndex={t.id === (cards.some(c => c.id === focused) ? focused : 'system') ? 0 : -1} onFocus={() => setFocused(t.id)} onClick={() => void run(() => update({ ...settings,theme:t.id }))} onKeyDown={event => {
            if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(event.key)) return;
            event.preventDefault();
            const columns = grid.current ? getComputedStyle(grid.current).gridTemplateColumns.split(' ').length : 1;
            const delta = event.key === 'ArrowLeft' ? -1 : event.key === 'ArrowRight' ? 1 : event.key === 'ArrowUp' ? -columns : columns;
            const next = event.key === 'Home' ? 0 : event.key === 'End' ? cards.length-1 : (index+delta+cards.length)%cards.length;
            grid.current?.querySelectorAll<HTMLButtonElement>('.theme-card-select')[next]?.focus();
          }}>
            <ThemePreview theme={preview} compact selected={settings.theme === t.id} />
            <span className="theme-card-name"><strong>{t.name}</strong>{custom && <span className="theme-custom-chip">Custom</span>}</span><small>{t.author || 'Unknown author'}</small>
          </button>
          {t.id !== 'system' && <MenuTrigger isOpen={menu?.id === t.id} className="theme-card-menu ghost icon-button" aria-label={'Actions for '+t.name} onClick={event => { anchor.current = event.currentTarget; setMenu(menu?.id === t.id ? null : t); }}><MoreHorizontal size={18} aria-hidden /></MenuTrigger>}
        </article>;
      })}
      {files.filter(f => f.error).map(f => <article className="theme-card theme-error-card" key={f.file}><AlertCircle size={22} aria-hidden /><strong>{f.file}</strong><p role="alert">{f.error}</p><SettingsButton onClick={() => void run(() => themesRevealFile(f.file))}><FolderOpen size={16} aria-hidden />Reveal file</SettingsButton></article>)}
    </div>
    {resolveTheme(active).effects?.digitalRain && <SettingsToggleRow title="Animated background" subtitle="A subtle animation in empty conversations. Respects reduced motion."><SettingsToggleSwitch pressed={settings.themeAnimations?.[active.id] !== false} onClick={() => void run(() => update({ ...settings,themeAnimations:{ ...settings.themeAnimations,[active.id]:settings.themeAnimations?.[active.id] === false } }))} /></SettingsToggleRow>}
    {menu && <ThemeMenu anchor={anchor} close={closeMenu}>
      <PopoverMenuItem role="menuitem" icon={<Check size={16} />} onClick={() => menuAction(() => void run(() => update({ ...settings,theme:menu.id })))}>Apply</PopoverMenuItem>
      <PopoverMenuItem role="menuitem" icon={<Copy size={16} />} onClick={() => menuAction(() => duplicate(menu))}>Duplicate &amp; edit</PopoverMenuItem>
      {!builtinIds.includes(menu.id) && <PopoverMenuItem role="menuitem" icon={<Pencil size={16} />} onClick={() => menuAction(() => { setEditorReplace(true); setEditor(menu); })}>Edit</PopoverMenuItem>}
      <PopoverMenuItem role="menuitem" icon={<Download size={16} />} onClick={() => menuAction(() => void run(async () => { const path = await pickThemeExport(menu.id); if (path) await themesExport(menu,path); }))}>Export…</PopoverMenuItem>
      <PopoverMenuItem role="menuitem" icon={<Clipboard size={16} />} onClick={() => menuAction(() => void run(() => navigator.clipboard.writeText(JSON.stringify(menu,null,2))))}>Copy JSON</PopoverMenuItem>
      {!builtinIds.includes(menu.id) && <PopoverMenuItem role="menuitem" className="theme-menu-danger" icon={<Trash2 size={16} />} onClick={() => menuAction(() => setRemove(menu))}>Delete</PopoverMenuItem>}
    </ThemeMenu>}
    {error && <p role="alert">{error}</p>}
    {editor && <ThemeEditor initial={editor} restore={restore} close={() => setEditor(null)} save={t => save(t,editorReplace)} />}
    {paste !== null && !conflict && <ThemeDialog title="Paste theme" close={() => setPaste(null)}><SettingsTextArea aria-label="Theme JSON" value={paste} onChange={e => setPaste(e.target.value)} rows={12} />{pasteError && <p role="alert">{pasteError}</p>}<div className="theme-actions"><SettingsButton onClick={() => setPaste(null)}>Cancel</SettingsButton><SettingsButton disabled={!parsed} onClick={() => void run(acceptPaste)}>Import theme</SettingsButton></div></ThemeDialog>}
    {conflict && <ThemeDialog title="Theme id already exists" close={() => setConflict(null)}><p>Replace “{conflict.manifest.id}” or keep both themes?</p><div className="theme-actions"><SettingsButton onClick={() => setConflict(null)}>Cancel</SettingsButton><SettingsButton onClick={() => void run(() => resolveConflict('keepBoth'))}>Keep both</SettingsButton>{!builtinIds.includes(conflict.manifest.id) && conflict.manifest.id !== 'system' && <SettingsButton onClick={() => void run(() => resolveConflict('replace'))}>Replace</SettingsButton>}</div>{error && <p role="alert">{error}</p>}</ThemeDialog>}
    {remove && <ThemeDialog title="Delete theme" close={() => setRemove(null)}><p>Delete “{remove.name}”?</p><div className="theme-actions"><SettingsButton onClick={() => setRemove(null)}>Cancel</SettingsButton><SettingsButton onClick={() => void run(async () => { await themesDelete(remove.id); await reloadThemes(); setRemove(null); })}>Delete theme</SettingsButton></div>{error && <p role="alert">{error}</p>}</ThemeDialog>}
  </SettingsGroup></SettingsPage>;
}


function ThemeMenu({ anchor, close, children }: { anchor: RefObject<HTMLButtonElement|null>; close: () => void; children: React.ReactNode }) {
  const surface = useRef<HTMLDivElement|null>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    surface.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const outside = (e: MouseEvent) => { if (!surface.current?.contains(e.target as Node) && !anchor.current?.contains(e.target as Node)) closeRef.current(); };
    const escape = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.stopImmediatePropagation(); e.preventDefault(); closeRef.current(); } };
    document.addEventListener('mousedown',outside);
    document.addEventListener('keydown',escape,true);
    return () => { document.removeEventListener('mousedown',outside); document.removeEventListener('keydown',escape,true); };
  }, [anchor]);
  return <AnchoredPopover anchorRef={anchor} surfaceRef={surface} className="theme-menu" role="menu" aria-label="Theme actions" onKeyDown={event => {
    if (event.key === 'Tab') { close(); return; }
    if (!['ArrowUp','ArrowDown','Home','End'].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    const buttons = [...(surface.current?.querySelectorAll<HTMLButtonElement>('button') ?? [])], index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length-1 : (index+(event.key === 'ArrowUp' ? -1 : 1)+buttons.length)%buttons.length]?.focus();
  }}>{children}</AnchoredPopover>;
}
