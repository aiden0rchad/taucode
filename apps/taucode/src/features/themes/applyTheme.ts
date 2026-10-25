import './themes.css';
import { windowSetMaterial } from '@services/tauri';
import { pushErrorToast } from '@services/toasts';
import { resolveTheme } from './builtins';
import { validateTheme, type ThemeManifest } from './validate';
import { cardStyle } from './cardStyle';
let materials = Promise.resolve();
let lastMaterial: string | null | undefined;
export function applyTheme(manifest: ThemeManifest, animate = true, glow = true) {
  const errors = validateTheme(manifest);
  if (errors.length) throw new Error(errors.join('\n'));
  const theme = resolveTheme(manifest), root = document.documentElement;
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
  const material = theme.window?.translucent && theme.window.material !== 'none' ? theme.window.material : null;
  const appearance = material ? theme.appearance : null;
  const key = JSON.stringify([material, appearance]);
  if (lastMaterial !== key) {
    lastMaterial = key;
    materials = materials.then(() => windowSetMaterial(material, appearance)).catch(error => {
      lastMaterial = undefined;
      pushErrorToast({ title: 'Window material unavailable', message: String(error) });
    });
  }
  window.dispatchEvent(new Event('taucode-theme-applied'));
  return theme;
}
