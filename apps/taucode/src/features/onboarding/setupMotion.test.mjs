// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { afterEach, expect, it, vi } from 'vitest';
it('uses direction-aware CSS motion, reduced-motion crossfades, and static hidden blobs', () => {
  const css = readFileSync('src/features/onboarding/onboarding.css', 'utf8');
  expect(css).toContain('.setup-forward.setup-exit');
  expect(css).toContain('.setup-back.setup-enter');
  expect(css).toContain('@media(prefers-reduced-motion:reduce)');
  expect(css).toContain('animation:setup-fade 150ms both!important');
  expect(css).toContain('animation:none!important;filter:blur(80px)!important;transform:none!important');
  expect(css).toContain('animation-play-state:paused');
});

it('animates recap connectors and checks with shared delays, and keeps content entrances below 700ms', () => {
  const css = readFileSync('src/features/onboarding/onboarding.css', 'utf8');
  for (const selector of ['.setup-recap li', '.setup-recap li::before', '.setup-recap .setup-dot svg']) {
    const rule = css.slice(css.indexOf(selector + '{')).split('}')[0];
    expect(rule).toContain('animation-delay:var(--recap-delay)');
  }
  expect(css).toContain('animation:setup-line .24s');
  expect(css).toContain('animation:setup-check .24s');
  expect(css).toContain('animation:setup-word .42s');
  expect(css).toContain('animation-delay:min(calc(var(--word) * 35ms),245ms)');
  expect(css).toContain('animation-delay:min(calc(60ms + var(--word) * 25ms),300ms)');
  expect(css).toContain('@container(min-width:640px){.setup-theme-grid{grid-template-columns:repeat(4,minmax(0,1fr))}}');
  expect(css).toContain('.setup-provider-grid{grid-template-columns:repeat(3,minmax(0,1fr))');
  expect(css).toContain('.setup-theme .theme-mini{width:100%;height:78px;pointer-events:none}');
});

import { createElement } from 'react';
import { act, cleanup, render } from '@testing-library/react';
import { clearMocks } from '@tauri-apps/api/mocks';
import { SetupWizard } from './SetupWizard';
import { installUiPreview } from '@/dev/uiPreview';
import { builtins, resolveTheme } from '@/features/themes/builtins';
afterEach(() => { cleanup(); clearMocks(); vi.unstubAllEnvs(); localStorage.clear(); });
it.each(['dark', 'light', 'glass-dark', 'claude-dark', 'cyberpunk-neon'])('paints an opaque takeover immediately in %s, without shell or backdrop entrance animations', async themeId => {
  vi.stubEnv('MODE', 'uipreview');
  window.history.replaceState(null, '', '/?setup=done');
  installUiPreview();
  const theme = resolveTheme(builtins.find(theme => theme.id === themeId));
  // jsdom does not resolve custom properties. Substitute the actual theme token,
  // then check the production stylesheet's cascade on the mounted wizard.
  const css = readFileSync('src/features/onboarding/onboarding.css', 'utf8').replaceAll('var(--setup-base)', theme.tokens['--setup-base']);
  const style = document.createElement('style');
  style.textContent = readFileSync('src/styles/ds-modal.css', 'utf8') + css;
  document.head.append(style);
  try {
    render(createElement(SetupWizard, { onSettings: vi.fn(), onFinish: vi.fn() }));
    const root = document.querySelector('.setup-takeover');
    expect(getComputedStyle(root).backgroundColor).toMatch(/^rgb\(\d+, \d+, \d+\)$/);
    for (const element of [root, root.querySelector('.ds-modal-backdrop'), root.querySelector('.ds-modal-card')]) {
      expect(getComputedStyle(element).opacity).toBe('1');
      expect(getComputedStyle(element).animation).toBe('none');
    }
    expect(css).toContain('.setup-overlay > .ds-modal-backdrop{animation:setup-fade 150ms both}');
    await act(async () => {});
  } finally { style.remove(); }
});
