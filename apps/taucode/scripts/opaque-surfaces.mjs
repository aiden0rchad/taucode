// One-time migration: composite existing surface colors over each theme's app base.
// Borders, text, shadows, fonts and dimensions are deliberately untouched.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import postcss from 'postcss';

const bases = { dark: [18, 18, 18], light: [246, 247, 250], dim: [41, 44, 51] };
const hex = rgb => '#' + rgb.map(v => Math.round(v).toString(16).padStart(2, '0')).join('');
const blend = (rgb, alpha, base) => hex(rgb.map((v, i) => v * alpha + base[i] * (1 - alpha)));
const rgba = /rgba\(\s*([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\)/g;

function migrate(path) {
  const css = postcss.parse(readFileSync(path, 'utf8'));
  css.walkRules(rule => {
    if (!rule.selector.includes('reduced-transparency')) return;
    const selectors = rule.selectors.filter(selector => !selector.includes('reduced-transparency'));
    if (selectors.length) rule.selectors = selectors;
    else rule.remove();
  });
  css.walkDecls(decl => {
    if (decl.prop.includes('backdrop-filter')) { decl.remove(); return; }
    if (decl.prop === 'transition' && decl.value.includes('backdrop-filter')) {
      decl.value = decl.value.replace(/,?\s*backdrop-filter[^,]*/g, '');
    }
    if (!/^(background(-color|-image)?|--.*surface.*|--.*-bg(?:-.*)?|--ds-modal-backdrop)$/.test(decl.prop)) return;
    const theme = path.includes('themes.light') || path.includes('themes.system') ? 'light' : path.includes('themes.dim') ? 'dim' : 'dark';
    const themed = path.includes('themes.');
    const alpha = decl.value.match(/\/\s*([\d.]+)\s*\)$/);
    if (/^(hsl|rgb)a?\(/.test(decl.value) && alpha) {
      decl.value = `color-mix(in srgb, ${decl.value.replace(/\/\s*[\d.]+\s*\)$/, ')')} ${+alpha[1] * 100}%, var(--surface-app))`;
    }
    decl.value = decl.value.replace(rgba, (_, r, g, b, a) => themed
      ? blend([+r, +g, +b], +a, bases[theme])
      : `color-mix(in srgb, rgb(${r}, ${g}, ${b}) ${+(+a * 100).toFixed(4)}%, var(--surface-app))`);
    // Transparent controls still inherit their parent's opaque surface; gradients
    // and color mixes must resolve against the theme instead of the desktop.
    if (decl.value !== 'transparent') decl.value = decl.value.replace(/\btransparent\b/g, 'var(--surface-app)');
    if (themed && decl.parent.selector === ':root' && decl.prop === 'background-color') decl.value = 'var(--surface-app)';
  });
  css.walkRules(rule => { if (!rule.nodes.length) rule.remove(); });
  css.walkAtRules(rule => { if (rule.nodes && !rule.nodes.length) rule.remove(); });
  writeFileSync(path, css.toString());
}
function walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (entry.name.endsWith('.css')) migrate(path);
  }
}
walk('src');

// A text/border/status color used as a fill needs a separate opaque variant;
// retain the original alpha on text, borders and shadows.
const themePaths = Object.fromEntries(['dark', 'dim', 'light', 'system'].map(theme => [theme, `src/styles/themes.${theme}.css`]));
const themes = Object.fromEntries(Object.entries(themePaths).map(([theme, path]) => [theme, postcss.parse(readFileSync(path, 'utf8'))]));
const fills = new Set();
function fillVariants(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) { fillVariants(path); continue; }
    if (!entry.name.endsWith('.css')) continue;
    const css = postcss.parse(readFileSync(path, 'utf8'));
    css.walkDecls(decl => {
      if (!/^(background(-color|-image)?|--.*surface.*|--.*-bg(?:-.*)?)$/.test(decl.prop)) return;
      decl.value = decl.value.replace(/var\(--((?:text|border|status)-[\w-]+)/g, (_, name) => {
        fills.add(name);
        return `var(--surface-${name}`;
      });
    });
    writeFileSync(path, css.toString());
  }
}
fillVariants('src');
for (const [theme, css] of Object.entries(themes)) {
  const values = {};
  themes.dark.walkDecls(decl => { values[decl.prop] = decl.value; });
  css.walkDecls(decl => { values[decl.prop] = decl.value; });
  const base = bases[theme === 'system' ? 'light' : theme];
  const rule = css.nodes.find(node => node.type === 'rule') ?? css.nodes[0].nodes.find(node => node.type === 'rule');
  for (const name of fills) {
    const value = values[`--${name}`];
    if (!value || values[`--surface-${name}`]) continue;
    rule.append({ prop: `--surface-${name}`, value: value.replace(rgba, (_, r, g, b, a) => blend([+r, +g, +b], +a, base)) });
  }
  writeFileSync(themePaths[theme], css.toString());
}
