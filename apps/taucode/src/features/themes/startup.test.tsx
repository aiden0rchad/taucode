// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { builtins, resolveTheme } from './builtins';
import type { ThemeFile } from './validate';
import { windowSetMaterial } from '@services/tauri';

const catalog = vi.hoisted(() => ({ files: [] as ThemeFile[], loaded: false }));
vi.mock('./store', () => ({ useThemes: () => catalog }));
vi.mock('@services/tauri', () => ({ windowSetMaterial: vi.fn(async () => {}) }));
vi.mock('@services/toasts', () => ({ pushErrorToast: vi.fn() }));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  localStorage.clear();
  document.getElementById('taucode-theme')?.remove();
  catalog.files = [];
  catalog.loaded = false;
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const glass = builtins.find(t => t.id === 'glass-dark')!;

async function launch(theme = glass, system = false) {
  const previous = await import('./applyTheme');
  previous.applyTheme(theme, false, false, system);
  await act(async () => {});
  const saved = JSON.parse(localStorage.getItem('taucode.theme.last')!);
  expect(saved).toMatchObject({ id: theme.id, hash: expect.any(String), manifest: resolveTheme(theme) });
  vi.resetModules();
  vi.mocked(windowSetMaterial).mockClear();
  document.getElementById('taucode-theme')?.remove();
  const applier = await import('./applyTheme');
  applier.restoreTheme();
  expect(document.documentElement.dataset.themeId).toBe(theme.id);
  expect(document.getElementById('taucode-theme')?.textContent).toContain('--surface-app:');
  return applier;
}

it('restores Glass before settings, dedupes settings/reloads, and clears it once for opaque', async () => {
  await launch();
  const { useThemePreference } = await import('@/features/layout/hooks/useThemePreference');
  const fallback = vi.fn();
  const applied = vi.fn();
  window.addEventListener('taucode-theme-applied', applied);
  const view = renderHook(({ theme, ready }) => useThemePreference({ theme, themeAnimations: { 'glass-dark': false, 'glass-dark:neonGlow': false } }, fallback, ready),
    { initialProps: { theme: 'system', ready: false } });
  await act(async () => {});
  expect(vi.mocked(windowSetMaterial).mock.calls).toEqual([['sidebar', 'dark']]);
  view.rerender({ theme: 'glass-dark', ready: true });
  catalog.loaded = true;
  view.rerender({ theme: 'glass-dark', ready: true });
  await act(async () => {});
  expect(applied).not.toHaveBeenCalled();
  expect(windowSetMaterial).toHaveBeenCalledTimes(1);
  view.rerender({ theme: 'light', ready: true });
  await act(async () => {});
  expect(vi.mocked(windowSetMaterial).mock.calls).toEqual([['sidebar', 'dark'], [null, 'light']]);
  window.removeEventListener('taucode-theme-applied', applied);
});

it('retains a cached custom theme until the catalog arrives and applies changed content', async () => {
  const custom = { ...glass, id: 'custom', tokens: { '--text-accent': '#123456' } };
  await launch(custom);
  const { useThemePreference } = await import('@/features/layout/hooks/useThemePreference');
  const fallback = vi.fn();
  const view = renderHook(() => useThemePreference({ theme: 'custom', themeAnimations: { custom: false, 'custom:neonGlow': false } }, fallback));
  expect(document.documentElement.dataset.themeId).toBe('custom');
  catalog.loaded = true;
  catalog.files = [{ file: 'custom.taucode-theme.json', manifest: { ...custom, tokens: { '--text-accent': '#abcdef' } } }];
  view.rerender();
  await act(async () => {});
  expect(document.getElementById('taucode-theme')?.textContent).toContain('--text-accent:#abcdef');
  expect(windowSetMaterial).toHaveBeenCalledTimes(1);
  expect(fallback).not.toHaveBeenCalled();
});

it('passes null appearance for System, including cached Glass pairings', async () => {
  const { applyTheme } = await launch(glass, true);
  const { useThemePreference } = await import('@/features/layout/hooks/useThemePreference');
  vi.stubGlobal('matchMedia', () => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
  const view = renderHook(() => useThemePreference({ theme: 'system', themeDark: 'glass-dark' }, vi.fn()));
  await act(async () => {});
  expect(vi.mocked(windowSetMaterial).mock.calls).toEqual([['sidebar', null]]);
  view.unmount();
  applyTheme(builtins[0], true, true, true);
  await act(async () => {});
  expect(windowSetMaterial).toHaveBeenLastCalledWith(null, null);
  vi.unstubAllGlobals();
});

it.each(['null', '{', '{"manifest":{}}'])('ignores an invalid startup cache: %s', async value => {
  localStorage.setItem('taucode.theme.last', value);
  const { restoreTheme } = await import('./applyTheme');
  restoreTheme();
  await act(async () => {});
  expect(windowSetMaterial).not.toHaveBeenCalled();
  expect(document.getElementById('taucode-theme')).toBeNull();
});

it('rejects a changed cache hash and tolerates unavailable storage', async () => {
  const { applyTheme } = await import('./applyTheme');
  applyTheme(glass);
  await act(async () => {});
  const saved = JSON.parse(localStorage.getItem('taucode.theme.last')!);
  saved.manifest.tokens['--text-accent'] = '#123456';
  localStorage.setItem('taucode.theme.last', JSON.stringify(saved));
  vi.resetModules();
  vi.mocked(windowSetMaterial).mockClear();
  const next = await import('./applyTheme');
  next.restoreTheme();
  await act(async () => {});
  expect(windowSetMaterial).not.toHaveBeenCalled();
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('disabled'); });
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('full'); });
  expect(() => next.restoreTheme()).not.toThrow();
  expect(() => next.applyTheme(glass)).not.toThrow();
  await act(async () => {});
  expect(windowSetMaterial).toHaveBeenCalledWith('sidebar', 'dark');
});
