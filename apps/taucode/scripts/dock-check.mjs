import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export async function checkTabletSidebar(page, open) {
  await open({ mode: 'code', panel: 'browser', theme: 'light' });
  await page.locator('.app.layout-tablet').waitFor();
  const toggle = page.locator('.workspace-titlebar .mode-sidebar-toggle');
  if (await toggle.getAttribute('aria-label') === 'Collapse sidebar') await toggle.click();
  await page.locator('.app.layout-tablet.sidebar-collapsed').waitFor();
  assert.equal(await page.getByRole('separator', { name: 'Resize projects', exact: true }).count(), 0, 'Collapsed tablet project resizer must be excluded from accessibility navigation');
  assert.equal(await page.locator('.tablet-projects').getByRole('button').count(), 0, 'Collapsed tablet projects must exclude their buttons from accessibility navigation');
  assert(await page.locator('.tablet-projects,.projects-resizer').evaluateAll(elements => elements.every(element => getComputedStyle(element).visibility === 'hidden')), 'Collapsed tablet project wrapper and resizer must be hidden');
  await toggle.click();
  await page.locator('.app.layout-tablet:not(.sidebar-collapsed)').waitFor();
  assert.equal(await page.getByRole('separator', { name: 'Resize projects', exact: true }).count(), 1, 'Expanded tablet project resizer must remain accessible');
  assert(await page.locator('.tablet-projects').getByRole('button').count() > 0, 'Expanded tablet projects must restore their controls');
  console.log('PASS Code tablet sidebar accessibility 1024x720 touch');
}

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
      return Math.abs(m.left - r.left - r.width * x) < 1 && Math.abs(m.top - r.top - r.height * y) < 1 && Math.abs(m.width - (r.width * w - parseFloat(css.getPropertyValue("--dock-rail-reserve")))) < 1 && Math.abs(m.height - r.height * h) < 1;
    });
    const errors = await page.evaluate(() => {
      const rect = element => element.getBoundingClientRect();
      const shell = document.querySelector('.dock-workspace'), stage = rect(document.querySelector('.dock-stage')), body = rect(document.querySelector('.workspace-body'));
      const rail = document.querySelector('.workspace-main .tablet-nav'), sidebar = document.querySelector('.workspace-main .sidebar');
      const expectedLeft = Math.max(body.left, rail ? rect(rail).right : body.left, !shell.classList.contains('sidebar-overlay') && localStorage.getItem('codexmonitor.sidebarCollapsed') !== 'true' && sidebar ? rect(sidebar).right : body.left);
      const errors = [];
      if (Math.abs(stage.left - expectedLeft) > 1 || Math.abs(stage.right - body.right) > 1) errors.push({ stage: stage.toJSON(), expectedLeft, body: body.toJSON() });
      for (const pane of document.querySelectorAll('.dock-pane:not(.dock-session-rail):not([hidden])')) {
        const actual = rect(pane), style = pane.style;
        const expected = { x: stage.left + stage.width * Number(style.getPropertyValue('--dock-box-x')), y: stage.top + stage.height * parseFloat(style.top) / 100, w: stage.width * Number(style.getPropertyValue('--dock-box-width')), h: stage.height * parseFloat(style.height) / 100 };
        if (Math.abs(actual.x - expected.x) > 1 || Math.abs(actual.y - expected.y) > 1 || Math.abs(actual.width - expected.w) > 1 || Math.abs(actual.height - expected.h) > 1) errors.push({ id: pane.dataset.dockPane, actual: actual.toJSON(), expected });
        if (!pane.parentElement.classList.contains('dock-body')) errors.push({ id: pane.dataset.dockPane, parent: pane.parentElement.className });
      }
      const browser = document.querySelector('[data-dock-pane=browser]:not([hidden])');
      if (browser) {
        const header = browser.querySelector('.dock-pane-header'), tabs = browser.querySelector('.dock-browser-tabs');
        if (tabs.parentElement !== header || Math.abs(rect(header).height - 40) > 1) errors.push({ browserHeader: rect(header).toJSON(), tabsParent: tabs.parentElement.className });
        const controls = [...header.querySelectorAll('button')].filter(button => /^(Expand Browser|Restore Browser|Actions for Browser pane|Close Browser pane)$/.test(button.getAttribute('aria-label')));
        for (let index = 0; index < controls.length; index++) {
          const control = controls[index], bounds = rect(control), hit = document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
          if (!control.contains(hit)) errors.push({ browserControlCovered: control.getAttribute('aria-label'), bounds: bounds.toJSON(), hit: hit?.outerHTML.slice(0, 160) });
          for (const previous of controls.slice(0, index)) {
            const other = rect(previous);
            if (bounds.left < other.right && bounds.right > other.left && bounds.top < other.bottom && bounds.bottom > other.top) errors.push({ browserControlOverlap: [control.getAttribute('aria-label'), previous.getAttribute('aria-label')] });
          }
        }
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
  assert.equal(await page.locator('[data-dock-pane=browser]').evaluate(element => element.style.getPropertyValue('--dock-box-width')), '1');
  await page.getByRole('button', { name: 'Show conversation', exact: true }).click();
  assert.equal(await page.locator('.dock-pane:not(.dock-session-rail):not([hidden])').count(), 0, 'Conversation must be accessible in compact mode');
  assert(await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').evaluate(element => !element.hasAttribute('inert') && getComputedStyle(element).visibility !== 'hidden'));
  await page.getByRole('button', { name: 'Toggle Browser', exact: true }).click(); await geometry();
  await page.setViewportSize({ width, height });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.locator('.dock-workspace:not(.is-compact)').waitFor(); await geometry();
  assert(await page.locator('[data-dock-pane=terminal]').isVisible() && await page.locator('[data-dock-pane=browser]').isVisible(), 'Widening must restore the saved tiled tools');
  await page.getByRole('button', { name: 'Expand Browser', exact: true }).click();
  assert(await page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)').evaluate(element => element.hasAttribute('inert') && getComputedStyle(element).visibility === 'hidden'), 'Filled browser must exclude conversation from focus');
  const browserAccessibility = async () => {
    assert.equal(await page.getByRole('button', { name: 'Restore Browser', exact: true }).count(), 1, 'Expanded Browser restore must remain accessible');
    const snapshot = await page.locator('.dock-workspace').ariaSnapshot();
    assert(snapshot.includes('toolbar "Workspace views"'), 'Workspace toolbar disappeared from accessibility tree');
    assert.equal(await page.locator('.dock-stage > *').count(), 0, 'Measurement stage must not own pane accessibility');
    assert(snapshot.includes('button "Restore Browser"'), 'Browser chrome disappeared from accessibility tree');
    const sidebar = page.locator('.workspace-main .sidebar').first();
    if (await page.getByRole('button', { name: 'Expand sidebar', exact: true }).count()) {
      assert.equal(await sidebar.getByRole('button').count(), 0, 'Collapsed sidebar must exclude its buttons from accessibility navigation');
      assert.equal(await sidebar.getByRole('link').count(), 0, 'Collapsed sidebar must exclude its links from accessibility navigation');
    }
  };
  try {
    for (const size of [width, 900, 740, 360, width]) {
      await page.setViewportSize({ width: size, height });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const toggle = page.locator('.workspace-titlebar .mode-sidebar-toggle');
      if (await toggle.getAttribute('aria-label') === 'Collapse sidebar') await toggle.click();
      await geometry();
      await browserAccessibility();
      if (!await page.locator('.app.layout-phone').count()) {
        await toggle.click(); await geometry();
        await browserAccessibility();
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

async function checkSessionRail(page, base) {
  await page.goto(`${base}/?mode=code&thread=thread-rich&panel=new`);
  const toggle = page.getByRole('button', { name: 'Session rail', exact: true });
  await toggle.waitFor();
  if (await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).count()) await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click();
  if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click();
  const rail = page.locator('[data-dock-pane=session]');
  const chat = page.locator('.workspace-main > .app > :is(.main,.tablet-main,.compact-shell)');
  const draft = page.locator('.composer-shell textarea').first();
  await draft.fill('Preserve this draft through rail resizing');
  // Typing outside dismisses a floating rail; start wide and explicitly reopen.
  if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click();
  for (const width of [1440, 800, 360, 1440]) {
    await page.setViewportSize({ width, height: 800 });
    await page.waitForFunction(floating => document.querySelector('.dock-workspace').classList.contains('session-floating') === floating, width < 840);
    const r = await rail.boundingBox(), c = await chat.boundingBox(), composer = await page.locator('.composer').boundingBox();
    assert(r && c && composer, `Missing rail/chat/composer at ${width}`);
    assert(r.x >= c.x && r.x + r.width <= (width < 840 ? c.x + c.width : width) + 1, `Rail outside chat area at ${width}`);
    if (width < 840) assert(r.y + r.height <= composer.y + 1, `Floating rail covers composer at ${width}`);
    else {
      assert(Math.abs(r.y + r.height - c.y - c.height + 8) < 1, 'Docked rail must use the space beside the composer');
      const messages = await page.locator('.messages').boundingBox();
      assert(messages && Math.abs(messages.x + messages.width - r.x - r.width - 8) < 1, 'Chat scrollbar must sit outside the rail');
      assert(await page.locator('.messages').evaluate(element => {
        const rect = element.getBoundingClientRect();
        return element.contains(document.elementFromPoint(rect.right - 4, rect.top + 40));
      }), 'Chat scrollbar must not be clipped or covered at the outer edge');
    }
    assert(width < 840 ? r.x < c.x + c.width : r.x >= c.x + c.width - 1, `Incorrect rail placement at ${width}`);
    assert.equal(await rail.locator('.dock-pane-header').count(), 0, 'Rail must not have expand/close chrome');
    assert(await draft.evaluate(element => !element.closest('[inert]')), 'Rail must keep chat interactive');
    assert.equal(await draft.inputValue(), 'Preserve this draft through rail resizing');
    assert.equal(await rail.locator('.code-session-rail').evaluate(element => getComputedStyle(element).scrollbarWidth), 'none', 'Rail scrollbar must stay hidden');
  }
  const railScroll = rail.getByRole('region', { name: 'Session details' });
  const chatScroll = await page.locator('.messages').evaluate(element => element.scrollTop);
  await railScroll.focus(); await page.keyboard.press('End');
  await page.waitForFunction(() => document.querySelector('.code-session-rail').scrollTop > 0);
  assert.equal(await page.locator('.messages').evaluate(element => element.scrollTop), chatScroll, 'Scrolling the rail must not move the chat');
  await page.setViewportSize({ width: 800, height: 800 });
  await page.locator('.dock-workspace.session-floating').waitFor();
  const before = await chat.boundingBox();
  await draft.click(); await rail.waitFor({ state: 'hidden' });
  assert.deepEqual(await chat.boundingBox(), before, 'Dismissing floating rail resized chat');
  await toggle.click(); await rail.waitFor({ state: 'visible' });
  await rail.getByRole('button', { name: 'Environment', exact: true }).focus();
  await page.keyboard.press('Escape'); await rail.waitFor({ state: 'hidden' });
  assert(await toggle.evaluate(element => document.activeElement === element), 'Escape did not restore toggle focus');
  console.log('PASS Session rail dock/float, composer clearance, dismissal and draft continuity');
  await page.setViewportSize({ width: 1440, height: 800 });
  for (const theme of ['light', 'dark', 'glass-light', 'glass-dark']) {
    await page.goto(`${base}/?mode=code&thread=thread-rich&panel=new&theme=${theme}`);
    await page.waitForFunction(theme => document.documentElement.dataset.themeId === theme, theme);
    await toggle.waitFor();
    if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click();
    await page.locator('.dock-workspace.has-session:not(.session-floating)').waitFor();
    const surface = page.locator('.dock-session-space'), s = await surface.boundingBox(), c = await chat.boundingBox(), r = await rail.boundingBox();
    assert(s && c && r && Math.abs(s.x - c.x) < 1 && Math.abs(s.y - c.y) < 1 && Math.abs(s.height - c.height) < 1 && Math.abs(s.x + s.width - r.x - r.width - 8) < 1, `${theme}: shared tint must span the entire chat and rail`);
    assert(await surface.evaluate(element => getComputedStyle(element).visibility === 'visible'), `${theme}: shared surface is hidden`);
    assert(await page.locator('.workspace-main > .app, .workspace-main .content').evaluateAll(elements => elements.every(element => {
      const style = getComputedStyle(element);
      return style.backgroundColor === 'rgba(0, 0, 0, 0)' && style.backgroundImage === 'none';
    })), `${theme}: extra chat tint would create a seam`);
    assert(await surface.evaluate(element => {
      const style = getComputedStyle(element);
      return document.documentElement.dataset.translucent === 'true' ? style.backgroundColor.startsWith('rgba(') && style.backgroundColor !== 'rgba(0, 0, 0, 0)' : style.backgroundImage.includes('linear-gradient');
    }), `${theme}: conversation material was lost`);
  }
  console.log('PASS Shared chat/rail surface in light, dark and both glass themes');
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
    const railPage = await browser.newPage({ viewport: { width: 1440, height: 800 }, reducedMotion: 'reduce' });
    await checkSessionRail(railPage, process.argv[2] ?? 'http://localhost:1421');
    await railPage.close();
    const page = await browser.newPage({ viewport: { width: 1024, height: 720 }, hasTouch: true, reducedMotion: 'reduce' });
    await checkTabletSidebar(page, async params => {
      const url = new URL(process.argv[2] ?? 'http://localhost:1421'); url.search = new URLSearchParams(params).toString();
      await page.goto(url.href); await page.locator('.dock-workspace').waitFor();
    });
    await page.close();
  } finally { await browser.close(); }
}
