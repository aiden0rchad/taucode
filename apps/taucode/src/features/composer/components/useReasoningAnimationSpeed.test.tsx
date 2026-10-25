// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { reasoningPlaybackRate, useReasoningAnimationSpeed } from "./useReasoningAnimationSpeed";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function animationClock(reduced = false) {
  let now = 0, nextId = 0;
  const frames = new Map<number, FrameRequestCallback>();
  vi.spyOn(performance, "now").mockImplementation(() => now);
  const request = vi.fn((callback: FrameRequestCallback) => { frames.set(++nextId, callback); return nextId; });
  const cancel = vi.fn((id: number) => { frames.delete(id); });
  vi.stubGlobal("requestAnimationFrame", request);
  vi.stubGlobal("cancelAnimationFrame", cancel);
  const motion = Object.assign(new EventTarget(), { matches: reduced });
  vi.spyOn(window, "matchMedia").mockReturnValue(motion as MediaQueryList);
  const animations = ["reasoning-flow", "reasoning-drift", "reasoning-glow"].map(animationName => ({
    animationName, playbackRate: 1, currentTime: 1234,
    updatePlaybackRate: vi.fn(function (this: { playbackRate: number }, rate: number) { this.playbackRate = rate; }),
  }));
  const getAnimations = vi.fn(() => animations as unknown as Animation[]);
  const surface = { current: { getAnimations } as unknown as HTMLElement };
  const advance = (milliseconds: number) => act(() => {
    now += milliseconds;
    const callbacks = [...frames.values()]; frames.clear();
    for (const callback of callbacks) callback(now);
  });
  return { surface, animations, frames, request, cancel, advance, motion, getAnimations };
}

it("uses a monotonic, progressively accelerating rate across model-relative positions", () => {
  expect(reasoningPlaybackRate(0)).toBeCloseTo(0.55);
  expect(reasoningPlaybackRate(1)).toBeCloseTo(3.75);
  const rates = Array.from({ length: 11 }, (_, index) => reasoningPlaybackRate(index / 10));
  for (let index = 1; index < rates.length; index++) expect(rates[index]).toBeGreaterThan(rates[index - 1]);
  expect(rates[10] - rates[9]).toBeGreaterThan(rates[1] - rates[0]);
});

it("starts at the selected rate and eases to a new rate without changing time or running idle frames", () => {
  const clock = animationClock();
  const { rerender } = renderHook(({ position }) => useReasoningAnimationSpeed(clock.surface, true, position), { initialProps: { position: 0 } });
  expect(clock.getAnimations).toHaveBeenCalledWith({ subtree: true });
  expect(clock.animations[0].playbackRate).toBeCloseTo(0.55);
  expect(clock.request).not.toHaveBeenCalled();
  rerender({ position: 1 });
  expect(clock.frames.size).toBe(1);
  clock.advance(225);
  expect(clock.animations[0].playbackRate).toBeCloseTo(1.05);
  clock.advance(225);
  expect(clock.animations[0].playbackRate).toBeCloseTo(2.15);
  clock.advance(450);
  for (const animation of clock.animations.slice(0, 2)) {
    expect(animation.playbackRate).toBeCloseTo(3.75);
    expect(animation.currentTime).toBe(1234);
  }
  expect(clock.animations[2].updatePlaybackRate).not.toHaveBeenCalled();
  expect(clock.frames.size).toBe(0);
  const requests = clock.request.mock.calls.length;
  clock.advance(2000);
  expect(clock.request).toHaveBeenCalledTimes(requests);
});

it("retargets from the current rate during dragging and cancels on close or unmount", () => {
  const clock = animationClock();
  const { rerender, unmount } = renderHook(({ position, open }) => useReasoningAnimationSpeed(clock.surface, open, position), { initialProps: { position: 0, open: true } });
  rerender({ position: 0.65, open: true });
  clock.advance(450);
  const rate = clock.animations[0].playbackRate;
  rerender({ position: 0.3, open: true });
  expect(clock.cancel).toHaveBeenCalled();
  expect(clock.frames.size).toBe(1);
  expect(clock.animations[0].playbackRate).toBe(rate);
  clock.advance(450);
  expect(clock.animations[0].playbackRate).toBeCloseTo((rate + reasoningPlaybackRate(0.3)) / 2);
  rerender({ position: 0.3, open: false });
  expect(clock.frames.size).toBe(0);
  rerender({ position: 1, open: true });
  expect(clock.animations[0].playbackRate).toBeCloseTo(3.75);
  expect(clock.frames.size).toBe(0);
  rerender({ position: 0, open: true });
  unmount();
  expect(clock.frames.size).toBe(0);
});

it("never tweens reduced motion and stops immediately when that preference changes", () => {
  const clock = animationClock(true);
  const { rerender } = renderHook(({ position }) => useReasoningAnimationSpeed(clock.surface, true, position), { initialProps: { position: 0 } });
  rerender({ position: 1 });
  expect(clock.getAnimations).not.toHaveBeenCalled();
  expect(clock.request).not.toHaveBeenCalled();
  act(() => { clock.motion.matches = false; clock.motion.dispatchEvent(new Event("change")); });
  expect(clock.animations[0].playbackRate).toBeCloseTo(3.75);
  expect(clock.request).not.toHaveBeenCalled();
  rerender({ position: 0 });
  expect(clock.frames.size).toBe(1);
  act(() => { clock.motion.matches = true; clock.motion.dispatchEvent(new Event("change")); });
  expect(clock.frames.size).toBe(0);
});
