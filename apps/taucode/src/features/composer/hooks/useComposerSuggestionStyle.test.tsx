// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useComposerSuggestionStyle } from "./useComposerSuggestionStyle";

vi.mock("../../../utils/caretPosition", () => ({ getCaretPosition: () => ({ left: 0, top: 0, lineHeight: 18 }) }));
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

it("flips below a centered composer and fits above a bottom composer after resize", () => {
  const pane = document.createElement("div");
  pane.className = "chat-pane";
  const container = pane.appendChild(document.createElement("div"));
  container.className = "composer-input";
  const textarea = container.appendChild(document.createElement("textarea"));
  document.body.appendChild(pane);
  vi.spyOn(pane, "getBoundingClientRect").mockReturnValue({ top: 80, bottom: 750 } as DOMRect);
  const bounds = vi.spyOn(container, "getBoundingClientRect").mockReturnValue({ top: 200, bottom: 300, left: 0 } as DOMRect);
  const textareaRef = { current: textarea };
  const { result, rerender } = renderHook(({ open }) => useComposerSuggestionStyle({
    isAutocompleteOpen: open, autocompleteAnchorIndex: 0, selectionStart: 1, text: "/", textareaRef,
  }), { initialProps: { open: true } });
  expect(result.current).toMatchObject({ top: "calc(100% + 8px)", bottom: "auto", maxHeight: 320 });

  bounds.mockReturnValue({ top: 600, bottom: 700, left: 0 } as DOMRect);
  act(() => window.dispatchEvent(new Event("resize")));
  expect(result.current).toMatchObject({ top: "auto", bottom: "calc(100% + 8px)", maxHeight: 320 });

  bounds.mockReturnValue({ top: 180, bottom: 700, left: 0 } as DOMRect);
  act(() => pane.dispatchEvent(new Event("scroll")));
  expect(result.current?.maxHeight).toBe(92);
  rerender({ open: false });
  expect(result.current).toBeUndefined();
});
