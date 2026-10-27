import assert from 'node:assert/strict';
import { mkdir, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { assertRowAlignment } from '../src/test/rowAlignment.mjs';
import { checkCodeDock } from './dock-check.mjs';

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
      // Baseline Code checks inspect the conversation; dock checks opt in via panel.
      if ((params.mode ?? 'code') === 'code' && !params.panel) await page.evaluate(() => {
        localStorage.setItem('codexmonitor.sidebarCollapsed', 'false');
        for (const key of Object.keys(localStorage).filter(key => key.startsWith('taucode.panel.code:'))) {
          const state = JSON.parse(localStorage.getItem(key));
          if (state?.dock) localStorage.setItem(key, JSON.stringify({ ...state, open: false, dock: { ...state.dock, expanded: null, focused: 'conversation' } }));
        }
      }).catch(() => {});
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
      if (await toggle.count()) { if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click(); }
      else if (await page.getByRole('button', { name: 'Toggle Files', exact: true }).count()) await page.getByRole('button', { name: 'Toggle Files', exact: true }).click();
      const result = await page.locator('body').evaluate(assertRowAlignment);
      assert(result.rows > 0 && result.icons > 0 && result.unmeasuredRows === 0 && result.unmeasuredIcons === 0, `Missing geometry: ${JSON.stringify(result)}`);
      assert(await page.locator('.workspace-titlebar, .main-header').getByRole('button', { name: 'Browser', exact: true }).count() === 0, 'Redundant Browser header button');
      console.log(`PASS ${label} row alignment ${width}x${height}: ${JSON.stringify(result)}`);
    });
    for (const mode of ['code', 'chat', 'work']) await check(`${mode} composer controls`, async () => {
      await open({ mode, task: 'new' });
      const composer = page.locator('.composer-shell').first();
      await composer.waitFor();
      for (const size of [300, 420, 470, 560, 680]) {
        await composer.evaluate((element, width) => { element.style.width = `${width}px`; element.style.maxWidth = 'none'; }, size);
        const overflow = await composer.evaluate(element => {
          const box = element.getBoundingClientRect();
          return [...element.querySelectorAll('.composer-shell-toolbar button, .composer-shell-toolbar select')].filter(control => {
            const r = control.getBoundingClientRect();
            return r.width > 0 && r.height > 0 && (r.left < box.left || r.right > box.right + 1 || r.top < box.top || r.bottom > box.bottom + 1);
          }).map(control => control.getAttribute('aria-label') || control.textContent);
        });
        assert.deepEqual(overflow, [], `${mode} composer ${size}px`);
      }
    });
    await check('Native browser toolbar', async () => {
      await open({ panel: 'browser' });
      await page.locator('.browser-native-slot').waitFor();
      const geometry = await page.locator('.browser-toolbar').evaluate(row => {
        const r = row.getBoundingClientRect();
        return { height: r.height, controls: [...row.querySelectorAll(':scope > button, .browser-address, .mode-row-menu-trigger')].map(el => {
          const b = el.getBoundingClientRect();
          return { label: el.getAttribute('aria-label') || el.className, delta: Math.abs(b.y + b.height / 2 - r.y - r.height / 2), height: b.height };
        }) };
      });
      assert.equal(geometry.height, 36);
      for (const control of geometry.controls) assert(control.delta <= .5, JSON.stringify(control));
      assert(geometry.controls.every(c => c.height === 28 || c.height === 30), JSON.stringify(geometry));
    });
    for (const mode of ['code', 'chat', 'work']) await check(`${mode} expanded side panel`, async () => {
      if (mode === 'code') { await checkCodeDock(page, { width, height, open }); return; }
      await page.evaluate(() => {
        localStorage.setItem('codexmonitor.sidebarWidth', '320');
        localStorage.setItem('codexmonitor.sidebarCollapsed', 'false');
        localStorage.setItem('codexmonitor.rightPanelWidth', '420');
      });
      await open({ mode, panel: 'browser', task: 'new', theme: 'light' });
      const panel = page.getByRole('complementary', { name: 'Side panel', exact: true });
      const slot = page.locator('.browser-native-slot');
      await slot.waitFor();
      await slot.evaluate(element => { element.dataset.expansionContinuity = 'retained'; });
      const selected = await page.locator('.side-panel-tabs [role=tab][aria-selected=true]').textContent();
      const normal = await page.locator('.tabbed-side-panel').boundingBox();
      const preferred = await page.evaluate(() => localStorage.getItem('codexmonitor.rightPanelWidth'));
      await page.getByRole('button', { name: 'Expand side panel', exact: true }).click();
      const geometry = async label => {
        await page.waitForFunction(() => {
          const body = document.querySelector('.workspace-body').getBoundingClientRect();
          const shell = document.querySelector('.workspace-shell');
          const panel = document.querySelector('.tabbed-side-panel').getBoundingClientRect();
          const rail = document.querySelector('.workspace-main .tablet-nav')?.getBoundingClientRect();
          const sidebar = document.querySelector('.workspace-main .sidebar')?.getBoundingClientRect();
          const sidebarVisible = localStorage.getItem('codexmonitor.sidebarCollapsed') !== 'true';
          const left = Math.max(body.left, rail?.right ?? body.left, !shell.classList.contains('sidebar-overlay') && sidebarVisible ? sidebar?.right ?? body.left : body.left);
          return Math.abs(panel.left - left) <= 1 && Math.abs(panel.right - body.right) <= 1 && Math.abs(panel.top - body.top) <= 1 && Math.abs(panel.bottom - body.bottom) <= 1;
        });
        assert.equal(await panel.count(), 1, `${label}: expanded panel must remain nonmodal`);
        assert.equal(await page.getByRole('dialog', { name: 'Side panel' }).count(), 0, `${label}: unexpected dialog`);
        assert.equal(await page.getByRole('button', { name: 'Dismiss side panel' }).count(), 0, `${label}: unexpected scrim`);
        assert.equal(await page.getByRole('separator', { name: 'Resize side panel' }).count(), 0, `${label}: expanded resizer`);
        assert.equal(await slot.getAttribute('data-expansion-continuity'), 'retained', `${label}: native slot remounted`);
        assert.equal(await page.locator('.side-panel-tabs [role=tab][aria-selected=true]').textContent(), selected, `${label}: active tab changed`);
        assert.equal(await page.evaluate(() => localStorage.getItem('codexmonitor.rightPanelWidth')), preferred, `${label}: width preference changed`);
        const hiddenConversation = await page.locator('.workspace-main .composer-shell textarea').first().evaluate(element => {
          element.focus(); return Boolean(element.closest('[inert]')) && document.activeElement !== element;
        });
        assert(hiddenConversation, `${label}: covered conversation remains focusable`);
      };
      const sidebarVisible = async visible => {
        const toggle = page.locator('.workspace-titlebar .mode-sidebar-toggle');
        if ((await toggle.getAttribute('aria-label') === 'Collapse sidebar') !== visible) await toggle.click();
      };
      const sidebarUsable = async label => {
        const usable = await page.locator('.workspace-main .sidebar').first().evaluate(element => {
          const r = element.getBoundingClientRect();
          const target = document.elementFromPoint(r.left + Math.min(30, r.width / 2), r.top + Math.min(80, r.height / 2));
          const button = [...element.querySelectorAll('button:not(:disabled),input:not(:disabled)')].find(control => control.getClientRects().length);
          button?.focus();
          return r.width > 0 && !element.closest('[inert]') && element.contains(target) && (!button || document.activeElement === button);
        });
        assert(usable, `${label}: sidebar is covered or cannot receive focus`);
      };
      try {
        await sidebarVisible(true);
        const sidebarResize = page.getByRole('separator', { name: 'Resize sidebar', exact: true });
        if (await sidebarResize.count()) await sidebarResize.press('ArrowRight');
        else await page.evaluate(() => { localStorage.setItem('codexmonitor.sidebarWidth', '330'); window.dispatchEvent(new Event('taucode:sidebar')); });
        await geometry('resized sidebar'); await sidebarUsable('resized sidebar');
        const divider = page.locator('.workspace-main .sidebar-resizer:visible, .workspace-main .projects-resizer:visible').first();
        const originalSidebarWidth = await page.evaluate(() => Number(localStorage.getItem('codexmonitor.sidebarWidth')));
        const originalPanelLeft = (await panel.boundingBox()).x;
        for (const delta of [24, -24]) {
          const hit = await divider.evaluate(element => {
            const r = element.getBoundingClientRect();
            const x = r.left + r.width / 2, y = r.top + r.height / 2;
            const target = document.elementFromPoint(x, y);
            return { x, y, ownsCenter: element.contains(target), target: target?.className };
          });
          assert(hit.ownsCenter, `Expanded panel covers sidebar divider center: ${JSON.stringify(hit)}`);
          const sidebarWidth = await page.evaluate(() => Number(localStorage.getItem('codexmonitor.sidebarWidth')));
          const panelLeft = (await panel.boundingBox()).x;
          await page.mouse.move(hit.x, hit.y); await page.mouse.down();
          try {
            await page.mouse.move(hit.x + delta, hit.y, { steps: 4 });
            await page.waitForFunction(({ expectedWidth, expectedLeft }) =>
              Number(localStorage.getItem('codexmonitor.sidebarWidth')) === expectedWidth &&
              Math.abs(document.querySelector('.tabbed-side-panel').getBoundingClientRect().left - expectedLeft) <= 1,
            { expectedWidth: sidebarWidth + delta, expectedLeft: panelLeft + delta });
          } finally { await page.mouse.up(); }
          await geometry(`sidebar pointer drag ${delta}px`);
        }
        assert.equal(await page.evaluate(() => Number(localStorage.getItem('codexmonitor.sidebarWidth'))), originalSidebarWidth, 'Pointer drag did not restore the sidebar preference');
        assert(Math.abs((await panel.boundingBox()).x - originalPanelLeft) <= 1, 'Pointer drag did not restore the expanded panel boundary');
        for (const size of [width, 900, 740, 360, width]) {
          await page.setViewportSize({ width: size, height });
          await sidebarVisible(false); await geometry(`${size}px collapsed sidebar`);
          if (await page.locator('.app.layout-phone').count()) {
            assert(await page.locator('.compact-shell').evaluate(element => element.hasAttribute('inert') && getComputedStyle(element).visibility === 'hidden'), 'Phone conversation and Projects navigation must leave the focus order until contraction');
          } else {
            await sidebarVisible(true); await geometry(`${size}px visible sidebar`); await sidebarUsable(`${size}px`);
          }
        }
        await page.getByRole('button', { name: 'Contract side panel', exact: true }).click();
        await page.getByRole('separator', { name: 'Resize side panel' }).waitFor();
        await page.waitForFunction(expected => Math.abs(document.querySelector('.tabbed-side-panel').getBoundingClientRect().width - expected) <= 1, normal.width);
        const restored = await page.locator('.tabbed-side-panel').boundingBox();
        assert(Math.abs(restored.width - normal.width) <= 1, `Normal width changed: ${JSON.stringify({ normal, restored })}`);
        assert.equal(await slot.getAttribute('data-expansion-continuity'), 'retained', 'Native slot remounted on contraction');
        assert.equal(await page.locator('.side-panel-tabs [role=tab][aria-selected=true]').textContent(), selected);
        assert.equal(await page.evaluate(() => localStorage.getItem('codexmonitor.rightPanelWidth')), preferred);
      } finally { await page.setViewportSize({ width, height }); }
    });
    await check('Settings', async () => {
      await open({ settings: 'appearance', theme: 'dark' });
      const rect = await centered('Settings', '.settings-window');
      assert(Math.abs(rect.width - Math.min(1180, width * .94)) <= 2 && Math.abs(rect.height - Math.min(900, height * .92)) <= 2, `Settings size: ${JSON.stringify(rect)}`);
    });
    await check('Setup theme previews', async () => {
      await open({ setup: '1' });
      await page.getByRole('button', { name: 'Get started', exact: true }).click();
      await page.locator('.setup-theme .theme-mini').first().waitFor();
      const clipped = await page.locator('.setup-theme .theme-mini').evaluateAll(minis => minis.flatMap(mini => {
        const box = mini.getBoundingClientRect();
        return [...mini.querySelectorAll('.theme-mini-message, .theme-mini-prose, .theme-mini-button')].map(el => [el.className, el.getBoundingClientRect()]).filter(([, r]) => r.left < box.left || r.right > box.right || r.top < box.top || r.bottom > box.bottom).map(([name]) => name);
      }));
      assert.deepEqual(clipped, []);
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
