// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { SettingsNav, SETTINGS_GROUPS } from './SettingsNav';
afterEach(cleanup);
it('puts Appearance first in Core with a palette icon and a navigable route', () => {
  const select = vi.fn();
  render(<SettingsNav activeSection="appearance" onSelectSection={select} />);
  expect(SETTINGS_GROUPS[0]).toMatchObject({label:'Core'});
  expect(SETTINGS_GROUPS[0].sections[0]).toBe('appearance');
  const button = screen.getByRole('button',{name:'Appearance'});
  expect(screen.getAllByRole('button')[0]).toBe(button);
  expect(button.querySelector('.lucide-palette')).toBeTruthy();
  expect(button.getAttribute('aria-current')).toBe('page');
  fireEvent.click(button);
  expect(select).toHaveBeenCalledWith('appearance');
});
