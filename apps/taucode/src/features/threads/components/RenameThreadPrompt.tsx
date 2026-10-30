import { useEffect, useRef } from "react";
import { ModalShell } from "../../design-system/components/modal/ModalShell";

type RenameThreadPromptProps = {
  title?: string;
  busy?: boolean;
  error?: string;
  maxLength?: number;
  currentName: string;
  name: string;
  onChange: (value: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
};

export function RenameThreadPrompt({
  title = "Rename thread",
  busy = false,
  error,
  maxLength,
  currentName,
  name,
  onChange,
  onCancel,
  onConfirm,
}: RenameThreadPromptProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const previous = document.activeElement;
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, []);

  useEffect(() => {
    if (busy) return;
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [busy]);

  return (
    <ModalShell
      className="worktree-modal"
      onBackdropClick={busy ? undefined : onCancel}
      ariaLabel={title}
      onKeyDown={event => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); if (!busy) onCancel(); }
        if (event.key !== "Tab") return;
        const fields = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("input:not(:disabled), button:not(:disabled)"));
        const first = fields[0];
        const last = fields[fields.length - 1];
        if (!first || event.shiftKey && document.activeElement === first || !event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          (event.shiftKey ? last : first)?.focus();
        }
      }}
    >
      <div className="ds-modal-title worktree-modal-title">{title}</div>
      <div className="ds-modal-subtitle worktree-modal-subtitle">Current name: "{currentName}"</div>
      <label className="ds-modal-label worktree-modal-label" htmlFor="thread-rename">
        New name
      </label>
      <input
        id="thread-rename"
        ref={inputRef}
        className="ds-modal-input worktree-modal-input"
        value={name}
        readOnly={busy}
        maxLength={maxLength}
        aria-invalid={Boolean(error)}
        aria-describedby={error ? "thread-rename-error" : undefined}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (busy) return;
          if (event.key === "Enter" && !event.nativeEvent.isComposing && name.trim()) {
            event.preventDefault();
            onConfirm();
          }
        }}
      />
      {error && <div id="thread-rename-error" className="ds-modal-error" role="alert">{error}</div>}
      <div className="ds-modal-actions worktree-modal-actions">
        <button
          className="ghost ds-modal-button worktree-modal-button"
          onClick={onCancel}
          type="button"
          disabled={busy}
        >
          Cancel
        </button>
        <button
          className="primary ds-modal-button worktree-modal-button"
          onClick={onConfirm}
          type="button"
          disabled={busy || name.trim().length === 0}
        >
          {busy ? "Renaming…" : "Rename"}
        </button>
      </div>
    </ModalShell>
  );
}
