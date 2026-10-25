import { ReasoningControl, type ReasoningControlProps } from "./ReasoningControl";
import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { ArrowUp, Plus, Settings2, Square } from "lucide-react";
import { useMenuController } from "@app/hooks/useMenuController";
import { AnchoredPopover } from "@/features/design-system/components/popover/AnchoredPopover";

export function ComposerShell({ children, menu, toolbar, controls, reasoning, actions, className = "" }: {
  children: ReactNode; menu: ReactNode; toolbar?: ReactNode; controls?: ReactNode; reasoning?: ReasoningControlProps; actions: ReactNode; className?: string;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const add = useMenuController({ popoverRef: popover, onDismiss: () => trigger.current?.focus() });
  useEffect(() => {
    if (add.isOpen) popover.current?.querySelector<HTMLButtonElement>(".composer-add-menu button:not(:disabled)")?.focus();
  }, [add.isOpen]);
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const input = root.current?.querySelector("textarea");
    if (!input) return;
    input.style.height = "auto";
    input.style.height = `${Math.min(Math.max(input.scrollHeight, 48), 260)}px`;
  });
  return <div className={`composer-shell ${className}`} ref={root}>
    {children}
    <div className="composer-shell-toolbar">
      <div className="composer-shell-left">
      <div className="composer-add" ref={add.containerRef}>
        <button ref={trigger} type="button" aria-haspopup="dialog" className="ghost icon-button" title="Add attachments and tools" aria-label="Add attachments and tools" aria-expanded={add.isOpen} onClick={add.toggle}><Plus aria-hidden /></button>
        {add.isOpen && <AnchoredPopover anchorRef={trigger} surfaceRef={popover} role="dialog" aria-label="Attachments and tools" className="composer-add-menu" onClick={event => { if ((event.target as HTMLElement).closest("button")) { add.close(); trigger.current?.focus(); } }}>{menu}</AnchoredPopover>}
      </div>
      {toolbar}
      </div>
      <div className="composer-shell-actions">{reasoning && <ReasoningControl {...reasoning} />}{controls}{actions}</div>
    </div>
  </div>;
}

export function ComposerSendButton({ running, disabled, disabledReason, onSend, onStop, label = "Send", runningHint }: {
  running?: boolean; disabled?: boolean; disabledReason: string; onSend: () => void; onStop?: () => void; label?: string; runningHint?: string;
}) {
  const title = running ? `Stop${runningHint ? " · " + runningHint : ""}` : disabled ? disabledReason : `${label} · Enter to send, Shift+Enter for a new line`;
  return <span title={title}><button type="button" className={`composer-action ${running ? "is-stop" : "is-send"}`} aria-label={running ? "Stop" : label} title={title} disabled={!running && disabled} onClick={running ? onStop : onSend}>{running ? <Square aria-hidden /> : <ArrowUp aria-hidden />}</button></span>;
}

export function ComposerAccessControl({ value, onChange, disabled, title }: { value: "ask" | "read-only" | "full-access"; onChange?: (value: "ask" | "read-only" | "full-access") => void; disabled?: boolean; title?: string }) {
  return <label className="composer-chip composer-access-chip" title={title ?? `Access: ${value}`}><Settings2 size={14} aria-hidden /><select aria-label="Access mode" value={value} disabled={disabled || !onChange} onChange={event => onChange?.(event.target.value as typeof value)}><option value="ask">Ask</option><option value="read-only">Read-only</option><option value="full-access">Full access</option></select></label>;
}
