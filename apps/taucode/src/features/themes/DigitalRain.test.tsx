// @vitest-environment jsdom
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { DigitalRain } from './DigitalRain';
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); document.documentElement.dataset.digitalRain = 'false'; });
it('mounts only for enabled empty states and respects reduced motion', () => {
  document.documentElement.dataset.digitalRain = 'false';
  const view = render(<DigitalRain />);
  expect(view.container.querySelector('canvas')).toBeNull();
  Object.defineProperty(window,'matchMedia',{configurable:true,value:() => ({matches:true,addEventListener(){},removeEventListener(){}})});
  document.documentElement.dataset.digitalRain = 'true';
  act(() => window.dispatchEvent(new Event('taucode-theme-applied')));
  expect(view.container.querySelector('canvas')).toBeNull();
});
it('caps frames at 20 fps and stops when hidden, offscreen or unmounted', () => {
  vi.useFakeTimers();
  document.documentElement.dataset.digitalRain = 'true';
  Object.defineProperty(window,'matchMedia',{configurable:true,value:() => ({matches:false,addEventListener(){},removeEventListener(){}})});
  Object.defineProperty(document,'hidden',{configurable:true,value:false});
  const draw = vi.fn();
  vi.spyOn(HTMLCanvasElement.prototype,'getContext').mockReturnValue({ clearRect:draw,fillText:vi.fn() } as unknown as CanvasRenderingContext2D);
  let intersect: (entries: {isIntersecting:boolean}[]) => void = () => {};
  Object.defineProperty(window,'IntersectionObserver',{configurable:true,value:class { constructor(cb: typeof intersect) { intersect = cb; } observe() {} disconnect() {} }});
  const view = render(<DigitalRain />);
  expect(view.container.querySelector('canvas')).toBeTruthy();
  act(() => vi.advanceTimersByTime(1000));
  expect(draw).not.toHaveBeenCalled();
  act(() => { intersect([{isIntersecting:true}]); vi.advanceTimersByTime(1000); });
  expect(draw).toHaveBeenCalledTimes(20);
  Object.defineProperty(document,'hidden',{configurable:true,value:true});
  act(() => { document.dispatchEvent(new Event('visibilitychange')); vi.advanceTimersByTime(1000); });
  expect(draw).toHaveBeenCalledTimes(20);
  Object.defineProperty(document,'hidden',{configurable:true,value:false});
  act(() => { document.dispatchEvent(new Event('visibilitychange')); intersect([{isIntersecting:false}]); vi.advanceTimersByTime(1000); });
  expect(draw).toHaveBeenCalledTimes(20);
  view.unmount();
  expect(vi.getTimerCount()).toBe(0);
});
