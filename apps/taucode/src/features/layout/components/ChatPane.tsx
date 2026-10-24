import type { ReactNode } from "react";

type ChatPaneProps = { messagesNode: ReactNode; composerNode: ReactNode; className?: string };

export function ChatPane({ messagesNode, composerNode, className }: ChatPaneProps) {
  return <div className={`chat-pane${className ? ` ${className}` : ""}`}>
    <div className="chat-pane-messages">{messagesNode}</div>
    {composerNode && <div className="chat-pane-composer">{composerNode}</div>}
  </div>;
}
