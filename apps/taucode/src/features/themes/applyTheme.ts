import './themes.css';
import { windowSetMaterial } from '@services/tauri';
import { pushErrorToast } from '@services/toasts';
import { resolveTheme } from './builtins';
import { validateTheme, type ThemeManifest } from './validate';
import { cardStyle } from './cardStyle';
let materials = Promise.resolve();
let lastMaterial: string | undefined;
let lastTheme: string | undefined;
const storageKey = 'taucode.theme.last';
// Sort object keys so file formatting/order cannot trigger a theme re-apply.
function content(value: unknown): string {
  return JSON.stringify(value, (_, v) => v && typeof v === 'object' && !Array.isArray(v)
    ? Object.fromEntries(Object.keys(v).sort().map(k => [k, v[k]])) : v);
}
function hash(theme: ThemeManifest): string {
  let value = 2166136261;
  for (const char of content(theme)) value = Math.imul(value ^ char.charCodeAt(0), 16777619);
  return (value >>> 0).toString(16);
}
export function restoreTheme() {
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) ?? 'null');
    if (!saved || validateTheme(saved.manifest).length || saved.id !== saved.manifest.id
      || saved.hash !== hash(saved.manifest) || typeof saved.animate !== 'boolean'
      || typeof saved.glow !== 'boolean' || typeof saved.system !== 'boolean') return;
    applyResolvedTheme(saved.manifest, saved.animate, saved.glow, saved.system);
  } catch { /* A missing, invalid or unavailable cache leaves settings in charge. */ }
}
export function applyTheme(manifest: ThemeManifest, animate = true, glow = true, system = false) {
  const errors = validateTheme(manifest);
  if (errors.length) throw new Error(errors.join('\n'));
  return applyResolvedTheme(resolveTheme(manifest), animate, glow, system);
}
function applyResolvedTheme(theme: ThemeManifest, animate: boolean, glow: boolean, system: boolean) {
  const material = theme.window?.translucent && theme.window.material !== 'none' ? theme.window.material : null;
  const appearance = system ? null : theme.appearance;
  const key = JSON.stringify([material, appearance]);
  if (lastMaterial !== key) {
    lastMaterial = key;
    materials = materials.then(() => windowSetMaterial(material, appearance)).catch(error => {
      if (lastMaterial === key) lastMaterial = undefined;
      pushErrorToast({ title: 'Window material unavailable', message: String(error) });
    });
  }
  const signature = content([theme, animate, glow, system]);
  if (lastTheme === signature) return theme;
  const root = document.documentElement;
  const style = document.getElementById('taucode-theme') ?? Object.assign(document.createElement('style'), { id: 'taucode-theme' });
  const fonts = Object.entries(theme.fonts ?? {}).map(([k,v]) => `--${k === 'mono' ? 'code' : k}-font-family:${v}!important;`).join('') + Object.entries(cardStyle(theme, glow)).map(([k,v]) => `${k}:${v};`).join('');
  // Even a custom alpha color cannot expose the desktop in an opaque theme.
  const backing = theme.window?.translucent ? '' : `background:linear-gradient(var(--surface-app),var(--surface-app)),${theme.appearance === 'light' ? '#f7f7f8' : '#141416'};`;
  style.textContent = `:root[data-theme-id="${theme.id}"]{${Object.entries(theme.tokens).map(([k,v]) => `${k}:${v};`).join('')}${fonts}${backing}${theme.fonts?.ui ? 'font-family:var(--ui-font-family);' : ''}color-scheme:${theme.appearance};}`;
  if (!style.parentNode) document.head.append(style);
  root.dataset.theme = theme.appearance;
  root.dataset.themeId = theme.id;
  root.dataset.translucent = String(!!theme.window?.translucent);
  root.dataset.neonGlow = String(!!theme.effects?.neonGlow && glow);
  root.dataset.neonPalette = String(!!theme.effects?.neonGlow);
  root.dataset.digitalRain = String(!!theme.effects?.digitalRain && animate);
  root.style.colorScheme = theme.appearance;
  lastTheme = signature;
  try {
    localStorage.setItem(storageKey, JSON.stringify({ id: theme.id, hash: hash(theme), manifest: theme, animate, glow, system }));
  } catch { /* Storage can be disabled or full; the theme still applies. */ }
  window.dispatchEvent(new Event('taucode-theme-applied'));
  return theme;
}
