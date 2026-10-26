import { useEffect, useRef } from 'react';
import type { AppSettings } from '@/types';
import { builtins, selectTheme } from '@/features/themes/builtins';
import { applyTheme } from '@/features/themes/applyTheme';
import { useThemes } from '@/features/themes/store';
import { pushErrorToast } from '@services/toasts';

export function useThemePreference(settings: Pick<AppSettings, "theme" | "themeLight" | "themeDark" | "themeAnimations">, fallback: () => void, ready = true) {
  const { files, loaded } = useThemes();
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;
  useEffect(() => {
    if (!ready) return;
    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    const update = () => {
      const themes = [...builtins, ...files.flatMap(f => f.manifest ? [f.manifest] : [])];
      const selected = settings.theme === 'system'
        ? (media?.matches ? settings.themeDark ?? 'dark' : settings.themeLight ?? 'light') : settings.theme;
      if (!loaded && !themes.some(t => t.id === selected)) return;
      let system = settings.theme === 'system';
      if (loaded && settings.theme !== 'system' && !themes.some(t => t.id === settings.theme)) {
        // Keep the last valid theme while a developer fixes a broken file.
        if (files.some(f => f.error && f.file === `${settings.theme}.taucode-theme.json`)) return;
        pushErrorToast({ id: 'theme-unavailable', title: 'Theme unavailable', message: `“${settings.theme}” was removed or is unknown. Using System.` });
        fallbackRef.current();
        system = true;
      }
      const theme = selectTheme(settings.theme, themes, media?.matches ?? false, settings.themeLight, settings.themeDark);
      applyTheme(theme, settings.themeAnimations?.[theme.id] !== false,settings.themeAnimations?.[`${theme.id}:neonGlow`] !== false, system);
    };
    update();
    media?.addEventListener('change', update);
    return () => media?.removeEventListener('change', update);
  }, [files, loaded, ready, settings.theme, settings.themeLight, settings.themeDark, settings.themeAnimations]);
}
