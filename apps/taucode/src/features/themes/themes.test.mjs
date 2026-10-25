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
      const surfaces = ['app','sidebar','topbar','messages','composer','card','popover','right-panel','command'];
      for (const surface of surfaces) {
        const bg = rgba(theme.tokens['--surface-'+surface]);
        expect(bg, surface).not.toBeNull();
        if (!theme.window?.translucent || !['app','messages','sidebar','topbar'].includes(surface)) expect(bg[3],surface).toBeGreaterThanOrEqual(theme.window?.translucent ? .94 : 1);
        const backdrops = theme.window?.translucent ? (theme.appearance === 'dark' ? ['#000000','#4a4a4e'] : ['#ffffff','#bdbdc2']) : ['#000000','#ffffff'];
        for (const desktop of backdrops) {
          const composed = composite(bg,rgba(desktop));
          expect(contrast(rgba(theme.tokens['--text-primary']),composed),surface).toBeGreaterThanOrEqual(theme.id === 'matrix' ? 7 : 4.5);
          expect(contrast(rgba(theme.tokens['--text-muted']),composed),surface).toBeGreaterThanOrEqual(3);
        }
      }
    });
  }
  it('keeps glass visibly translucent and sidebar cards readable over the material', () => {
    for (const theme of builtins.filter(t => t.window?.translucent).map(resolveTheme)) {
      for (const surface of ['sidebar','topbar']) expect(rgba(theme.tokens['--surface-'+surface])[3]).toBeLessThanOrEqual(.5);
      for (const surface of ['app','messages']) expect(rgba(theme.tokens['--surface-'+surface])[3]).toBe(.72);
      const backdrops = theme.appearance === 'dark' ? ['#000000','#4a4a4e'] : ['#ffffff','#bdbdc2'];
      for (const desktop of backdrops) {
        const sidebar = composite(rgba(theme.tokens['--surface-sidebar']),rgba(desktop));
        const card = composite([...rgba(theme.tokens['--surface-card']).slice(0,3),.35],sidebar);
        expect(contrast(rgba(theme.tokens['--text-primary']),card)).toBeGreaterThanOrEqual(4.5);
        expect(contrast(rgba(theme.tokens['--text-muted']),card)).toBeGreaterThanOrEqual(3);
      }
    }
  });
  it('uses readable neon actions and muted card text', () => {
    const { tokens } = resolveTheme(builtins.find(t => t.id === 'cyberpunk-neon'));
    expect(contrast(rgba(tokens['--surface-app']),rgba(tokens['--border-accent']))).toBeGreaterThanOrEqual(4.5);
    expect(contrast(rgba(tokens['--text-muted']),rgba(tokens['--surface-card']))).toBeGreaterThanOrEqual(4.5);
  });
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
