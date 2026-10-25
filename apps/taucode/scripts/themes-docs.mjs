import { readFileSync, writeFileSync } from 'node:fs';
const css = readFileSync('src/styles/themes.dark.css', 'utf8');
export const tokens = Object.fromEntries([...css.matchAll(/(--[\w-]+):\s*([^;]+);/g)].filter(([, key]) => /^--(scrollbar|surface|text|border|status|tree|shadow|message-link|select-caret|radius|terminal|code-color)/.test(key)).map(([, key]) => [key, { group: key.startsWith('--scrollbar') ? 'Scrollbars' : key.includes('radius') ? 'Radii' : key.includes('terminal') ? 'Terminal' : key.includes('code-color') ? 'Code' : key.includes('accent') || key.includes('link') ? 'Accent' : key.startsWith('--text') ? 'Text' : key.startsWith('--border') ? 'Borders' : key.startsWith('--status') ? 'Status' : key.startsWith('--tree') ? 'Tree' : 'Surfaces', label: key.slice(2).replaceAll('-', ' ') }]));
export const table = '| Token | Group | Label |\n| --- | --- | --- |\n' + Object.entries(tokens).map(([key, { group, label }]) => `| \`${key}\` | ${group} | ${label} |`).join('\n');
export const effectsTable = '| Effect | Type | Behavior |\n| --- | --- | --- |\n| `effects.digitalRain` | boolean | Empty-state animation; respects reduced motion and Animated background. |\n| `effects.neonGlow` | boolean | Static accent, focus, selected-row and heading glows; controlled by Glow effects. |';
export const shapeTable = '| Field | Values | Default |\n| --- | --- | --- |\n| `shape.corners` | squircle, round, chamfer | squircle |\n| `shape.chamfer` | 4–20 px | 10 |\n| `border.gradient` | 2–4 safe colors | Solid border |\n| `border.angle` | 0–360 degrees | 135 |\n| `border.highlight` | boolean | false |';
if (process.argv[1]?.endsWith('themes-docs.mjs')) {
  writeFileSync('src/features/themes/tokens.json', JSON.stringify(tokens, null, 2) + '\n');
  const palettes = Object.fromEntries(['dark', 'light', 'dim'].map(id => [id, Object.fromEntries([...readFileSync(`src/styles/themes.${id}.css`, 'utf8').matchAll(/(--[\w-]+):\s*([^;]+);/g)].filter(m => Object.hasOwn(tokens, m[1])).map(m => [m[1], m[2]]))]));
  writeFileSync('src/features/themes/palettes.json', JSON.stringify(palettes, null, 2) + '\n');
  const schemaPath = '../../docs/theme.schema.json';
  const schema = JSON.parse(readFileSync(schemaPath, 'utf8'));
  schema.properties.tokens.properties = Object.fromEntries(Object.entries(tokens).map(([key, token]) => [key, { $ref: `#/$defs/${token.group === 'Radii' ? 'radius' : 'color'}`, description: token.label }]));
  writeFileSync(schemaPath, JSON.stringify(schema, null, 2) + '\n');
  const path = '../../docs/themes.md';
  const doc = readFileSync(path, 'utf8');
  writeFileSync(path, doc.replace(/<!-- shape:start -->[\s\S]*<!-- shape:end -->/, `<!-- shape:start -->\n${shapeTable}\n<!-- shape:end -->`).replace(/<!-- effects:start -->[\s\S]*<!-- effects:end -->/, `<!-- effects:start -->\n${effectsTable}\n<!-- effects:end -->`).replace(/<!-- tokens:start -->[\s\S]*<!-- tokens:end -->/, `<!-- tokens:start -->\n${table}\n<!-- tokens:end -->`));
}
