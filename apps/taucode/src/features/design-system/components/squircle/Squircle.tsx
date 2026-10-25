import { createElement, useLayoutEffect, useRef, useState, type HTMLAttributes, type Ref, type CSSProperties } from "react";
import { squirclePath } from "./squirclePath";
import "./squircle.css";

type Props = HTMLAttributes<HTMLElement> & {
  as?: "div" | "section" | "button";
  disabled?: boolean;
  type?: "button";
  ref?: Ref<HTMLElement>;
};

export function Squircle({ as = "div", children, className = "", ref, style, ...props }: Props) {
  const root = useRef<HTMLElement | null>(null);
  const [native] = useState(() => typeof CSS !== "undefined" && !!CSS.supports?.("corner-shape", "squircle"));
  const [shape, setShape] = useState({ fill: "", stroke: "" });
  useLayoutEffect(() => {
    if (native || !root.current) return;
    const element = root.current;
    const radius = parseFloat(getComputedStyle(element).getPropertyValue("--radius-window")) || 20;
    let frame = 0, previousWidth = -1, previousHeight = -1;
    const measure = () => {
      frame = 0;
      const { width, height } = element.getBoundingClientRect();
      if (width === previousWidth && height === previousHeight) return;
      previousWidth = width; previousHeight = height;
      setShape({ fill: squirclePath(width, height, radius), stroke: squirclePath(width, height, radius, 0.6, 0.5) });
    };
    measure();
    const observer = new ResizeObserver(() => { if (!frame) frame = requestAnimationFrame(measure); });
    observer.observe(element, { box: "border-box" });
    return () => { observer.disconnect(); cancelAnimationFrame(frame); };
  }, [native]);
  return createElement(as, {
    ...props,
    style: { ...style, "--squircle-clip": shape.fill ? `path('${shape.fill}')` : undefined } as CSSProperties,
    className: `squircle ${native ? "squircle-native" : "squircle-svg"} ${className}`,
    ref: (element: HTMLElement | null) => {
      root.current = element;
      if (typeof ref === "function") ref(element);
      else if (ref) ref.current = element;
    },
  }, <>
    <span aria-hidden="true" className="squircle-background" style={!native && shape.fill ? { clipPath: `path('${shape.fill}')` } : undefined} />
    {children}
    {!native && <svg aria-hidden="true" className="squircle-border" width="100%" height="100%"><path d={shape.stroke} fill="none" strokeWidth="1" vectorEffect="non-scaling-stroke" /></svg>}
  </>);
}
