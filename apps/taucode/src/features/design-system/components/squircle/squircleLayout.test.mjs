// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { createElement, Fragment } from 'react';
import { cleanup, render } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { Squircle } from './Squircle';

const styles = ['src/styles/settings.css', 'src/features/themes/themes.css', 'src/features/design-system/components/squircle/squircle.css'].map(path => readFileSync(path, 'utf8')).join('\n');
afterEach(cleanup);

// Every clipped caller: these declarations are the CSS contract; jsdom does
// not lay out grids. webkit-check.mjs verifies the resulting pixel geometry.
it.each(['settings-window', 'theme-editor-card', 'theme-color-swatch', 'theme-mini', 'theme-app-preview'])('preserves the %s clipping layout', className => {
  const { container } = render(createElement(Fragment, null,
    createElement('style', null, styles),
    createElement(Squircle, { className: `${className} squircle-clip`, style: { padding: '7px' } }, createElement('button', null, 'Child')),
  ));
  const host = container.querySelector(`.${className}`);
  const content = host.querySelector(':scope > .squircle-content');
  const hostCss = getComputedStyle(host);
  const css = getComputedStyle(content);
  expect(css.gridArea).toBe('1 / 1 / -1 / -1');
  expect(css.flex).toBe('1 1 auto');
  expect(css.alignSelf).toBe('stretch');
  expect(css.justifySelf).toBe('stretch');
  expect(css.width).toBe('100%');
  expect(css.height).toBe('100%');
  expect(css.minWidth).toBe('0');
  expect(css.minHeight).toBe('0');
  expect(css.display).toBe(hostCss.display);
  const layout = ([...container.querySelector('style').sheet.cssRules]).find(rule => rule.selectorText === '.squircle > .squircle-content').style;
  for (const property of ['display', 'flex-direction', 'flex-wrap', 'grid-template-columns', 'grid-template-rows', 'grid-template-areas', 'grid-auto-flow', 'grid-auto-columns', 'grid-auto-rows', 'gap', 'align-items', 'justify-items', 'align-content', 'justify-content']) {
    expect(layout.getPropertyValue(property), property).toBe('inherit');
  }
  expect(hostCss.padding).toBe('7px');
  expect(parseFloat(css.padding) || 0).toBe(0);
  expect(css.clipPath).toBe('var(--squircle-clip)');
  expect(getComputedStyle(host).overflow).toBe('visible');
  expect(host.querySelector(':scope > .squircle-border')).toBeTruthy();
});

it.each(['composer-shell', 'workspace-card project-group-card', 'work-space project-group-card', 'theme-preview-project', 'theme-preview-composer', ''])('keeps unwrapped %s children in the host layout', className => {
  const { container } = render(createElement(Squircle, { className }, createElement('button', null, 'Child')));
  expect(container.querySelector('.squircle-content')).toBeNull();
  expect(container.querySelector('.squircle > button')).toBeTruthy();
});
