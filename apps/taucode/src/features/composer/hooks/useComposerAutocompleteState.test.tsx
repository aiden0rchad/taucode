/** @vitest-environment jsdom */
import { createRef } from "react";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useComposerAutocompleteState } from "./useComposerAutocompleteState";

describe("useComposerAutocompleteState file mentions", () => {
  it("suggests a file even if it is already mentioned earlier in the message", () => {
    const files = ["src/App.tsx", "src/main.tsx"];
    const text = "Please review @src/App.tsx and also @";
    const selectionStart = text.length;
    const textareaRef = createRef<HTMLTextAreaElement>();
    textareaRef.current = {
      focus: vi.fn(),
      setSelectionRange: vi.fn(),
    } as unknown as HTMLTextAreaElement;

    const { result } = renderHook(() =>
      useComposerAutocompleteState({
        text,
        selectionStart,
        disabled: false,

        skills: [],

        prompts: [],
        files,
        textareaRef,
        setText: vi.fn(),
        setSelectionStart: vi.fn(),
      }),
    );

    expect(result.current.isAutocompleteOpen).toBe(true);
    expect(result.current.autocompleteMatches.map((item) => item.label)).toContain(
      "src/App.tsx",
    );
  });

  it("marks root-level file suggestions as Files group", () => {
    const files = ["AGENTS.md", "src/main.tsx"];
    const text = "@";
    const selectionStart = text.length;
    const textareaRef = createRef<HTMLTextAreaElement>();
    textareaRef.current = {
      focus: vi.fn(),
      setSelectionRange: vi.fn(),
    } as unknown as HTMLTextAreaElement;

    const { result } = renderHook(() =>
      useComposerAutocompleteState({
        text,
        selectionStart,
        disabled: false,

        skills: [],

        prompts: [],
        files,
        textareaRef,
        setText: vi.fn(),
        setSelectionStart: vi.fn(),
      }),
    );

    const rootItem = result.current.autocompleteMatches.find(
      (item) => item.label === "AGENTS.md",
    );
    expect(rootItem?.group).toBe("Files");
  });
});

describe("useComposerAutocompleteState slash commands", () => {
  it("keeps the basic commands in discovery order", () => {
    const text = "/";
    const selectionStart = text.length;
    const textareaRef = createRef<HTMLTextAreaElement>();
    textareaRef.current = {
      focus: vi.fn(),
      setSelectionRange: vi.fn(),
    } as unknown as HTMLTextAreaElement;

    const { result } = renderHook(() =>
      useComposerAutocompleteState({
        text,
        selectionStart,
        disabled: false,

        skills: [],

        prompts: [],
        files: [],
        textareaRef,
        setText: vi.fn(),
        setSelectionStart: vi.fn(),
      }),
    );

    const labels = result.current.autocompleteMatches.map((item) => item.label);
    expect(labels).toEqual(
      expect.arrayContaining([
        "compact",
        "goal",
        "new",
        "plan",
        "sidechat",
      ]),
    );
    expect(labels).toEqual([
      "sidechat",
      "plan",
      "goal",
      "new",
      "compact",
    ]);
  });

  it("does not offer app commands inside prose or paths", () => {
    const { result, rerender } = renderHook(({ text }) => useComposerAutocompleteState({
      text, selectionStart: text.length, disabled: false, skills: [], prompts: [], files: [],
      textareaRef: createRef(), setText: vi.fn(), setSelectionStart: vi.fn(),
    }), { initialProps: { text: "Look at /" } });
    expect(result.current.isAutocompleteOpen).toBe(false);
    rerender({ text: "/Users/" });
    expect(result.current.isAutocompleteOpen).toBe(false);
  });

  it("inserts a readable plan without the slash and preserves trailing draft text", () => {
    const setText = vi.fn();
    const { result } = renderHook(() => useComposerAutocompleteState({
      text: "/plan build a timer", selectionStart: 5, disabled: false, skills: [], prompts: [], files: [],
      textareaRef: createRef(), setText, setSelectionStart: vi.fn(),
    }));
    act(() => result.current.applyAutocomplete(result.current.autocompleteMatches[0]));
    expect(setText).toHaveBeenCalledWith(expect.stringMatching(/^Plan: build a timer\n\nPlan this task/));
  });


});
