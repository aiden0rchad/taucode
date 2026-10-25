import { useEffect } from "react";
import type { RefObject } from "react";

type UseDismissibleMenuOptions = {
  isOpen: boolean;
  containerRef: RefObject<HTMLElement | null>;
  onClose: () => void;
  closeOnEscape?: boolean;
  popoverRef?: RefObject<HTMLElement | null>;
};

export function useDismissibleMenu({
  isOpen,
  containerRef,
  onClose,
  closeOnEscape = true,
  popoverRef,
}: UseDismissibleMenuOptions) {
  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (target && (containerRef.current?.contains(target) || popoverRef?.current?.contains(target))) {
        return;
      }
      onClose();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!closeOnEscape || event.key !== "Escape") {
        return;
      }
      onClose();
    };

    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [closeOnEscape, containerRef, isOpen, onClose, popoverRef]);
}
