// @vitest-environment jsdom
import { act, cleanup, fireEvent, renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { useThemedScrollbars } from './useThemedScrollbars';

afterEach(() => { cleanup(); vi.useRealTimers(); document.body.innerHTML = ''; });
it('uses passive scroll capture, resets the 900ms timeout and cleans up', () => {
  vi.useFakeTimers();
  const listen = vi.spyOn(document,'addEventListener');
  const { unmount } = renderHook(useThemedScrollbars);
  expect(listen).toHaveBeenCalledWith('scroll',expect.any(Function),{capture:true,passive:true});
  document.body.innerHTML = '<div class="messages"></div><div class="settings-content"></div>';
  const [messages,settings] = [...document.body.children];
  fireEvent.scroll(messages);
  expect(messages.classList.contains('is-scrolling')).toBe(true);
  act(() => vi.advanceTimersByTime(800));
  fireEvent.scroll(messages); fireEvent.scroll(settings);
  act(() => vi.advanceTimersByTime(899));
  expect(messages.classList.contains('is-scrolling')).toBe(true);
  act(() => vi.advanceTimersByTime(1));
  expect(document.querySelector('.is-scrolling')).toBeNull();
  fireEvent.scroll(messages); unmount();
  expect(messages.classList.contains('is-scrolling')).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
  listen.mockRestore();
});
it('themes all scroll surfaces with stable-width, inset thumbs and eased sidebar fades', () => {
  const css = readFileSync('src/styles/scrollbars.css','utf8');
  for (const selector of ['.sidebar-body','.messages','.chat-scroll','.side-panel-content','.settings-content','.theme-editor-controls','.theme-editor-preview','.ds-popover','.settings-select-options','.composer-model-options']) expect(css).toContain(selector);
  expect(css).toContain('width: 8px; height: 8px');
  expect(css).toContain('border: 2px solid transparent');
  expect(css).toContain('scrollbar-width: thin');
  expect(css).toContain('*.is-scrolling::-webkit-scrollbar-thumb');
  expect(css).toContain('margin-inline: -6px; padding-inline: 6px');
  const sidebar = readFileSync('src/styles/sidebar.css','utf8');
  expect(sidebar).toContain('#000 28px');
  expect(sidebar).toContain('--sidebar-fade-bottom-mid: rgba(0,0,0,.65)');
});
