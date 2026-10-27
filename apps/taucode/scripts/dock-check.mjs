import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export async function checkCodeDock(page, { width, height, open }) {
  const savedPanels = await page.evaluate(() => Object.fromEntries(Object.entries(localStorage).filter(([key]) => key.startsWith('taucode.panel.code:')))).catch(() => ({}));
  await open({ mode: 'code', panel: 'browser', theme: 'light' });
  await page.locator('.dock-stage').waitFor();
  const slot = page.locator('.browser-native-slot');
  await slot.waitFor();
  await slot.evaluate(element => { element.dataset.dockContinuity = 'retained'; });
  const geometry = async () => {
    await page.waitForFunction(() => {
      const shell = document.querySelector('.dock-workspace'), stage = document.querySelector('.dock-stage');
      const main = document.querySelector('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)');
      if (!shell || !stage || !main) return false;
      const r = stage.getBoundingClientRect(), css = getComputedStyle(shell), m = main.getBoundingClientRect();
      const x = Number(css.getPropertyValue('--dock-conversation-x')), y = parseFloat(css.getPropertyValue('--dock-conversation-y')) / 100;
      const w = Number(css.getPropertyValue('--dock-conversation-width')), h = parseFloat(css.getPropertyValue('--dock-conversation-height')) / 100;
      return Math.abs(m.left - r.left - r.width * x) < 1 && Math.abs(m.top - r.top - r.height * y) < 1 && Math.abs(m.width - r.width * w) < 1 && Math.abs(m.height - r.height * h) < 1;
    });
    const errors = await page.evaluate(() => {
      const rect = element => element.getBoundingClientRect();
      const shell = document.querySelector('.dock-workspace'), stage = rect(document.querySelector('.dock-stage')), body = rect(document.querySelector('.workspace-body'));
      const rail = document.querySelector('.workspace-main .tablet-nav'), sidebar = document.querySelector('.workspace-main .sidebar');
      const expectedLeft = Math.max(body.left, rail ? rect(rail).right : body.left, !shell.classList.contains('sidebar-overlay') && localStorage.getItem('codexmonitor.sidebarCollapsed') !== 'true' && sidebar ? rect(sidebar).right : body.left);
      const errors = [];
      if (Math.abs(stage.left - expectedLeft) > 1 || Math.abs(stage.right - body.right) > 1) errors.push({ stage: stage.toJSON(), expectedLeft, body: body.toJSON() });
      for (const pane of document.querySelectorAll('.dock-pane:not([hidden])')) {
        const actual = rect(pane), style = pane.style;
        const expected = { x: stage.left + stage.width * parseFloat(style.left) / 100, y: stage.top + stage.height * parseFloat(style.top) / 100, w: stage.width * parseFloat(style.width) / 100, h: stage.height * parseFloat(style.height) / 100 };
        if (Math.abs(actual.x - expected.x) > 1 || Math.abs(actual.y - expected.y) > 1 || Math.abs(actual.width - expected.w) > 1 || Math.abs(actual.height - expected.h) > 1) errors.push({ id: pane.dataset.dockPane, actual: actual.toJSON(), expected });
      }
      return errors;
    });
    assert.deepEqual(errors, [], `Dock geometry ${width}x${height}`);
    assert.equal(await slot.getAttribute('data-dock-continuity'), 'retained', 'Browser remounted');
  };
  await geometry();
  await page.getByRole('button', { name: 'Toggle Terminal', exact: true }).click();
  const terminal = page.locator('[data-dock-pane=terminal]');
  await terminal.waitFor();
  await terminal.evaluate(element => { element.dataset.dockContinuity = 'retained'; });
  await geometry();
  if (await page.locator('.dock-workspace.is-compact').count()) { await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click(); await page.locator('.dock-workspace:not(.is-compact)').waitFor(); await geometry(); }
  const divider = page.locator('.dock-divider').first(), before = Number(await divider.getAttribute('aria-valuenow')), bounds = await divider.boundingBox();
  const x = bounds.x + bounds.width / 2, y = bounds.y + bounds.height / 4;
  const dividerHit = await divider.evaluate(element => { const r = element.getBoundingClientRect(), hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 4); return { owns: element.contains(hit), rect: r.toJSON(), hit: hit?.outerHTML.slice(0, 300), z: getComputedStyle(element).zIndex, pointer: getComputedStyle(element).pointerEvents }; });
  assert(dividerHit.owns, `Dock divider center is covered: ${JSON.stringify(dividerHit)}`);
  await page.mouse.move(x, y); await page.mouse.down();
  try { await page.mouse.move(x + 24, y, { steps: 4 }); await page.waitForFunction(before => Number(document.querySelector('.dock-divider').getAttribute('aria-valuenow')) !== before, before); }
  finally { await page.mouse.up(); }
  await geometry();
  await page.getByRole('button', { name: 'Expand Browser', exact: true }).click({ button: 'right' });
  for (const name of ['Move left', 'Move right', 'Move top', 'Move bottom', 'Fill', 'Tile']) assert.equal(await page.getByRole('menuitem', { name, exact: true }).count(), 1, `Missing pane menu: ${name}`);
  const conversationBeforeMove = await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').boundingBox();
  const railBeforeMove = await page.locator('[data-dock-pane=session]').boundingBox();
  await page.getByRole('menuitem', { name: 'Move top', exact: true }).click(); await geometry();
  assert.deepEqual(await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').boundingBox(), conversationBeforeMove, 'Pane menu move resized the conversation');
  assert.deepEqual(await page.locator('[data-dock-pane=session]').boundingBox(), railBeforeMove, 'Pane menu move resized the session rail');
  await page.setViewportSize({ width: 720, height });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.locator('.dock-workspace.is-compact').waitFor(); await geometry();
  assert.equal(await page.locator('.dock-pane:not([hidden])').count(), 1, 'Compact Code must show one focused pane');
  assert.equal(await page.locator('[data-dock-pane=browser]').evaluate(element => element.style.width), '100%');
  await page.getByRole('button', { name: 'Show conversation', exact: true }).click();
  assert.equal(await page.locator('.dock-pane:not([hidden])').count(), 0, 'Conversation must be accessible in compact mode');
  assert(await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').evaluate(element => !element.hasAttribute('inert') && getComputedStyle(element).visibility !== 'hidden'));
  await page.getByRole('button', { name: 'Toggle Browser', exact: true }).click(); await geometry();
  await page.setViewportSize({ width, height });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.locator('.dock-workspace:not(.is-compact)').waitFor(); await geometry();
  assert(await page.locator('.dock-pane:not([hidden])').count() >= 3, 'Widening must restore the saved tiled panes');
  await page.getByRole('button', { name: 'Expand Browser', exact: true }).click();
  assert(await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').evaluate(element => element.hasAttribute('inert') && getComputedStyle(element).visibility === 'hidden'), 'Filled browser must exclude conversation from focus');
  try {
    for (const size of [width, 900, 740, 360, width]) {
      await page.setViewportSize({ width: size, height });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const toggle = page.locator('.workspace-titlebar .mode-sidebar-toggle');
      if (await toggle.getAttribute('aria-label') === 'Collapse sidebar') await toggle.click();
      await geometry();
      if (!await page.locator('.app.layout-phone').count()) {
        await toggle.click(); await geometry();
        await page.waitForFunction(() => { const sidebar = document.querySelector('.workspace-main .sidebar'); return sidebar && getComputedStyle(sidebar).pointerEvents !== 'none' && Number(getComputedStyle(sidebar).opacity) > .99; });
        const sidebarHit = await page.locator('.workspace-main .sidebar').first().evaluate(element => {
          const r = element.getBoundingClientRect(), hit = document.elementFromPoint(r.left + Math.min(30, r.width / 2), r.top + Math.min(80, r.height / 2));
          return { owns: element.contains(hit) && !element.closest('[inert]'), rect: r.toJSON(), hit: hit?.outerHTML.slice(0, 200), ancestors: [element, element.parentElement, element.parentElement.parentElement].map(el => [el.className, getComputedStyle(el).zIndex, getComputedStyle(el).isolation, getComputedStyle(el).position]) };
        });
        assert(sidebarHit.owns, `Sidebar is covered at ${size}px: ${JSON.stringify(sidebarHit)}`);
      }
    }
    if (await page.locator('.dock-workspace.is-compact').count()) await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click();
    await page.getByRole('button', { name: 'Restore Browser', exact: true }).click(); await geometry();
    assert.equal(await terminal.getAttribute('data-dock-continuity'), 'retained', 'Terminal remounted');
    await page.getByRole('button', { name: 'Close Terminal pane', exact: true }).click();
    await page.getByRole('button', { name: 'Toggle Terminal', exact: true }).click(); await geometry();
    assert.equal(await terminal.getAttribute('data-dock-continuity'), 'retained', 'Terminal remounted after reopening');
    console.log(`PASS Code dock geometry, drag, move/fill/tile and continuity ${width}x${height}`);
  } finally {
    await page.setViewportSize({ width, height });
    await page.evaluate(saved => { for (const key of Object.keys(localStorage)) if (key.startsWith('taucode.panel.code:')) localStorage.removeItem(key); for (const [key, value] of Object.entries(saved)) localStorage.setItem(key, value); }, savedPanels);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const require = createRequire(new URL('../engines/browser/package.json', import.meta.url));
  const browser = await require('playwright').webkit.launch();
  try {
    for (const width of [1728, 1280, 1440, 1024]) {
      const height = 720, page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' });
      await checkCodeDock(page, { width, height, open: async params => {
        const url = new URL(process.argv[2] ?? 'http://localhost:1421'); url.search = new URLSearchParams(params).toString();
        await page.goto(url.href); await page.locator('.dock-workspace').waitFor(); await page.evaluate(() => document.fonts.ready);
      } });
      await page.close();
    }
  } finally { await browser.close(); }
}
