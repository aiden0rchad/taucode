import { useLayoutEffect, useRef, type RefObject } from "react";

export const reasoningPlaybackRate = (position: number) => 0.55 + 3.2 * position ** 1.6;

export function useReasoningAnimationSpeed(surface: RefObject<HTMLElement | null>, open: boolean, position: number) {
  const initialized = useRef(new WeakSet<Animation>());
  const target = reasoningPlaybackRate(position);
  useLayoutEffect(() => {
    if (!open || !surface.current?.getAnimations) { initialized.current = new WeakSet(); return; }
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame: number | null = null;
    const cancel = () => { if (frame !== null) cancelAnimationFrame(frame); frame = null; };
    const retarget = () => {
      cancel();
      if (motion.matches) { initialized.current = new WeakSet(); return; }
      const animations = (surface.current?.getAnimations?.({ subtree: true }) ?? []).filter(animation =>
        "animationName" in animation && ["reasoning-flow", "reasoning-drift"].includes(String(animation.animationName)));
      const tween = animations.flatMap(animation => {
        if (!initialized.current.has(animation)) {
          initialized.current.add(animation);
          animation.updatePlaybackRate(target);
          return [];
        }
        return animation.playbackRate === target ? [] : [{ animation, from: animation.playbackRate }];
      });
      if (!tween.length) return;
      const start = performance.now();
      const tick = (now: number) => {
        const progress = Math.min(1, Math.max(0, (now - start) / 900));
        const eased = progress * progress * (3 - 2 * progress);
        for (const { animation, from } of tween) animation.updatePlaybackRate(from + (target - from) * eased);
        frame = progress < 1 ? requestAnimationFrame(tick) : null;
      };
      frame = requestAnimationFrame(tick);
    };
    retarget();
    motion.addEventListener("change", retarget);
    return () => { cancel(); motion.removeEventListener("change", retarget); };
  }, [open, target, surface]);
}
