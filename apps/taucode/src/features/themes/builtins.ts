import palettes from './palettes.json';
import { themeTokens } from './tokens';
import type { ThemeManifest } from './validate';
const base = (id: string, appearance: 'light' | 'dark', tokens: Record<string,string>, extra: Partial<ThemeManifest> = {}): ThemeManifest => ({ $schema: 'https://taucode.dev/theme.schema.json', schemaVersion: 1, id, name: id.split('-').map(s => s[0].toUpperCase()+s.slice(1)).join(' '), author: 'taucode', version: '1.0.0', appearance, tokens, window: { translucent: false, material: 'none' }, effects: { digitalRain: false }, ...extra });
const mono = 'ui-monospace, "SF Mono", "Menlo", monospace';
const serif = 'ui-serif, "New York", "Iowan Old Style", "Georgia", serif';
export const builtins: ThemeManifest[] = [
  base('dark', 'dark', palettes.dark), base('light', 'light', palettes.light), base('dim', 'dark', palettes.dim, { extends: 'dark' }),
  ...(['dark','light'] as const).map(appearance => base(`glass-${appearance}`, appearance, {
    '--surface-sidebar': appearance === 'dark' ? 'rgba(20, 20, 24, 0.88)' : 'rgba(248, 248, 250, 0.88)',
    '--surface-topbar': appearance === 'dark' ? 'rgba(20, 20, 24, 0.88)' : 'rgba(248, 248, 250, 0.88)',
    '--text-muted': appearance === 'dark' ? '#bfc1c9' : '#555863',
  }, { extends: appearance, window: { translucent: true, material: 'sidebar' } })),
  base('matrix', 'dark', {
    ...Object.fromEntries(Object.keys(themeTokens).filter(k => k.startsWith('--surface-') && !/text|border|status/.test(k)).map(k => [k, '#030803'])),
    '--text-primary': '#d1ffdb', '--text-stronger': '#b8ffc9', '--text-strong': '#e4ffeb', '--text-muted': '#a0d5ad', '--text-subtle': '#8fca9b',
    '--text-accent': '#00ff41', '--message-link-color': '#00ff41', '--border-accent': '#00ff41', '--tree-line': '#276238', '--tree-line-active': '#00ff41', '--surface-active': '#12391d', '--surface-control-hover': '#16351d',
  }, { extends: 'dark', fonts: { ui: mono, mono, prose: mono }, effects: { digitalRain: true } }),
  ...(['light','dark'] as const).map(appearance => {
    const light = appearance === 'light';
    return base(`claude-${appearance}`, appearance, {
      '--surface-app': light ? '#faf9f5' : '#262624', '--surface-messages': light ? '#faf9f5' : '#262624', '--surface-topbar': light ? '#faf9f5' : '#262624',
      '--surface-sidebar': light ? '#f5f4ed' : '#1f1e1d', '--surface-sidebar-opaque': light ? '#f5f4ed' : '#1f1e1d',
      ...Object.fromEntries(['card','composer','popover','bubble-user','card-muted','right-panel'].map(k => [`--surface-${k}`, light ? '#ffffff' : '#30302e'])),
      '--surface-card-strong': light ? '#f0eee6' : '#3e3e38', '--surface-hover': light ? '#f0eee6' : '#3e3e38', '--surface-active': light ? '#f0eee6' : '#3e3e38',
      '--text-primary': light ? '#141413' : '#f5f4ef', '--text-strong': light ? '#141413' : '#f5f4ef', '--text-muted': light ? '#5e5d59' : '#b7b5a9', '--text-subtle': light ? '#5e5d59' : '#b7b5a9',
      '--border-subtle': light ? '#e8e6dc' : '#3e3e38', '--border-accent': light ? '#c96442' : '#d97757', '--text-accent': light ? '#a7482b' : '#d97757', '--message-link-color': light ? '#a7482b' : '#d97757', '--surface-text-accent': light ? '#c96442' : '#d97757',
    }, { extends: appearance, fonts: { prose: serif } });
  }),
];
export function resolveTheme(theme: ThemeManifest): ThemeManifest {
  const parent = theme.extends ? builtins.find(t => t.id === theme.extends) : theme.id === 'dark' ? undefined : builtins.find(t => t.id === (theme.id === 'light' ? 'dark' : theme.appearance));
  const inherited = parent ? resolveTheme(parent) : undefined;
  return { ...inherited, ...theme, tokens: { ...inherited?.tokens, ...theme.tokens }, fonts: { ...inherited?.fonts, ...theme.fonts } };
}
export function themeTokenOverrides(theme: ThemeManifest) {
  const base = resolveTheme({ ...theme,tokens:{} }).tokens;
  return Object.fromEntries(Object.entries(theme.tokens).filter(([key,value]) => value !== base[key]));
}
export function selectTheme(id: string, themes: ThemeManifest[], dark: boolean, lightId = 'light', darkId = 'dark') {
  const system = () => themes.find(t => t.id === (dark ? darkId : lightId) && t.appearance === (dark ? 'dark' : 'light')) ?? builtins.find(t => t.id === (dark ? 'dark' : 'light'))!;
  return id === 'system' ? system() : themes.find(t => t.id === id) ?? system();
}
