import { useEffect, useRef, useState } from 'react';
function enabled() {
  return document.documentElement.dataset.digitalRain === 'true' && !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
}
export function DigitalRain() {
  const [on,setOn] = useState(enabled);
  useEffect(() => {
    const media = window.matchMedia?.('(prefers-reduced-motion: reduce)'), update = () => setOn(enabled());
    media?.addEventListener('change',update);
    window.addEventListener('taucode-theme-applied',update);
    return () => { media?.removeEventListener('change',update); window.removeEventListener('taucode-theme-applied',update); };
  }, []);
  return on ? <RainCanvas /> : null;
}
function RainCanvas() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current!;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    let visible = false, timer: ReturnType<typeof setInterval>|undefined;
    let drops: number[] = [];
    const draw = () => {
      const width = canvas.clientWidth, height = canvas.clientHeight;
      if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; drops = Array.from({ length:Math.ceil(width/16) },() => Math.random()*height); }
      ctx.clearRect(0,0,width,height);
      ctx.fillStyle = getComputedStyle(canvas).getPropertyValue('--text-accent');
      ctx.font = '14px monospace';
      drops.forEach((y,i) => {
        ctx.fillText(String.fromCharCode(0x30a0 + Math.floor(Math.random()*90)),i*16,y);
        drops[i] = y > height + 20 ? 0 : y + 4;
      });
    };
    const sync = () => {
      clearInterval(timer); timer = undefined;
      if (visible && !document.hidden) timer = setInterval(draw,50);
    };
    const observer = new window.IntersectionObserver(entries => { visible = entries.some(e => e.isIntersecting); sync(); });
    observer.observe(canvas);
    document.addEventListener('visibilitychange',sync);
    return () => { clearInterval(timer); observer.disconnect(); document.removeEventListener('visibilitychange',sync); };
  }, []);
  return <canvas ref={ref} className="digital-rain" aria-hidden="true" />;
}
