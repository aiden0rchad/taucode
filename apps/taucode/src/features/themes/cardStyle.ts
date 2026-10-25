import type { ThemeManifest } from './validate';

// Shared by the root applier and isolated gallery/editor previews. Reset every key.
export function cardStyle(theme: ThemeManifest, glow = true) {
  const stops = theme.border?.gradient ?? [];
  return {
    '--card-corners': theme.shape?.corners ?? 'squircle',
    '--card-chamfer': `${theme.shape?.chamfer ?? 10}px`,
    ...Object.fromEntries(Array.from({ length: 4 }, (_, i) => [`--card-border-${i + 1}`, stops[i] ?? 'transparent'])),
    '--card-border-stops': String(stops.length),
    '--card-border-angle': String(theme.border?.angle ?? 135),
    '--card-border-highlight': theme.border?.highlight ? '1' : '0',
    '--card-neon-glow': theme.effects?.neonGlow && glow ? '1' : '0',
    '--card-neon-fill': theme.effects?.neonGlow ? '1' : '0',
    '--card-light-edge': theme.border?.highlight && theme.appearance === 'light' ? '1' : '0',
  };
}
