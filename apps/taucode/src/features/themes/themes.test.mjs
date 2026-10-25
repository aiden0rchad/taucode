import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { builtins, resolveTheme, selectTheme } from './builtins';
import { validateTheme } from './validate';
import { rgba, composite, contrast } from './contrast';
import { themeTokens } from './tokens';
it('paints swatches over a checkerboard with the shared one-pixel inset squircle ring', () => {
  const css = readFileSync('src/features/themes/themes.css','utf8');
  const rule = selector => css.split(selector+' {')[1].split('}')[0];
  expect(rule('.theme-color-swatch')).toContain('--squircle-border: var(--border-strong)');
  expect(rule('.theme-color-swatch > .squircle-background')).toContain('linear-gradient(var(--theme-swatch-color), var(--theme-swatch-color)), repeating-conic-gradient(');
  expect(rule('.theme-color-swatch > .squircle-background')).toContain('background-size: auto, 8px 8px');
  expect(rule('.theme-color-swatch input[type="color"]')).toContain('opacity: 0');
  const squircle = readFileSync('src/features/design-system/components/squircle/squircle.css','utf8');
  expect(squircle).toContain('border: 1px solid var(--squircle-border, var(--border-subtle))');
});
describe('shared theme fixtures', () => {
  for (const file of readdirSync('src/features/themes/fixtures').filter(f => f.endsWith('.json'))) {
    it(file, () => expect(validateTheme(JSON.parse(readFileSync('src/features/themes/fixtures/'+file,'utf8'))).length === 0).toBe(file.startsWith('valid-')));
  }
});
describe('built-in themes', () => {
  for (const manifest of builtins) {
    it(manifest.id+' is valid and has readable, opaque content', () => {
      expect(validateTheme(manifest)).toEqual([]);
      const theme = resolveTheme(manifest);
      expect(Object.keys(theme.tokens).sort()).toEqual(Object.keys(themeTokens).sort());
      const surfaces = ['app','sidebar','topbar','messages','composer','card','popover','right-panel'];
      for (const surface of surfaces) {
        const bg = rgba(theme.tokens['--surface-'+surface]);
        expect(bg, surface).not.toBeNull();
        if (!theme.window?.translucent || !['sidebar','topbar'].includes(surface)) expect(bg[3],surface).toBeGreaterThanOrEqual(theme.window?.translucent ? .92 : 1);
        for (const desktop of [0,255]) {
          const composed = composite(bg,[desktop,desktop,desktop,1]);
          expect(contrast(rgba(theme.tokens['--text-primary']),composed),surface).toBeGreaterThanOrEqual(theme.id === 'matrix' ? 7 : 4.5);
          if (theme.window?.translucent) expect(contrast(rgba(theme.tokens['--text-muted']),composed),surface).toBeGreaterThanOrEqual(3);
        }
      }
    });
  }
  it('inherits only overrides and follows System pairings', () => {
    const custom = { ...builtins[0],id:'custom',extends:'light',appearance:'light',tokens:{ '--text-accent':'#005522' } };
    expect(resolveTheme(custom).tokens['--surface-app']).toBe(resolveTheme(builtins.find(t => t.id === 'light')).tokens['--surface-app']);
    expect(resolveTheme(custom).tokens['--text-accent']).toBe('#005522');
    expect(selectTheme('system',builtins,false,'claude-light','claude-dark').id).toBe('claude-light');
    expect(selectTheme('system',builtins,true,'claude-light','claude-dark').id).toBe('claude-dark');
    expect(selectTheme('missing',builtins,false).id).toBe('light');
    expect(selectTheme('system',builtins,false,'dark').id).toBe('light');
    for (const id of ['dark','light','dim']) expect(selectTheme(id,builtins,true).id).toBe(id);
  });
  it('preserves the original CSS values and paints shell surfaces', () => {
    for (const id of ['dark','light','dim']) {
      const css = readFileSync('src/styles/themes.'+id+'.css','utf8');
      const tokens = builtins.find(t => t.id === id).tokens;
      for (const [,key,value] of css.matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
        if (Object.prototype.hasOwnProperty.call(themeTokens,key)) expect(tokens[key],id+key).toBe(value);
      }
    }
    const css = readFileSync('src/styles/base.css','utf8');
    expect(css).toMatch(/html,\s*body\s*\{[^}]*background: var\(--surface-app\)/);
    expect(css).toMatch(/\.app\s*\{[^}]*background: var\(--surface-app\)/);
    expect(readFileSync('src/styles/sidebar.css','utf8')).toContain('background: var(--surface-sidebar)');
  });
});
