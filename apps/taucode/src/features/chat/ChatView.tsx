import { DigitalRain } from "@/features/themes/DigitalRain";
import { threadEffortKey, useReasoningSelection } from "@/features/models/reasoning";
import { SidebarResizeHandle } from "@/features/layout/components/SidebarResizeHandle";
import { useSidebarState } from "@/features/layout/hooks/useSidebarState";
import { ComposerDictation } from "@/features/composer/components/ComposerDictation";
import { NewConversationRow } from "@/features/chat/NewConversationRow";
import { formatRelativeTimeShort } from "@utils/time";
import { Globe, X, Lightbulb, Calendar, PenLine, Pencil, RotateCcw, Copy, Search } from "lucide-react";
import { ComposerShell, ComposerSendButton, ComposerAccessControl } from "@/features/composer/components/ComposerShell";
import { getFileTypeIconUrl } from "@utils/fileTypeIcons";
import { useEffect, useRef, useState } from "react";
import type { AppSettings } from "@/types";
import { chatRequest, exportMarkdownFile, getAppSettings, updateAppSettings, readChatAttachment } from "@services/tauri";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { Markdown } from "@/features/messages/components/Markdown";
import { modelKey, modelRef } from "@/features/settings/registry";
import { ModeSwitcher, useModeSettings } from "./ModeShell";
import { chatGroups, chatMarkdown, chatTitle, inlineAttachment, readAttachment, type Attachment } from "./chat";
import { useChat } from "./useChat";
import Trash from "lucide-react/dist/esm/icons/trash-2";
import { WorkspaceShell } from "@/features/panels/SidePanel";
import { SidebarBottomRail } from "@app/components/SidebarBottomRail";
import { RowMenu } from "./RowMenu";

const examples = [{ label: "Explore an idea", prompt: "Explain something surprising about the ocean", Icon: Lightbulb }, { label: "Plan a weekend", prompt: "Help me plan a relaxing weekend", Icon: Calendar }, { label: "Write a story", prompt: "Give me three ideas for a short story", Icon: PenLine }];

export function ChatView({ settings, onOpenSettings = () => {} }: { settings: AppSettings; onOpenSettings?: () => void }) {
  const chat = useChat();
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const updateSettings = useModeSettings(settings);
  const models = (settings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ ...m, provider: p.name, key: `${p.id}/${m.id}`, label: `${p.name} · ${m.displayName || m.id}` })));
  const [model, setModel] = useState(modelKey(settings.chatDefaultModel ?? settings.agentPolicy?.defaultModel) || models[0]?.key || "");
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [editing, setEditing] = useState<string>();
  const searchInput = useRef<HTMLInputElement>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [trash, setTrash] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dictating, setDictating] = useState(false);
  const [rename, setRename] = useState<string | null>(null);
  const [name, setName] = useState("");

  const [defaultModel, setDefaultModel] = useState(modelKey(settings.chatDefaultModel));
  const [newWeb, setNewWeb] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const end = useRef<HTMLDivElement>(null);
  const active = chat.chats.find(c => c.id === chat.active);
  const capability = models.find(m => m.key === model)?.reasoning;
  const reasoning = useReasoningSelection(model, chat.active, capability);
  const users = chat.messages.filter(m => m.role === "user");
  const lastUser = users[users.length - 1];
  const supportsImages = models.find(m => m.key === model)?.supportsImages !== false;
  const disabled = busy || chat.running;
  const nativeDrop = useRef<(paths: string[]) => void>(() => {});
  useEffect(() => {
    if (import.meta.env.MODE === "uipreview" && new URLSearchParams(window.location.search).get("composer") === "attachments") {
      void import("@/dev/uiPreview/fixtures").then(({ composerAttachments }) => setAttachments(composerAttachments));
    }
  }, []);
  useEffect(() => subscribeWindowDragDrop(event => {
    setDragOver(event.payload.type === "enter" || event.payload.type === "over"); if (event.payload.type === "drop") nativeDrop.current(event.payload.paths ?? []);
  }), []);

  useEffect(() => { if (active?.model) setModel(`${active.modelProvider}/${active.model}`); }, [active?.id, active?.model, active?.modelProvider]);
  useEffect(() => { end.current?.scrollIntoView?.({ block: "end" }); }, [chat.messages]);
  const run = async (operation: () => Promise<unknown>) => {
    setBusy(true); chat.setError("");
    try { await operation(); return true; } catch (e) { chat.setError(String(e)); return false; } finally { setBusy(false); }
  };
  const clearDraft = () => { setDraft(""); setAttachments([]); setEditing(undefined); };
  const addFiles = (files: File[]) => { void run(async () => {
    if (attachments.length + files.length > 31) throw new Error("Attach at most 31 files.");
    const added = await Promise.all(files.map(file => readAttachment(file, supportsImages)));
    setAttachments(current => [...current, ...added]);
  }); };
  nativeDrop.current = paths => {
    if (disabled || !paths.length) return;
    void run(async () => {
      if (attachments.length + paths.length > 31) throw new Error("Attach at most 31 files.");
      const added = await Promise.all(paths.map(path => readChatAttachment(path)));
      if (!supportsImages && added.some(a => a.url)) throw new Error("This model does not support images.");
      setAttachments(current => [...current, ...added.map(a => a.text !== undefined ? { ...a, text: inlineAttachment(a.name, a.text) } : a)]);
    });
  };
  const send = () => { if (disabled || dictating || (!draft.trim() && !attachments.length)) return; void run(async () => {
    const text = [draft, ...attachments.flatMap(a => a.text ? [a.text] : [])].filter(Boolean).join("\n\n");
    await chat.send(text, attachments.flatMap(a => a.url ? [a.url] : []), model || null, editing, newWeb, reasoning.level);
    clearDraft();
  }); };
  const mutate = (action: string, id: string, params: Record<string, unknown> = {}) => run(async () => {
    await chatRequest(action, { threadId: id, ...params });
    if (action === "trash" && chat.active === id) { chat.setActive(null); clearDraft(); }
    await chat.refresh();
  });
  const selectModel = (next: string) => run(async () => {
    if (attachments.some(a => a.url) && models.find(m => m.key === next)?.supportsImages === false) throw new Error("Remove images before selecting a text-only model.");
    if (chat.active) { const id = await chat.fork(chat.active, next); if (reasoning.requested || reasoning.level) localStorage.setItem(threadEffortKey(id), reasoning.requested || reasoning.level!); setEditing(undefined); }
    setModel(next);
  });

  const web = active ? Boolean(active.chat.web) : newWeb;
  const toggleWeb = () => { if (active) { void mutate("web", active.id, { web: !web }); } else setNewWeb(!web); };

  return <WorkspaceShell titleActions={<span className="thread-header-model" title={`pi · ${model}`}>pi · {model}</span>} reservedShortcuts={Object.entries(settings).filter(([key, value]) => key.endsWith("Shortcut") && typeof value === "string").map(([, value]) => String(value))} scope={`chat:${chat.active ?? "new"}`} title={active ? chatTitle(active) : "New chat"} onNavigate={scope => { const id = scope.slice(5); if (id === "new") chat.setActive(null); else void chat.open(id); }} actions={active ? [{ label: "Rename", onClick: () => { setRename(active.id); setName(chatTitle(active)); } }, { label: "Copy", onClick: () => { void navigator.clipboard.writeText(chatMarkdown(chatTitle(active), chat.messages)).catch(e => chat.setError(String(e))); } }, { label: "Export", onClick: () => { void run(() => exportMarkdownFile(chatMarkdown(chatTitle(active), chat.messages), "chat.md", "Export chat")); } }, { label: "Move to Trash", onClick: () => { void mutate("trash", active.id); } }] : []} searchUrl={settings.browser?.searchUrl} onInterrupt={() => chatRequest("stop", { threadId: chat.active }).then(() => {})}><div className={`chat-layout${sidebarCollapsed ? " sidebar-collapsed" : ""}`}>
    <SidebarResizeHandle />
    <aside className="sidebar chat-sidebar" aria-label="Chats">
      <ModeSwitcher />
      {!searchOpen && <NewConversationRow label="New chat" disabled={busy} shortcut={settings.newAgentShortcut} handleShortcut onSearch={() => setSearchOpen(true)} onNew={() => { chat.setActive(null); clearDraft(); setNewWeb(false); setModel(defaultModel || modelKey(settings.agentPolicy?.defaultModel) || models[0]?.key || ""); setTrash(false); if (window.innerWidth < 760) setSidebarCollapsed(true); }} />}
      {searchOpen && <label className="sidebar-search is-open"><Search aria-hidden /><input autoFocus onKeyDown={e => { if (e.key === "Escape") { setSearch(""); setSearchOpen(false); } }} ref={searchInput} type="search" aria-label="Search chats" placeholder="Search chats" value={search} onChange={e => setSearch(e.target.value)} /><button type="button" className="ghost icon-button" aria-label="Close search" onClick={() => { setSearch(""); setSearchOpen(false); }}><X aria-hidden /></button></label>}
      <div className="chat-list">
        {Object.entries(chatGroups(chat.chats, search, trash)).map(([group, entries]) => entries.length > 0 && <section key={group} aria-label={group}>
          <h2 className="sidebar-section-header"><span className="sidebar-section-title">{group}</span></h2>
          {entries.map(entry => <RowMenu key={entry.id} label={chatTitle(entry)} actions={trash ? [{ label: "Restore", disabled: busy || Date.now() / 1000 - (entry.chat.deletedAt ?? 0) > 30 * 86400, onClick: () => { void mutate("restore", entry.id); } }] : [
            { label: entry.chat.pinned ? "Unpin" : "Pin", disabled: busy, onClick: () => { void mutate("pin", entry.id, { pinned: !entry.chat.pinned }); } },
            { label: "Rename", disabled: busy, onClick: () => { setRename(entry.id); setName(chatTitle(entry)); } },
            { label: "Delete", disabled: busy || entry.running, onClick: () => { void mutate("trash", entry.id); } },
          ]}>
            {rename === entry.id ? <form onSubmit={e => { e.preventDefault(); void mutate("rename", entry.id, { title: name }).then(success => { if (success) setRename(null); }); }}>
              <input autoFocus aria-label="Chat title" maxLength={200} value={name} onChange={e => setName(e.target.value)} /><button disabled={busy || !name.trim()}>Save</button><button type="button" onClick={() => setRename(null)}>Cancel</button>
            </form> : <button className={`thread-row has-secondary-line${chat.active === entry.id ? " active" : ""}${entry.chat.pinned ? " is-pinned" : ""}`} title={`${chatTitle(entry)} · pi · ${entry.modelProvider}/${entry.model}`} data-depth={0} aria-label={chatTitle(entry)} aria-current={chat.active === entry.id ? "page" : undefined} disabled={busy || trash} onClick={() => { void run(async () => { await chat.open(entry.id); clearDraft(); if (window.innerWidth < 760) setSidebarCollapsed(true); }); }}>
              <span className={`thread-status ${entry.running ? "processing" : "ready"}`} aria-label={entry.running ? "Running" : "Ready"} />
              <span className="thread-content"><span className="thread-name">{chatTitle(entry)}</span><span className="thread-details">{trash ? "Restore within 30 days" : entry.model || "Chat"}</span></span><span className="thread-time">{formatRelativeTimeShort(entry.updatedAt * 1000)}</span>
            </button>}
          </RowMenu>)}
        </section>)}
        {!chat.chats.length && <p className="chat-muted">Your conversations will appear here.</p>}
      </div>
      <SidebarBottomRail onOpenSettings={onOpenSettings} onOpenDebug={() => {}} showDebugButton={false}><button className="ghost sidebar-utility-button" aria-label={trash ? "Back to chats" : "Trash"} title={trash ? "Back to chats" : "Trash"} aria-pressed={trash} onClick={() => setTrash(!trash)}><Trash size={14} aria-hidden /></button></SidebarBottomRail>
    </aside>
    <main className={`chat-main${!chat.messages.length ? " is-empty" : ""}`}>

      <div className="chat-scroll" role="log" aria-label="Conversation" aria-live="polite">
        <div className="chat-column">
          {!chat.messages.length && <div className="chat-empty"><DigitalRain /><span className="chat-eyebrow">A little room to think</span><h1>What’s on your mind?</h1><p>Ask a question, explore an idea, or bring something to read.</p></div>}
          {chat.messages.map(message => <article className={`message chat-message ${message.role}`} key={message.id}>
            <div className="chat-message-body">
            <h2>{message.role === "user" ? "You" : "Assistant"}</h2>
            {message.images?.map((url, index) => <img key={index} src={url} alt={`Attachment ${index + 1}`} />)}
            <Markdown codeBlockStyle="message" className="markdown" value={message.text} showFilePath={false} />
            </div>
            <div className="message-hover-actions"><button type="button" className="ghost icon-button" aria-label="Copy message" title="Copy message" onClick={() => { void navigator.clipboard.writeText(message.text).catch(() => chat.setError("Could not copy message.")); }}><Copy aria-hidden /></button>{message.id === lastUser?.id && <button className="ghost icon-button" aria-label="Edit and resend" title="Edit and resend" disabled={disabled} onClick={() => { setDraft(message.text); setAttachments(message.images?.map((url, i) => ({ name: `Image ${i + 1}`, url })) ?? []); setEditing(message.id); }}><Pencil aria-hidden /></button>}{message.role === "assistant" && message.id === chat.messages[chat.messages.length - 1]?.id && lastUser && <button className="ghost icon-button" aria-label="Regenerate last answer" title="Regenerate last answer" disabled={disabled} onClick={() => { void run(() => chat.send(lastUser.text, lastUser.images ?? [], model || null, lastUser.id, web, reasoning.level)); }}><RotateCcw aria-hidden /></button>}</div>
          </article>)}

          {chat.running && <p role="status" className="chat-muted">Responding…</p>}
          <div ref={end} />
        </div>
      </div>
      <form className="chat-composer chat-column" onSubmit={e => { e.preventDefault(); send(); }} onDragOver={e => { e.preventDefault(); setDragOver(true); }} onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragOver(false); }} onDrop={e => { e.preventDefault(); setDragOver(false); if (!disabled) addFiles(Array.from(e.dataTransfer.files)); }}>
        <input ref={fileInput} hidden type="file" aria-label="Attach files" multiple disabled={disabled} onChange={e => { addFiles(Array.from(e.target.files ?? [])); e.target.value = ""; }} />
        <ComposerShell reasoning={{ modelName: models.find(m => m.key === model)?.displayName || models.find(m => m.key === model)?.id, models: models.map(m => ({ id: m.key, name: m.displayName || m.id, provider: m.provider })), selectedModelId: model, defaultModelId: settings.agentPolicy?.defaultModel ? settings.agentPolicy.defaultModel.providerId + "/" + settings.agentPolicy.defaultModel.modelId : undefined, onSelectModel: selectModel, capability, value: reasoning.requested, onChange: reasoning.onChange, disabled, onOpenSettings }} menu={<><button type="button" disabled={disabled} onClick={() => fileInput.current?.click()}>Attach files or images</button>{settings.mcpServers?.some(c => c.id === "taucode-browser" && c.enabled) && <button type="button" disabled={disabled} aria-pressed={web} onClick={toggleWeb}><Globe aria-hidden />{web ? "Turn off Web" : "Turn on Web"}</button>}<button type="button" disabled={disabled || !model} onClick={() => { void run(async () => {
          const value = defaultModel === model ? "" : model;
          const updated = await updateAppSettings({ ...await getAppSettings(), chatDefaultModel: modelRef(value) });
          updateSettings?.(updated);
          setDefaultModel(value);
        }); }}>{defaultModel === model ? "Use policy default" : "Make chat default"}</button></>} toolbar={<><ComposerAccessControl value="read-only" title="Chat has no file or command access" />{web && <button type="button" className="composer-chip" disabled={disabled} onClick={toggleWeb} title="Turn off Web"><Globe aria-hidden />Web<X aria-hidden /></button>}</>} actions={<> <ComposerDictation settings={settings} disabled={busy || chat.running} onBusyChange={setDictating} onTranscript={text => setDraft(current => current + (current && !/\s$/.test(current) ? " " : "") + text)} onError={chat.setError} onOpenSettings={onOpenSettings} /><ComposerSendButton running={chat.running} disabled={busy || dictating || !model || (!draft.trim() && !attachments.length)} disabledReason={dictating ? "Finish dictation first" : busy ? "Please wait" : !model ? "Choose a model in Settings" : "Write a message or add an attachment"} onSend={send} onStop={() => { void run(() => chatRequest("stop", { threadId: chat.active })); }} /></>}>
        {dragOver && <div className="composer-drop-overlay">Drop files or images</div>}
        {!!attachments.length && <ul className="chat-attachments">{attachments.map((a, index) => <li key={index}><img src={a.url || getFileTypeIconUrl(a.name)} alt="" />{a.name}<button type="button" disabled={busy} aria-label={`Remove ${a.name}`} onClick={() => setAttachments(current => current.filter((_, i) => i !== index))}><X aria-hidden /></button></li>)}</ul>}
        {editing && <div className="chat-editing">Editing the last message · creates a new branch <button type="button" onClick={clearDraft}>Cancel</button></div>}
        <textarea aria-label="Message" placeholder="Ask anything" rows={1} value={draft} disabled={busy} onChange={e => setDraft(e.target.value)} onPaste={e => { const files = Array.from(e.clipboardData.files); if (files.length) { e.preventDefault(); if (!disabled) addFiles(files); } }} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }} />

        {chat.error && <p role="alert">{chat.error}</p>}
        </ComposerShell>
      </form>
      {!chat.messages.length && <div className="chat-examples starter-pills">{examples.map(({ label, prompt, Icon }) => <button key={label} onClick={() => setDraft(prompt)}><Icon aria-hidden />{label}</button>)}</div>}
    </main>
  </div></WorkspaceShell>;
}
