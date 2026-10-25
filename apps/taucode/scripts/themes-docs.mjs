import { readFileSync, writeFileSync } from 'node:fs';
const css = readFileSync('src/styles/themes.dark.css', 'utf8');
export const tokens = Object.fromEntries([...css.matchAll(/(--[\w-]+):\s*([^;]+);/g)].filter(([, key]) => /^--(surface|text|border|status|tree|shadow|message-link|select-caret|radius|terminal|code-color)/.test(key)).map(([, key]) => [key, { group: key.includes('radius') ? 'Radii' : key.includes('terminal') ? 'Terminal' : key.includes('code-color') ? 'Code' : key.includes('accent') || key.includes('link') ? 'Accent' : key.startsWith('--text') ? 'Text' : key.startsWith('--border') ? 'Borders' : key.startsWith('--status') ? 'Status' : key.startsWith('--tree') ? 'Tree' : 'Surfaces', label: key.slice(2).replaceAll('-', ' ') }]));
export const table = '| Token | Group | Label |\n| --- | --- | --- |\n' + Object.entries(tokens).map(([key, { group, label }]) => `| \`${key}\` | ${group} | ${label} |`).join('\n');
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
  writeFileSync(path, doc.replace(/<!-- tokens:start -->[\s\S]*<!-- tokens:end -->/, `<!-- tokens:start -->\n${table}\n<!-- tokens:end -->`));
}
