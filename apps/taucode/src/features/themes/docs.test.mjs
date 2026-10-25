import { it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { table, tokens, effectsTable, shapeTable } from '../../../scripts/themes-docs.mjs';
it('keeps the token reference, allowlist and schema up to date', () => {
  expect(readFileSync('../../docs/themes.md','utf8')).toContain(table);
  expect(readFileSync('../../docs/themes.md','utf8')).toContain(effectsTable);
  expect(readFileSync('../../docs/themes.md','utf8')).toContain(shapeTable);
  expect(Object.keys(JSON.parse(readFileSync('../../docs/theme.schema.json','utf8')).properties.effects.properties)).toEqual(['digitalRain', 'neonGlow']);
  expect(JSON.parse(readFileSync('src/features/themes/tokens.json','utf8'))).toEqual(tokens);
  const schema = JSON.parse(readFileSync('../../docs/theme.schema.json','utf8'));
  expect(Object.keys(schema.properties.tokens.properties).sort()).toEqual(Object.keys(tokens).sort());
});
