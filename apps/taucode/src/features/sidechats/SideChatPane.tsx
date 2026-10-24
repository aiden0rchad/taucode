import { useState, type ComponentProps, type ReactNode } from "react";
import type { AccessMode, ConversationItem, ModelOption } from "@/types";
import { Messages } from "@/features/messages/components/Messages";
import type { OpenSideChat } from "./useSideChats";
import "./sidechats.css";

export function SideChatSplit({ main, children, compact = false }: { main: ReactNode; children: ReactNode; compact?: boolean }) {
  const [width, setWidth] = useState(45);
  return <div className="side-chat-split">
    {!compact && <div className="side-chat-main">{main}</div>}
    {!compact && <div role="separator" aria-label="Resize side chat" aria-orientation="vertical" aria-valuenow={width} aria-valuemin={25} aria-valuemax={70} tabIndex={0} className="side-chat-divider"
      onKeyDown={e => { if (e.key === "ArrowLeft" || e.key === "ArrowRight") { e.preventDefault(); setWidth(w => Math.min(70, Math.max(25, w + (e.key === "ArrowLeft" ? 5 : -5)))); } }}
      onPointerDown={e => e.currentTarget.setPointerCapture(e.pointerId)}
      onPointerMove={e => {
        if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
        const bounds = e.currentTarget.parentElement!.getBoundingClientRect();
        setWidth(Math.min(70, Math.max(25, (bounds.right - e.clientX) / bounds.width * 100)));
      }} onPointerUp={e => e.currentTarget.releasePointerCapture(e.pointerId)} />}
    <aside className="side-chat-sheet" aria-label="Side chat" style={{ flexBasis: compact ? "100%" : `${width}%` }}>{children}</aside>
  </div>;
}

export function SideChatPane({ chat, items, running, models, messageProps, onSend, onInterrupt, onClose, onDelete, onSendToMain }: {
  chat: OpenSideChat; items: ConversationItem[]; running: boolean; models: ModelOption[];
  messageProps: Pick<ComponentProps<typeof Messages>, "openTargets" | "selectedOpenAppId" | "workspacePath" | "userInputRequests" | "onUserInputSubmit">;
  onSend: (text: string, model: string | null, access: AccessMode) => Promise<{ status: string }>;
  onInterrupt: () => Promise<unknown>; onClose: () => void; onDelete: () => void; onSendToMain: (text: string) => void;
}) {
  const [draft, setDraft] = useState("");
  const [model, setModel] = useState(chat.model);
  const [access, setAccess] = useState<AccessMode>("read-only");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const send = async () => {
    if (!draft.trim() || sending || running) return;
    setSending(true); setError(null);
    try { const result = await onSend(draft, model, access); if (result.status === "sent") setDraft(""); else setError("Message could not be sent. Your draft is preserved."); }
    catch (e) { setError(String(e)); }
    finally { setSending(false); }
  };
  return <>
    <header className="side-chat-header"><button className="ghost" onClick={onClose} aria-label="Close side chat">← Back</button><strong>{chat.title}</strong><span role="status">{running ? "Running" : "Idle"}</span><button className="ghost" onClick={onDelete}>Archive</button></header>
    <Messages {...messageProps} items={items} threadId={chat.id} workspaceId={chat.workspaceId} isThinking={running} onSendToMain={onSendToMain} />
    <form className="side-chat-composer" onSubmit={e => { e.preventDefault(); void send(); }}>
      <div className="side-chat-controls">
        <label>Model<select value={model ?? ""} onChange={e => setModel(e.target.value)} disabled={running}>
          {!models.some(m => m.id === model) && <option value={model ?? ""}>{model ?? "Parent model"}</option>}
          {models.filter(m => !m.engines || m.engines.includes(chat.engine)).map(m => <option key={m.id} value={m.id}>{m.displayName}</option>)}
        </select></label>
        <label>Access<select value={access} onChange={e => setAccess(e.target.value as AccessMode)} disabled={running}>
          <option value="read-only">Read-only</option><option value="current">Workspace</option><option value="full-access">Full access</option>
        </select></label>
      </div>
      <label className="side-chat-prompt">Message<textarea aria-label="Side chat message" value={draft} onChange={e => setDraft(e.target.value)} onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === "Enter") { e.preventDefault(); void send(); } }} /></label>
      {error && <p role="alert">{error}</p>}
      {running ? <button type="button" onClick={() => { void onInterrupt().catch(e => setError(String(e))); }}>Stop side chat</button> : <button type="submit" disabled={sending || !draft.trim()}>Send</button>}
    </form>
  </>;
}
