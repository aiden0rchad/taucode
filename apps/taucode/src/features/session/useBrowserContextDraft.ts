import { useCallback, useRef, type RefObject } from "react";

export function useBrowserContextDraft({ scope, draft, onDraftChange, attachImages, textareaRef }: {
  scope: string | null;
  draft: string;
  onDraftChange: (text: string) => void;
  attachImages: (images: string[]) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
}) {
  const latest = useRef({ scope, draft });
  latest.current = { scope, draft };
  return useCallback((context: { text: string; image?: string }) => {
    // A screenshot captured before navigation must not land in another draft.
    if (!scope || latest.current.scope !== scope) return;
    if (context.text) {
      const text = [latest.current.draft, context.text].filter(Boolean).join("\n\n");
      latest.current.draft = text;
      onDraftChange(text);
    }
    if (context.image) attachImages([context.image]);
    textareaRef.current?.focus();
  }, [scope, onDraftChange, attachImages, textareaRef]);
}
