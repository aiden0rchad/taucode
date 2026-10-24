import { expect, it } from "vitest";

const styles = Object.values(import.meta.glob<string>("../**/*.css", { query: "?raw", import: "default", eager: true })).join("\n");
const sources = import.meta.glob<string>("../**/*.tsx", { query: "?raw", import: "default", eager: true });

it("defines every referenced CSS variable", () => {
  const definitions = new Set([...styles.matchAll(/(--[\w-]+)\s*:/g)].map(match => match[1]));
  const references = new Set([...styles.matchAll(/var\(\s*(--[\w-]+)/g)].map(match => match[1]));
  expect([...references].filter(name => !definitions.has(name)).sort()).toEqual([]);
});

it("keeps opaque surfaces and uses Lucide instead of emoji or glyph icons", () => {
  expect(styles).not.toMatch(/backdrop-filter\s*:/);
  const offenders = Object.entries(sources).filter(([file, source]) => !file.includes(".test.") && /[▶☰]|[\u{1F300}-\u{1FAFF}]/u.test(source)).map(([file]) => file);
  expect(offenders).toEqual([]);
});

it("uses the shared composer in every mode", () => {
  for (const path of ["../features/composer/components/ComposerInput.tsx", "../features/chat/ChatView.tsx", "../features/work/WorkView.tsx"]) expect(sources[path]).toContain("<ComposerShell");
});

it.each(["▶", "☰"])("rejects the %s text icon in TSX", glyph => {
  expect(Object.entries(sources).filter(([file, source]) => !file.includes(".test.") && source.includes(glyph)).map(([file]) => file)).toEqual([]);
});
