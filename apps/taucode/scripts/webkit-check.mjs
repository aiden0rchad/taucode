import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { assertRowAlignment } from '../src/test/rowAlignment.mjs';

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
      const root = { code: '.app', chat: '.chat-layout:not(.work-layout)', work: '.work-layout' }[params.mode ?? 'code'];
      await page.locator(root).waitFor();
      if (params.theme) await page.waitForFunction(id => document.documentElement.dataset.themeId === id, params.theme);
      await page.evaluate(() => document.fonts.ready);
      await page.mouse.move(0, 0);
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
      return rect;
    };
    const check = async (label, fn) => {
      try { await fn(); } catch (error) {
        failures.push(`${label} ${width}x${height}: ${error.message}`);
        console.error(failures.at(-1));
      }
    };
    for (const [label, params] of [
      ['Code thread', { mode: 'code' }],
      ['Code project home', { mode: 'code', thread: 'home' }],
      ['Chat', { mode: 'chat' }],
      ['Work composer', { mode: 'work', task: 'new' }],
      ['Work task', { mode: 'work', task: 'task-needs-input' }],
    ]) await check(`${label} row alignment`, async () => {
      await open({ ...params, theme: 'glass-dark' });
      if (params.mode === 'chat') await page.locator('.chat-layout:not(.work-layout) .chat-list .thread-row').first().waitFor();
      if (params.thread === 'home') await page.locator('.sidebar .workspace-row').first().click();
      if (params.task !== 'task-needs-input') await page.locator('.composer-shell-toolbar').first().waitFor();
      const toggle = page.getByRole('button', { name: 'Toggle side panel', exact: true });
      if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click();
      const result = await page.locator('body').evaluate(assertRowAlignment);
      assert(result.rows > 0 && result.icons > 0 && result.unmeasuredRows === 0 && result.unmeasuredIcons === 0, `Missing geometry: ${JSON.stringify(result)}`);
      assert(await page.locator('.workspace-titlebar, .main-header').getByRole('button', { name: 'Browser', exact: true }).count() === 0, 'Redundant Browser header button');
      console.log(`PASS ${label} row alignment ${width}x${height}: ${JSON.stringify(result)}`);
    });
    await check('Settings', async () => {
      await open({ settings: 'appearance', theme: 'dark' });
      const rect = await centered('Settings', '.settings-window');
      assert(Math.abs(rect.width - Math.min(1180, width * .94)) <= 2 && Math.abs(rect.height - Math.min(900, height * .92)) <= 2, `Settings size: ${JSON.stringify(rect)}`);
    });
    await check('Theme editor', async () => {
      await open({ settings: 'appearance', theme: 'dark' });
      await page.getByRole('button', { name: 'Actions for Dark', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Duplicate & edit', exact: true }).click();
      await centered('Theme editor', '.theme-editor-card');
    });
    for (const [theme, name] of [['glass-dark', 'Glass Dark'], ['cyberpunk-neon', 'Cyberpunk Neon']]) await check(`${name} live preview`, async () => {
      await open({ settings: 'appearance', theme });
      const settings = await centered('Settings', '.settings-window');
      assert(Math.abs(settings.width - Math.min(1180, width * .94)) <= 2 && Math.abs(settings.height - Math.min(900, height * .92)) <= 2, `Settings size: ${JSON.stringify(settings)}`);
      await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
      await page.getByRole('menuitem', { name: 'Duplicate & edit', exact: true }).click();
      await centered(`${name} editor`, '.theme-editor-card');
      const preview = page.getByTestId('theme-preview');
      await preview.waitFor({ state: 'visible' });
      const [outer, sidebar, main] = await Promise.all([
        preview.boundingBox(), preview.locator('.theme-preview-sidebar').boundingBox(), preview.locator('.theme-preview-main').boundingBox(),
      ]);
      const details = JSON.stringify({ outer, sidebar, main });
      assert(outer && sidebar && main && outer.width > 0, details);
      assert(sidebar.width / outer.width >= .25 && sidebar.width / outer.width <= .31, `Preview sidebar width: ${details}`);
      assert(main.x >= sidebar.x + sidebar.width, `Preview columns overlap: ${details}`);
      assert(Math.abs(main.x + main.width - outer.x - outer.width) <= 2, `Preview main right edge: ${details}`);
      await page.screenshot({ path: resolve(out, `${theme}-editor-${width}x${height}.png`), animations: 'disabled' });
    });
    await check('Add engine', async () => {
      await open({ settings: 'engines', theme: 'dark' });
      await page.getByRole('button', { name: 'Add engine', exact: true }).click();
      await centered('Add engine', '.engine-dialog-card');
    });
    for (const theme of ids) await check(theme, async () => {
      await open({ theme });
      const composer = page.locator('.composer-shell').first();
      await composer.waitFor({ state: 'visible' });
      const composerRect = await composer.boundingBox();
      assert(composerRect && composerRect.height > 0 && composerRect.width >= (width === 1440 ? 400 : 1), `Composer size: ${JSON.stringify(composerRect)}`);
      const project = page.locator('.sidebar .project-group-card').first();
      await project.waitFor({ state: 'visible' });
      const geometry = await project.evaluate(element => {
        const body = element.closest('.sidebar-body');
        const css = getComputedStyle(body), rect = body.getBoundingClientRect();
        const left = rect.x + parseFloat(css.borderLeftWidth) + parseFloat(css.paddingLeft);
        return { card: element.getBoundingClientRect().toJSON(), left, width: body.clientWidth - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight) };
      });
      assert(geometry.card.height > 0 && Math.abs(geometry.card.x - geometry.left) <= 2 && Math.abs(geometry.card.width - geometry.width) <= 2, `Project must fill padded sidebar: ${JSON.stringify(geometry)}`);
      await page.screenshot({ path: resolve(out, `${theme}-${width}x${height}.png`), animations: 'disabled' });
    });
    for (const theme of ['glass-dark', 'cyberpunk-neon', 'dark', 'light']) {
      for (const sidebarWidth of [220, 280, 420]) {
        for (const density of ['compact', 'comfortable']) await check(`Work ${theme} ${sidebarWidth}px ${density}`, async () => {
          await open({ mode: 'work', theme, task: 'task-needs-input' });
          await page.locator('.work-task-row').first().waitFor();
          await page.evaluate(({ sidebarWidth, density }) => {
            const layout = document.querySelector('.work-layout');
            layout.style.setProperty('--sidebar-width', `${sidebarWidth}px`);
            layout.style.setProperty('--layout-sidebar-column', `${sidebarWidth}px`);
            document.documentElement.dataset.sidebarDensity = density;
          }, { sidebarWidth, density });
          const geometry = await page.locator('.work-layout .sidebar').evaluate(sidebar => {
            const rect = element => element.getBoundingClientRect().toJSON();
            const body = sidebar.querySelector('.sidebar-body');
            const css = getComputedStyle(body), bounds = body.getBoundingClientRect();
            return {
              sidebar: rect(sidebar),
              body: { left: bounds.x + parseFloat(css.borderLeftWidth) + parseFloat(css.paddingLeft), width: body.clientWidth - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight) },
              cards: [...sidebar.querySelectorAll('.work-space')].map(card => ({
                card: rect(card),
                rows: [...card.querySelectorAll('.work-task-row')].map(row => ({
                  row: rect(row), content: [...row.children].map(rect),
                  metaVisible: row.querySelector('.thread-time') ? getComputedStyle(row.querySelector('.thread-time')).visibility : null,
                  ellipsis: getComputedStyle(row.querySelector('.thread-name')).textOverflow,
                })),
              })),
            };
          });
          const details = JSON.stringify(geometry);
          assert(Math.abs(geometry.sidebar.width - sidebarWidth) <= 1, `Sidebar width: ${details}`);
          assert(geometry.cards.flatMap(card => card.rows).length > 0, 'Missing Work task rows');
          for (const { card, rows } of geometry.cards) {
            assert(card.x > geometry.sidebar.x && Math.abs(card.x - geometry.body.left) <= 1 && Math.abs(card.width - geometry.body.width) <= 1, `Work card must fill the inset sidebar body: ${details}`);
            for (const [i, { row, content, metaVisible, ellipsis }] of rows.entries()) {
              assert(row.height > 0 && row.height <= 34 && Math.abs(row.height - (density === 'compact' ? 28 : 32)) <= 1, `Work row height: ${details}`);
              assert(row.x >= card.x && row.right <= card.right + 1 && row.y >= card.y && row.bottom <= card.bottom + 1, `Work row outside card: ${details}`);
              assert(!i || row.y - rows[i - 1].row.bottom >= 1.5, `Work rows overlap or lack gap: ${details}`);
              assert(content.every(child => child.x >= row.x && child.right <= row.right + 1 && child.y >= row.y && child.bottom <= row.bottom + 1), `Work row content overflow: ${details}`);
              assert((metaVisible === null || metaVisible === 'visible') && ellipsis === 'ellipsis', `Work metadata/title: ${details}`);
            }
          }
          // At narrow widths the side panel opens as an overlay whose scrim would swallow the hover.
          const scrim = page.locator('.side-panel-scrim');
          if (await scrim.count()) await scrim.first().click();
          await page.locator('.work-task-row:has(.thread-time)').first().hover();
          assert(await page.locator('.work-task-row .thread-time').first().evaluate(element => getComputedStyle(element).visibility === 'hidden'), 'Work metadata must share the action slot on hover');
          const action = page.locator('.mode-row:has(.work-task-row:has(.thread-time)) .mode-row-menu-trigger').first();
          assert(await action.evaluate(element => getComputedStyle(element).opacity === '1'), 'Work row menu missing on hover');
          await page.locator('.work-layout').evaluate(assertRowAlignment);
          await page.screenshot({ path: resolve(out, `work-${theme}-${sidebarWidth}-${density}-${width}x${height}.png`), animations: 'disabled' });
          console.log(`PASS Work ${theme} ${sidebarWidth}px ${density} ${width}x${height}`);
        });
      }
    }
    await page.close();
  }
} finally { await browser.close(); }
if (failures.length) {
  console.error(`${failures.length} WebKit checks failed.`);
  process.exitCode = 1;
}
