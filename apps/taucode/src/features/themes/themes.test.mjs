import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { builtins, resolveTheme, selectTheme } from './builtins';
import { validateTheme } from './validate';
import { rgba, composite, contrast } from './contrast';
import { themeTokens } from './tokens';
it('resolves themeable scrollbar colors for every palette', () => {
  for (const builtin of builtins) {
    const theme = resolveTheme(builtin);
    for (const token of ['--scrollbar-thumb','--scrollbar-thumb-hover']) {
      expect(themeTokens[token].group).toBe('Scrollbars');
      expect(theme.tokens[token]).toBeTruthy();
    }
    if (builtin.id.startsWith('glass-')) expect(theme.tokens['--scrollbar-thumb']).toBe('rgba(255,255,255,0.22)');
    else if (builtin.id === 'cyberpunk-neon') expect(theme.tokens['--scrollbar-thumb-hover']).toBe('rgba(0,229,255,0.5)');
    else if (builtin.id === 'matrix') expect(theme.tokens['--scrollbar-thumb']).toBe('rgba(0,255,65,0.22)');
    else expect(theme.tokens['--scrollbar-thumb']).toContain('var(--text-primary) 18%');
  }
});
it('keeps heat labels readable across all themes without changing slider heat colors', () => {
  const css = readFileSync('src/features/composer/components/reasoning.css','utf8');
  expect(css).toContain('--reasoning-ink: color-mix(in srgb, var(--reasoning-color) 45%, var(--text-primary))');
  for (const theme of builtins.map(resolveTheme)) {
    for (const heat of ['#8998ef','#e9ac38','#f06436']) {
      const foreground = rgba(theme.tokens['--text-primary']);
      const ink = rgba(heat).slice(0,3).map((v,i) => v*.45+foreground[i]*.55).concat(1);
      for (const surface of ['--surface-popover','--surface-composer']) expect(contrast(ink,rgba(theme.tokens[surface])),theme.id+heat+surface).toBeGreaterThanOrEqual(4.5);
    }
  }
});
import { cardStyle } from './cardStyle';
it('limits new border styling to Glass and Neon and resets inherited CSS keys', () => {
  for (const theme of builtins) {
    if (theme.id.startsWith('glass-')) {
      expect(theme.shape.corners).toBe('squircle');
      expect(theme.border).toMatchObject({angle:160,highlight:true});
      expect(theme.border.gradient).toHaveLength(3);
    } else if (theme.id === 'cyberpunk-neon') {
      expect(theme.shape).toEqual({corners:'chamfer',chamfer:10});
      expect(theme.border).toEqual({gradient:['#ff2bd6','#a855f7','#00e5ff'],angle:135});
      expect(cardStyle(theme,false)['--card-neon-glow']).toBe('0');
      expect(cardStyle(theme,false)['--card-neon-fill']).toBe('1');
    } else {
      expect(theme.shape).toBeUndefined();
      expect(theme.border).toBeUndefined();
      expect(cardStyle(theme)).toMatchObject({'--card-corners':'squircle','--card-border-stops':'0','--card-border-highlight':'0','--card-border-1':'transparent'});
    }
  }
  const inherited = resolveTheme({schemaVersion:1,id:'custom',name:'Custom',appearance:'dark',extends:'cyberpunk-neon',tokens:{}});
  expect(inherited.shape.corners).toBe('chamfer');
  expect(resolveTheme({...inherited,border:{}}).border.gradient).toBeUndefined();
});
it('separates the average Glass Light edge from its sidebar by at least 1.3:1', () => {
  const theme = resolveTheme(builtins.find(t => t.id === 'glass-light'));
  const renderer = readFileSync('src/features/design-system/components/squircle/Squircle.tsx','utf8');
  const hairline = rgba(renderer.match(/className="squircle-hairline"[^>]*stroke="([^"]+)"/)[1]);
  const stops = theme.border.gradient.map(rgba);
  for (const desktop of ['#ffffff','#bdbdc2']) {
    const sidebar = composite(rgba(theme.tokens['--surface-sidebar']),rgba(desktop));
    const edge = composite(hairline,sidebar);
    const samples = Array.from({length:101},(_,i) => {
      const position = i/50, index = Math.min(1,Math.floor(position)), t = position-index;
      const a = composite(stops[index],edge), b = composite(stops[index+1],edge);
      return a.map((v,j) => v*(1-t)+b[j]*t);
    });
    // The 2px hairline under a 1px gradient contributes half the painted edge.
    const average = edge.slice(0,3).map((v,i) => (v+samples.reduce((sum,c) => sum+c[i],0)/samples.length)/2).concat(1);
    expect(contrast(average,sidebar)).toBeGreaterThanOrEqual(1.3);
  }
});
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
