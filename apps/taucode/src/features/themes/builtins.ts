import palettes from './palettes.json';
import { themeTokens } from './tokens';
import type { ThemeManifest } from './validate';
const base = (id: string, appearance: 'light' | 'dark', tokens: Record<string,string>, extra: Partial<ThemeManifest> = {}): ThemeManifest => ({ $schema: 'https://taucode.dev/theme.schema.json', schemaVersion: 1, id, name: id.split('-').map(s => s[0].toUpperCase()+s.slice(1)).join(' '), author: 'taucode', version: '1.0.0', appearance, tokens, window: { translucent: false, material: 'none' }, effects: { digitalRain: false, neonGlow: false }, ...extra });
const mono = 'ui-monospace, "SF Mono", "Menlo", monospace';
const serif = 'ui-serif, "New York", "Iowan Old Style", "Georgia", serif';
export const builtins: ThemeManifest[] = [
  base('dark', 'dark', palettes.dark), base('light', 'light', palettes.light), base('dim', 'dark', palettes.dim, { extends: 'dark' }),
  ...(['dark','light'] as const).map(appearance => base(`glass-${appearance}`, appearance, {
    '--scrollbar-thumb': 'rgba(255,255,255,0.22)', '--scrollbar-thumb-hover': 'rgba(255,255,255,0.32)',
    '--tree-line': appearance === 'dark' ? 'rgba(255,255,255,0.18)' : '#9398a3',
    '--tree-line-active': appearance === 'dark' ? 'rgba(255,255,255,0.55)' : '#566178',
    // The sidebar material already blurs and tints the desktop; keep the web layers light so it shows.
    '--surface-sidebar': appearance === 'dark' ? 'rgba(24, 24, 28, 0.18)' : 'rgba(246, 246, 248, 0.22)',
    '--surface-topbar': appearance === 'dark' ? 'rgba(24, 24, 28, 0.18)' : 'rgba(246, 246, 248, 0.22)',
    '--surface-app': appearance === 'dark' ? 'rgba(24, 24, 28, 0.55)' : 'rgba(246, 246, 248, 0.58)',
    '--surface-messages': appearance === 'dark' ? 'rgba(24, 24, 28, 0.55)' : 'rgba(246, 246, 248, 0.58)',
    '--text-muted': appearance === 'dark' ? '#c4c6ce' : '#555863',
    '--text-subtle': appearance === 'dark' ? '#c4c6ce' : '#555863',
  }, { extends: appearance, window: { translucent: true, material: 'sidebar' }, shape: { corners: 'squircle' }, border: { gradient: appearance === 'dark' ? ['rgba(255,255,255,0.30)', 'rgba(255,255,255,0.06)', 'rgba(255,255,255,0.14)'] : ['rgba(255,255,255,0.95)', 'rgba(0,0,0,0.06)', 'rgba(255,255,255,0.7)'], angle: 160, highlight: true } })),
  base('cyberpunk-neon', 'dark', {
    '--scrollbar-thumb': 'rgba(255,43,214,0.5)', '--scrollbar-thumb-hover': 'rgba(0,229,255,0.5)',
    '--surface-app': '#0b0714', '--surface-messages': '#0b0714',
    '--surface-sidebar': '#0e0a1c', '--surface-sidebar-opaque': '#0e0a1c', '--surface-topbar': '#0e0a1c',
    ...Object.fromEntries(['card','card-muted','composer','popover','right-panel','bubble-user','control','item'].map(k => [`--surface-${k}`, '#160f2b'])),
    '--surface-card-strong': '#1e1538', '--surface-hover': '#24183f', '--surface-control-hover': '#24183f', '--surface-active': '#2d1450',
    '--text-primary': '#ece8ff', '--text-strong': '#ffffff', '--text-stronger': '#ffffff', '--text-muted': '#a89fd0', '--text-subtle': '#a89fd0',
    '--border-accent': '#ff2bd6', '--surface-text-accent': '#ff2bd6', '--tree-line-active': '#ff2bd6',
    '--text-accent': '#00e5ff', '--message-link-color': '#00e5ff', '--tree-line': 'color-mix(in srgb, #a855f7 45%, var(--surface-sidebar))',
    '--status-warning': '#fcee0a', '--status-success': '#3dffa2', '--status-error': '#ff3b6b',
    '--surface-status-warning': '#fcee0a', '--surface-status-success': '#3dffa2', '--surface-status-error': '#ff3b6b',
    '--border-subtle': '#2b2150', '--border-muted': '#2b2150', '--border-strong': '#3d2f6e',
    '--surface-command': '#0f0a1f', '--surface-diff-card': '#0f0a1f', '--shadow-accent': 'rgba(255, 43, 214, 0.45)',
  }, { extends: 'dark', effects: { neonGlow: true }, shape: { corners: 'chamfer', chamfer: 10 }, border: { gradient: ['#ff2bd6', '#a855f7', '#00e5ff'], angle: 135 } }),
  base('matrix', 'dark', {
    '--scrollbar-thumb': 'rgba(0,255,65,0.22)', '--scrollbar-thumb-hover': 'rgba(0,255,65,0.32)',
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
