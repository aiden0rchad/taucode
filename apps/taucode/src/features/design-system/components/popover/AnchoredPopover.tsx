import { useLayoutEffect, type ComponentProps, type RefObject } from "react";
import { createPortal } from "react-dom";
import { PopoverSurface } from "./PopoverPrimitives";

type Props = ComponentProps<typeof PopoverSurface> & {
  anchorRef: RefObject<HTMLElement | null>;
  surfaceRef: RefObject<HTMLDivElement | null>;
};

export function AnchoredPopover({ anchorRef, surfaceRef, style, ...props }: Props) {
  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const surface = surfaceRef.current;
    if (!anchor || !surface) return;
    const place = () => {
      const rect = anchor.getBoundingClientRect();
      const viewport = window.visualViewport;
      const left = (viewport?.offsetLeft ?? 0) + 8;
      const top = (viewport?.offsetTop ?? 0) + 8;
      const width = (viewport?.width ?? window.innerWidth) - 16;
      surface.style.maxWidth = `${Math.max(0, width)}px`;
      surface.style.setProperty("--popover-available-height", `${Math.max(0, rect.top - top - 8)}px`);
      surface.style.maxHeight = `min(var(--popover-max-height, 100vh), ${Math.max(0, rect.top - top - 8)}px)`;
      // Layout dimensions exclude the popover's entry-animation scale.
      surface.style.left = `${Math.max(left, Math.min(rect.left, left + width - surface.offsetWidth))}px`;
      // Keep the bottom edge fixed as the content grows or shrinks upward.
      surface.style.bottom = `${window.innerHeight - rect.top + 8}px`;
    };
    place();
    const onScroll = (event: Event) => {
      if (event.target instanceof Node && surface.contains(event.target)) return;
      place();
    };
    window.addEventListener("resize", place);
    window.addEventListener("scroll", onScroll, true);
    window.visualViewport?.addEventListener("resize", place);
    window.visualViewport?.addEventListener("scroll", place);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", onScroll, true);
      window.visualViewport?.removeEventListener("resize", place);
      window.visualViewport?.removeEventListener("scroll", place);
    };
  }, [anchorRef, surfaceRef]);
  return createPortal(<PopoverSurface {...props} ref={surfaceRef} style={{ color: "var(--text-primary)", ...style, position: "fixed", top: "auto", right: "auto", minWidth: 0, boxSizing: "border-box", overflowY: "auto", zIndex: 1000 }} />, document.body);
}
