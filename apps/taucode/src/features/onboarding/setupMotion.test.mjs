import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
it('uses direction-aware CSS motion, reduced-motion crossfades, and static hidden blobs', () => {
  const css = readFileSync('src/features/onboarding/onboarding.css', 'utf8');
  expect(css).toContain('.setup-forward.setup-exit');
  expect(css).toContain('.setup-back.setup-enter');
  expect(css).toContain('@media(prefers-reduced-motion:reduce)');
  expect(css).toContain('animation:setup-fade 150ms both!important');
  expect(css).toContain('animation:none!important;filter:blur(80px)!important;transform:none!important');
  expect(css).toContain('animation-play-state:paused');
});
