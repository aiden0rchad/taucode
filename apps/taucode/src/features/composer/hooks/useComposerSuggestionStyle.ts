import { useLayoutEffect, useState, type CSSProperties, type RefObject } from "react";
import { getCaretPosition } from "../../../utils/caretPosition";

const CARET_ANCHOR_GAP = 8;

type UseComposerSuggestionStyleArgs = {
  isAutocompleteOpen: boolean;
  autocompleteAnchorIndex: number | null;
  selectionStart: number | null;
  text: string;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
};

export function useComposerSuggestionStyle({
  isAutocompleteOpen,
  autocompleteAnchorIndex,
  selectionStart,
  text,
  textareaRef,
}: UseComposerSuggestionStyleArgs) {
  const [suggestionsStyle, setSuggestionsStyle] = useState<CSSProperties | undefined>(
    undefined,
  );

  useLayoutEffect(() => {
    if (!isAutocompleteOpen) {
      setSuggestionsStyle(undefined);
      return;
    }
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }
    const place = () => {
      const cursor = autocompleteAnchorIndex ?? textarea.selectionStart ?? selectionStart ?? text.length;
      const caret = getCaretPosition(textarea, cursor);
      if (!caret) return;
      const textareaRect = textarea.getBoundingClientRect();
      const container = textarea.closest(".composer-input");
      const containerRect = container?.getBoundingClientRect();
      const offsetLeft = textareaRect.left - (containerRect?.left ?? 0);
      const containerWidth = container?.clientWidth ?? textarea.clientWidth ?? 0;
      const popoverWidth = Math.min(containerWidth, 420);
      const rawLeft = offsetLeft + caret.left;
      const maxLeft = Math.max(0, containerWidth - popoverWidth);
      const left = Math.min(Math.max(0, rawLeft), maxLeft);
      const pane = textarea.closest(".chat-pane")?.getBoundingClientRect();
      const above = Math.max(0, (containerRect?.top ?? textareaRect.top) - Math.max(0, pane?.top ?? 0) - CARET_ANCHOR_GAP);
      const below = Math.max(0, Math.min(window.innerHeight, pane?.bottom ?? window.innerHeight) - (containerRect?.bottom ?? textareaRect.bottom) - CARET_ANCHOR_GAP);
      const opensBelow = above < 320 && below > above;
      setSuggestionsStyle({
        left,
        right: "auto",
        bottom: opensBelow ? "auto" : `calc(100% + ${CARET_ANCHOR_GAP}px)`,
        top: opensBelow ? `calc(100% + ${CARET_ANCHOR_GAP}px)` : "auto",
        maxHeight: Math.min(320, opensBelow ? below : above),
      });
    };
    place();
    window.addEventListener("resize", place);
    const pane = textarea.closest(".chat-pane");
    pane?.addEventListener("scroll", place);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(place);
    observer?.observe(textarea);
    if (pane) observer?.observe(pane);
    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", place);
      pane?.removeEventListener("scroll", place);
    };
  }, [autocompleteAnchorIndex, isAutocompleteOpen, selectionStart, text, textareaRef]);

  return suggestionsStyle;
}
