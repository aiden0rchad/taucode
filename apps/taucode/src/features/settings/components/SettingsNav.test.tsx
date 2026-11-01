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

it('filters section names and groups without navigating until a result is chosen', () => {
  const select = vi.fn();
  render(<SettingsNav activeSection="appearance" onSelectSection={select} />);
  const search = screen.getByRole('searchbox', { name: 'Find settings section' });
  fireEvent.change(search, { target: { value: 'browser' } });
  expect(screen.getAllByRole('button')).toHaveLength(1);
  expect(select).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Browser' }));
  expect(select).toHaveBeenCalledWith('browser');
  fireEvent.change(search, { target: { value: 'no such section' } });
  expect(screen.getByRole('status').textContent).toBe('No matching sections');
  fireEvent.keyDown(search, { key: 'Escape' });
  expect(screen.getByRole('button', { name: 'Appearance' })).toBeTruthy();
});
