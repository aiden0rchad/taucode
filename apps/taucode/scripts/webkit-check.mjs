import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const require = createRequire(new URL('../engines/browser/package.json', import.meta.url));
const { webkit } = require('playwright');
const base = new URL(process.argv[2] ?? 'http://localhost:1421');
const out = resolve(process.argv[3] ?? '.tmp/webkit');
const source = await readFile(new URL('../src/features/themes/validate.ts', import.meta.url), 'utf8');
const ids = [...source.match(/builtinIds = \[([^\]]+)\]/)[1].matchAll(/'([a-z0-9-]+)'/g)].map(match => match[1]);
assert(ids.length > 0, 'No built-in themes found');
await mkdir(out, { recursive: true });
const browser = await webkit.launch();
const failures = [];
try {
  for (const [width, height] of [[1440, 900], [1024, 700]]) {
    const page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' });
    const open = async params => {
      const url = new URL(base);
      url.search = new URLSearchParams({ mode: 'code', ...params }).toString();
      await page.goto(url.href);
      await page.locator('.app').waitFor();
      if (params.theme) await page.waitForFunction(id => document.documentElement.dataset.themeId === id, params.theme);
      await page.evaluate(() => document.fonts.ready);
    };
    const centered = async (name, selector) => {
      const card = page.locator(selector);
      await card.waitFor({ state: 'visible' });
      await card.evaluate(async element => {
        await Promise.all(element.getAnimations().filter(a => a.effect?.getComputedTiming().iterations !== Infinity).map(a => a.finished.catch(() => {})));
      });
      const rect = await card.boundingBox();
      const details = `${name} ${width}x${height}: ${JSON.stringify(rect)}`;
      assert(rect && rect.width > 0 && rect.height > 0, details);
      assert(Math.abs(rect.x + rect.width / 2 - width / 2) <= 2 && Math.abs(rect.y + rect.height / 2 - height / 2) <= 2, `Not centered: ${details}`);
      assert(rect.x >= 0 && rect.y >= 0 && rect.x + rect.width <= width && rect.y + rect.height <= height, `Outside viewport: ${details}`);
      console.log(`PASS ${details}`);
    };
    const check = async (label, fn) => {
      try { await fn(); } catch (error) {
        failures.push(`${label} ${width}x${height}: ${error.message}`);
        console.error(failures.at(-1));
      }
    };
    await check('Settings', async () => {
      await open({ settings: 'appearance', theme: 'dark' });
      await centered('Settings', '.settings-window');
    });
    await check('Theme editor', async () => {
      await open({ settings: 'appearance', theme: 'dark' });
      await page.getByRole('button', { name: 'Actions for Dark', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Duplicate & edit', exact: true }).click();
      await centered('Theme editor', '.theme-editor-card');
    });
    await check('Add engine', async () => {
      await open({ settings: 'engines', theme: 'dark' });
      await page.getByRole('button', { name: 'Add engine', exact: true }).click();
      await centered('Add engine', '.engine-dialog-card');
    });
    for (const theme of ids) await check(theme, async () => {
      await open({ theme });
      await page.screenshot({ path: resolve(out, `${theme}-${width}x${height}.png`), animations: 'disabled' });
    });
    await page.close();
  }
} finally { await browser.close(); }
if (failures.length) {
  console.error(`${failures.length} WebKit checks failed.`);
  process.exitCode = 1;
}
