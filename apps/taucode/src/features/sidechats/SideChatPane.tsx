import { useFeatureSettings } from "@settings/features";
import { formatModelRef } from "@settings/registry";
import { ArrowLeft, MessagesSquare, Shield } from "lucide-react";
import { ComposerShell, ComposerSendButton } from "@/features/composer/components/ComposerShell";
import { useState, type ComponentProps, type ReactNode } from "react";
import type { AccessMode, ConversationItem, ModelOption } from "@/types";
import { Messages } from "@/features/messages/components/Messages";
import { RowMenu } from "@/features/chat/RowMenu";
import { useReasoningSelection } from "@/features/models/reasoning";
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

export function SideChatPane({ chat, items, running, processingStartedAt = null, lastDurationMs = null, models, parentModel, messageProps, onSend, onInterrupt, onClose, onDelete, onRename, onOpenSplit, onSendToMain }: {
  chat: OpenSideChat; items: ConversationItem[]; running: boolean; models: ModelOption[];
  processingStartedAt?: number | null; lastDurationMs?: number | null;
  parentModel?: ModelOption | null;
  messageProps: Pick<ComponentProps<typeof Messages>, "openTargets" | "selectedOpenAppId" | "workspacePath" | "userInputRequests" | "onUserInputSubmit">;
  onSend: (text: string, model: string | null, access: AccessMode, effort?: string | null) => Promise<{ status: string }>;
  onInterrupt: () => Promise<unknown>; onClose: () => void; onDelete: () => void; onSendToMain: (text: string) => void;
  onRename?: () => void; onOpenSplit?: () => void;
}) {
  const settings = useFeatureSettings();
  const [draft, setDraft] = useState("");
  const [modelOverride, setModel] = useState<string | null>(null);
  const inheritedId = chat.model ?? parentModel?.id ?? null;
  const inheritedModel = models.find(entry => entry.id === inheritedId || entry.model === inheritedId) ?? (parentModel?.id === inheritedId ? parentModel : undefined);
  const model = modelOverride ?? inheritedModel?.id ?? inheritedId;
  const selectedModel = models.find(entry => entry.id === model) ?? (inheritedModel?.id === model ? inheritedModel : undefined);
  const capability = chat.engine === "claude" ? selectedModel?.acpReasoning : selectedModel?.reasoning;
  const reasoning = useReasoningSelection(model ?? "", chat.id, capability);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const send = async () => {
    if (!draft.trim() || sending || running) return;
    setSending(true); setError(null);
    try { const result = await onSend(draft, model, "read-only", reasoning.level); if (result.status === "sent") setDraft(""); else setError("Message could not be sent. Your draft is preserved."); }
    catch (e) { setError(String(e)); }
    finally { setSending(false); }
  };
  return <div className="side-chat-pane">
    <header className="side-chat-header"><MessagesSquare size={16} aria-hidden /><strong title={chat.title}>{chat.title}</strong><span className="thread-header-model" title={formatModelRef(settings, model).title}>{chat.engine} · {formatModelRef(settings, model).name}</span><span className="side-chat-status" role="status">{running ? "Running" : "Idle"}</span><RowMenu label="side chat" actions={[
      { label: "Rename", onClick: () => onRename?.(), disabled: !onRename },
      { label: "Archive", onClick: onDelete },
      { label: "Open in split view", onClick: () => onOpenSplit?.(), disabled: !onOpenSplit },
    ]}><span /></RowMenu></header>
    <div className="side-chat-messages"><Messages {...messageProps} items={items} threadId={chat.id} workspaceId={chat.workspaceId} isThinking={running} processingStartedAt={processingStartedAt} lastDurationMs={lastDurationMs} onSendToMain={onSendToMain} /></div>
    <form className="side-chat-composer" onSubmit={e => { e.preventDefault(); void send(); }}>
      <ComposerShell menu={<button type="button" onClick={onClose}><ArrowLeft aria-hidden />Close side chat</button>}
        reasoning={{ modelName: selectedModel?.displayName || formatModelRef(settings, model).name, models: models.filter(m => !m.engines || m.engines.includes(chat.engine)).map(m => ({ id: m.id, name: m.displayName, provider: m.providerName, note: m.id === (parentModel?.id ?? inheritedModel?.id) ? "Same as parent" : undefined })), selectedModelId: model, onSelectModel: setModel, capability, value: reasoning.requested, onChange: reasoning.onChange, disabled: running || sending }}
        toolbar={<span className="composer-chip" title="Side chats use read-only access"><Shield aria-hidden />Read-only</span>} actions={<ComposerSendButton running={running} disabled={sending || !draft.trim()} disabledReason={sending ? "Sending message" : "Write a message"} onSend={() => { void send(); }} onStop={() => { void onInterrupt().catch(e => setError(String(e))); }} />}>
      <label className="side-chat-prompt"><textarea rows={1} placeholder="Ask anything" aria-label="Side chat message" value={draft} onChange={e => setDraft(e.target.value)} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void send(); } }} /></label>
      {error && <p role="alert">{error}</p>}
      </ComposerShell>
    </form>
  </div>;
}
