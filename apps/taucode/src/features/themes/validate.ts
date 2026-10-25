import { themeTokens } from './tokens';
export const builtinIds = ['light', 'dark', 'dim', 'glass-dark', 'glass-light', 'matrix', 'claude-light', 'claude-dark'];
export type ThemeManifest = {
  $schema?: string; schemaVersion: 1; id: string; name: string; author?: string; version?: string; description?: string;
  appearance: 'light' | 'dark'; extends?: string; tokens: Record<string, string>;
  fonts?: { ui?: string; mono?: string; prose?: string };
  window?: { translucent: boolean; material: 'none' | 'sidebar' | 'underWindow' | 'hudWindow' };
  effects?: { digitalRain: boolean };
};
export type ThemeFile = { file: string; manifest?: ThemeManifest; error?: string };
const safe = (s: string) => s.length <= 120 && !/url\(|image-set\(|expression|[@;{}\\<]/i.test(s);
export function validFont(s: string) {
  return safe(s) && s.split(',').every(p => /^(?:"[a-zA-Z0-9 -]+"|'[a-zA-Z0-9 -]+'|serif|sans-serif|monospace|system-ui|ui-serif|ui-sans-serif|ui-monospace|ui-rounded|cursive|fantasy)$/.test(p.trim()));
}
function split(s: string) {
  let depth = 0, start = 0;
  const parts: string[] = [];
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '(') depth++;
    if (s[i] === ')') depth--;
    if (depth < 0) return [];
    if (s[i] === ',' && !depth) { parts.push(s.slice(start, i).trim()); start = i + 1; }
  }
  return depth ? [] : [...parts, s.slice(start).trim()];
}
export function validColor(s: string): boolean {
  if (!safe(s)) return false;
  if (/^#(?:[\da-f]{3,4}|[\da-f]{6}|[\da-f]{8})$/i.test(s)) return true;
  const ref = /^var\((--[a-z0-9-]+)\)$/.exec(s);
  if (ref) return Object.prototype.hasOwnProperty.call(themeTokens, ref[1]);
  const fn = /^(rgb|rgba|hsl|hsla|oklch)\((.+)\)$/.exec(s);
  if (fn) {
    const body = fn[2].trim(), legacy = body.includes(',');
    if (legacy && (fn[1] === 'oklch' || body.includes('/'))) return false;
    const alpha = body.split('/');
    if (alpha.length > 2) return false;
    const parts = legacy ? body.split(',').map(v => v.trim()) : alpha[0].trim().split(/\s+/);
    if (legacy ? ![3,4].includes(parts.length) : parts.length !== 3) return false;
    if (!legacy && alpha.length === 2) parts.push(alpha[1].trim());
    if (fn[1].startsWith('hsl') && (!parts[1].endsWith('%') || !parts[2].endsWith('%'))) return false;
    return parts.every(v => /^-?(?:\d+(?:\.\d+)?|\.\d+)%?$/.test(v));
  }
  if (s.startsWith('color-mix(') && s.endsWith(')')) {
    const p = split(s.slice(10, -1));
    return p.length === 3 && p[0] === 'in srgb' && p.slice(1).every(v => validColor(v.replace(/\s+(?:\d+(?:\.\d+)?|\.\d+)%$/, '')));
  }
  return false;
}
export function validateTheme(input: unknown): string[] {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return ['Theme must be an object'];
  const t = input as Record<string, unknown>, errors: string[] = [];
  const keys = ['$schema','schemaVersion','id','name','author','version','description','appearance','extends','tokens','fonts','window','effects'];
  for (const key of Object.keys(t)) if (!keys.includes(key)) errors.push(`Unknown field: ${key}`);
  if (t.schemaVersion !== 1) errors.push(typeof t.schemaVersion === 'number' && t.schemaVersion > 1 ? 'Theme made for a newer taucode' : 'schemaVersion must be 1');
  if (typeof t.id !== 'string' || !/^[a-z0-9-]{1,40}$/.test(t.id)) errors.push('id must contain 1–40 lowercase letters, digits or hyphens');
  if (typeof t.name !== 'string' || !t.name.trim() || t.name.length > 120) errors.push('name must contain 1–120 characters');
  for (const k of ['$schema','author','version','description']) if (t[k] !== undefined && (typeof t[k] !== 'string' || (t[k] as string).length > 1000)) errors.push(`${k} must be text (at most 1000 characters)`);
  if (t.appearance !== 'dark' && t.appearance !== 'light') errors.push('appearance must be light or dark');
  if (t.extends !== undefined && !builtinIds.includes(t.extends as string)) errors.push('extends must name a built-in theme');
  const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
  if (!object(t.tokens)) errors.push('tokens must be an object');
  else {
    for (const [key, v] of Object.entries(t.tokens)) {
      if (!Object.prototype.hasOwnProperty.call(themeTokens, key)) errors.push(`Unknown token: ${key}`);
      else if (typeof v !== 'string' || !(key.includes('radius') ? /^(?:\d+(?:\.\d+)?|\.\d+)(px|rem)$/.test(v) && safe(v) : validColor(v))) errors.push(`Invalid value for ${key}`);
    }
    const values = t.tokens;
    const visit = (key: string, path: string[]): boolean => path.includes(key) || (typeof values[key] === 'string' && [...values[key].matchAll(/var\((--[a-z0-9-]+)\)/g)].some(m => visit(m[1], [...path, key])));
    if (Object.keys(values).some(k => visit(k, []))) errors.push('Token references contain a cycle');
  }
  if (t.fonts !== undefined && (!object(t.fonts) || Object.entries(t.fonts).some(([k,v]) => !['ui','mono','prose'].includes(k) || typeof v !== 'string' || !validFont(v)))) errors.push('Invalid local font stack');
  if (t.window !== undefined && (!object(t.window) || Object.keys(t.window).some(k => !['translucent','material'].includes(k)) || typeof t.window.translucent !== 'boolean' || !['none','sidebar','underWindow','hudWindow'].includes(t.window.material as string))) errors.push('Invalid window material');
  if (t.effects !== undefined && (!object(t.effects) || Object.keys(t.effects).some(k => k !== 'digitalRain') || typeof t.effects.digitalRain !== 'boolean')) errors.push('Invalid effects');
  return errors;
}
export function parseTheme(json: string): ThemeManifest {
  const value: unknown = JSON.parse(json), errors = validateTheme(value);
  if (errors.length) throw new Error(errors.join('\n'));
  return value as ThemeManifest;
}
