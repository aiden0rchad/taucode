import { useEffect, useSyncExternalStore } from 'react';
import { themesList } from '@services/tauri';
import { subscribeThemesChanged } from '@services/events';
import { validateTheme, type ThemeFile } from './validate';
let files: ThemeFile[] = [];
let loaded = false;
let revision = 0;
let request = 0;
const listeners = new Set<() => void>();
const subscribe = (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; };
const snapshot = () => revision;
export async function reloadThemes() {
  const seq = ++request;
  const result = await themesList();
  if (seq !== request) return;
  files = (result ?? []).map(file => {
    const errors = file.manifest ? validateTheme(file.manifest) : [];
    return errors.length ? { file: file.file, error: errors.join('\n') } : file;
  });
  loaded = true;
  revision++;
  listeners.forEach(fn => fn());
}
export function useThemes() {
  useSyncExternalStore(subscribe, snapshot);
  useEffect(() => {
    void reloadThemes().catch(() => { /* A disconnected backend keeps the last valid catalog. */ });
    return subscribeThemesChanged(() => { void reloadThemes().catch(() => {}); });
  }, []);
  return { files, loaded };
}
