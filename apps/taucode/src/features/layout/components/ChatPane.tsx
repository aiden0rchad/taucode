import { useEffect, useRef, type ReactNode } from "react";

type ChatPaneProps = { messagesNode: ReactNode; composerNode: ReactNode; className?: string };

export function ChatPane({ messagesNode, composerNode, className }: ChatPaneProps) {
  const pane = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const composer = pane.current?.querySelector<HTMLElement>(".chat-pane-composer");
    if (!composer || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => pane.current?.style.setProperty("--composer-clearance", `${composer.getBoundingClientRect().height}px`));
    observer.observe(composer); return () => observer.disconnect();
  }, [composerNode]);
  return <div ref={pane} className={`chat-pane${className ? ` ${className}` : ""}`}>
    <div className="chat-pane-messages">{messagesNode}</div>
    {composerNode && <div className="chat-pane-composer">{composerNode}</div>}
  </div>;
}
