import { createElement, useId, useLayoutEffect, useRef, useState, type HTMLAttributes, type Ref, type CSSProperties } from "react";
import { chamferPath, roundedRectPath, squirclePath } from "./squirclePath";
import "./squircle.css";

type Props = HTMLAttributes<HTMLElement> & {
  as?: "div" | "section" | "button" | "span";
  disabled?: boolean;
  type?: "button";
  ref?: Ref<HTMLElement>;
};

export function Squircle({ as = "div", children, className = "", ref, style, ...props }: Props) {
  const root = useRef<HTMLElement | null>(null);
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, '');
  const [supported] = useState(() => typeof CSS !== "undefined" && !!CSS.supports?.("corner-shape", "squircle"));
  const [shape, setShape] = useState({ fill: "", stroke: "", highlightPath: "", kind: 'squircle', stops: 0, angle: 135, highlight: false, glow: false, neon: false, light: false, native: supported });
  // Inline preview variables can change independently of the global theme event.
  const styleKey = JSON.stringify(style);
  useLayoutEffect(() => {
    if (!root.current) return;
    const element = root.current;
    const read = () => {
      const css = getComputedStyle(element);
      const value = (key: string) => css.getPropertyValue(key).trim();
      const radiusValue = value('--radius-window');
      const parsedRadius = parseFloat(radiusValue);
      const radius = (Number.isFinite(parsedRadius) ? parsedRadius : 12) * (radiusValue.endsWith('rem') ? parseFloat(getComputedStyle(document.documentElement).fontSize) || 16 : 1);
      const kind = value('--card-corners') || 'squircle', stops = Number(value('--card-border-stops'));
      const highlight = value('--card-border-highlight') === '1';
      return { radius, cut: parseFloat(value('--card-chamfer')) || 10, kind, stops, angle: Number(value('--card-border-angle') || 135), highlight, glow: value('--card-neon-glow') === '1', neon: value('--card-neon-fill') === '1', light: value('--card-light-edge') === '1', native: supported && kind === 'squircle' && !stops && !highlight };
    };
    let config = read();
    let frame = 0, previousWidth = -1, previousHeight = -1;
    const measure = () => {
      frame = 0;
      const { width, height } = element.getBoundingClientRect();
      if (width === previousWidth && height === previousHeight) return;
      previousWidth = width; previousHeight = height;
      const path = (inset: number) => config.kind === 'chamfer' ? chamferPath(width, height, config.cut, 2, inset) : config.kind === 'round' ? roundedRectPath(width, height, config.radius, inset) : squirclePath(width, height, config.radius, 0.6, inset);
      const stroke = path(0.5);
      // Start at the left corner, follow the top edge, finish at the right corner.
      const inner = path(1.5);
      const edges = inner.split(' L ');
      const lastEdge = edges[edges.length - 1].replace(' Z', '');
      const highlightPath = config.kind === 'chamfer' ? `M ${lastEdge} L ${edges[0].slice(2)} L ${edges[1]}` : `M ${lastEdge} L ${edges[0].slice(2)}`;
      setShape({ ...config, fill: path(0), stroke, highlightPath });
    };
    measure();
    const themeChanged = () => { cancelAnimationFrame(frame); config = read(); previousWidth = -1; measure(); };
    window.addEventListener('taucode-theme-applied', themeChanged);
    const observer = new ResizeObserver(() => { if (!frame) frame = requestAnimationFrame(measure); });
    observer.observe(element, { box: "border-box" });
    return () => { observer.disconnect(); cancelAnimationFrame(frame); window.removeEventListener('taucode-theme-applied', themeChanged); };
  }, [supported, styleKey]);
  const { native } = shape;
  const gradient = shape.stops >= 2;
  const paint = gradient ? `url(#${id}-border)` : undefined;
  const clipContent = className.split(' ').includes('squircle-clip');
  return createElement(as, {
    ...props,
    style: { ...style, "--squircle-clip": shape.fill ? `path('${shape.fill}')` : undefined } as CSSProperties,
    className: `squircle ${native ? "squircle-native" : "squircle-svg"} ${gradient ? 'squircle-gradient' : ''} ${shape.neon ? 'squircle-neon' : ''} ${className}`,
    'data-card-corners': shape.kind,
    ref: (element: HTMLElement | null) => {
      root.current = element;
      if (typeof ref === "function") ref(element);
      else if (ref) ref.current = element;
    },
  }, <>
    <span aria-hidden="true" className="squircle-background" style={!native && shape.fill ? { clipPath: `path('${shape.fill}')` } : undefined} />
    {clipContent ? <span className="squircle-content">{children}</span> : children}
    {!native && <svg aria-hidden="true" className="squircle-border" width="100%" height="100%" fill="none" style={{'--squircle-active-filter': `url(#${id}-active)`} as CSSProperties}>
      <defs>
        {gradient && <linearGradient id={`${id}-border`} gradientUnits="objectBoundingBox" x1="0" y1="0.5" x2="1" y2="0.5" gradientTransform={`rotate(${shape.angle - 90} .5 .5)`}>
          {Array.from({ length: shape.stops }, (_, i) => <stop key={i} offset={`${100*i/(shape.stops-1)}%`} style={{ stopColor: `var(--card-border-${i+1})` }} />)}
        </linearGradient>}
        {gradient && <filter id={`${id}-active`} x="-50%" y="-50%" width="200%" height="200%" colorInterpolationFilters="sRGB"><feComponentTransfer>
          <feFuncR type="linear" slope="1.35" /><feFuncG type="linear" slope="1.35" /><feFuncB type="linear" slope="1.35" /><feFuncA type="linear" slope="1.6" />
        </feComponentTransfer></filter>}
        {gradient && shape.glow && <filter id={`${id}-glow`} x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3" /></filter>}
        {shape.highlight && <>
          <linearGradient id={`${id}-highlight`} gradientUnits="userSpaceOnUse" x2="100%"><stop offset="0" stopColor="white" stopOpacity="0" /><stop offset=".5" stopColor="white" stopOpacity=".35" /><stop offset="1" stopColor="white" stopOpacity="0" /></linearGradient>
        </>}
      </defs>
      {shape.light && <path className="squircle-hairline" d={shape.stroke} stroke="rgba(0,0,0,.22)" strokeWidth="2" />}
      {gradient && shape.glow && <path className="squircle-glow" d={shape.stroke} stroke={paint} strokeWidth="3" opacity=".35" filter={`url(#${id}-glow)`} />}
      <path className="squircle-stroke" d={shape.stroke} stroke={paint} strokeWidth="1" vectorEffect="non-scaling-stroke" />
      {shape.highlight && <path className="squircle-highlight" d={shape.highlightPath} stroke={`url(#${id}-highlight)`} strokeWidth="1" />}
    </svg>}
  </>);
}
