/** @vitest-environment jsdom */
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useBrowserContextDraft } from "./useBrowserContextDraft";

describe("browser context drafts", () => {
  it("appends context and PNG to an existing draft without replacing it", () => {
    const onDraftChange = vi.fn(); const attachImages = vi.fn();
    const textarea = document.createElement("textarea"); document.body.append(textarea);
    const { result } = renderHook(() => useBrowserContextDraft({ scope: "workspace:thread", draft: "Keep this question", onDraftChange, attachImages, textareaRef: { current: textarea } }));
    act(() => result.current({ text: "Page: https://example.com\nSelected button", image: "data:image/png;base64,AA==" }));
    expect(onDraftChange).toHaveBeenCalledWith("Keep this question\n\nPage: https://example.com\nSelected button");
    expect(attachImages).toHaveBeenCalledWith(["data:image/png;base64,AA=="]);
    expect(document.activeElement).toBe(textarea);
    textarea.remove();
  });
  it("uses the latest draft text and ignores a capture from a previous scope", () => {
    const onDraftChange = vi.fn(); const attachImages = vi.fn(); const textareaRef = { current: null };
    const { result, rerender } = renderHook(({ scope, draft }) => useBrowserContextDraft({ scope, draft, onDraftChange, attachImages, textareaRef }), { initialProps: { scope: "w:t", draft: "First" } });
    const capture = result.current;
    rerender({ scope: "w:t", draft: "Typed during capture" });
    act(() => capture({ text: "Context" }));
    expect(onDraftChange).toHaveBeenLastCalledWith("Typed during capture\n\nContext");
    rerender({ scope: "other:t", draft: "Different draft" });
    act(() => capture({ text: "Old context", image: "data:image/png;base64,AA==" }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
    expect(attachImages).not.toHaveBeenCalled();
  });
  it("works for a new workspace draft and ignores input without a workspace", () => {
    const onDraftChange = vi.fn(); const attachImages = vi.fn(); const textareaRef = { current: null };
    const { result, rerender } = renderHook(({ scope }: { scope: string | null }) => useBrowserContextDraft({ scope, draft: "", onDraftChange, attachImages, textareaRef }), { initialProps: { scope: "w:draft" as string | null } });
    act(() => result.current({ text: "Selection" }));
    expect(onDraftChange).toHaveBeenCalledWith("Selection");
    rerender({ scope: null });
    act(() => result.current({ text: "Ignored", image: "data:image/png;base64,AA==" }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
    expect(attachImages).not.toHaveBeenCalled();
  });
});
