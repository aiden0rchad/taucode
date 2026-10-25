import { useEffect } from 'react';

export function useThemedScrollbars() {
  useEffect(() => {
    const timers = new Map<HTMLElement, ReturnType<typeof setTimeout>>();
    const scroll = (event: Event) => {
      const node = event.target === document ? document.scrollingElement : event.target;
      if (!(node instanceof HTMLElement)) return;
      clearTimeout(timers.get(node));
      node.classList.add('is-scrolling');
      timers.set(node, setTimeout(() => { node.classList.remove('is-scrolling'); timers.delete(node); }, 900));
    };
    document.addEventListener('scroll', scroll, { capture: true, passive: true });
    return () => {
      document.removeEventListener('scroll', scroll, true);
      for (const [node,timer] of timers) { clearTimeout(timer); node.classList.remove('is-scrolling'); }
    };
  }, []);
}
