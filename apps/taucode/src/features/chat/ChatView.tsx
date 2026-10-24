import { useEffect, useRef, useState } from "react";
import type { AppSettings } from "@/types";
import { chatRequest, exportMarkdownFile, getAppSettings, updateAppSettings, readChatAttachment } from "@services/tauri";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { Markdown } from "@/features/messages/components/Markdown";
import { modelKey, modelRef } from "@/features/settings/registry";
import { ModeSwitcher, useModeSettings } from "./ModeShell";
import { chatGroups, chatMarkdown, chatTitle, inlineAttachment, readAttachment, type Attachment } from "./chat";
import { useChat } from "./useChat";
import Plus from "lucide-react/dist/esm/icons/plus";
import Trash from "lucide-react/dist/esm/icons/trash-2";
import Download from "lucide-react/dist/esm/icons/download";
import { BrowserDock } from "@/features/browser/BrowserDock";
import { SidebarBottomRail } from "@app/components/SidebarBottomRail";
import { ModeHeader } from "./ModeHeader";
import { RowMenu } from "./RowMenu";

const examples = ["Explain something surprising about the ocean", "Help me plan a relaxing weekend", "Give me three ideas for a short story"];

export function ChatView({ settings, onOpenSettings = () => {} }: { settings: AppSettings; onOpenSettings?: () => void }) {
  const chat = useChat();
  const updateSettings = useModeSettings(settings);
  const models = (settings.providers ?? []).filter(p => p.enabled).flatMap(p => p.models.filter(m => m.enabled).map(m => ({ ...m, key: `${p.id}/${m.id}`, label: `${p.name} · ${m.displayName || m.id}` })));
  const [model, setModel] = useState(modelKey(settings.chatDefaultModel ?? settings.agentPolicy?.defaultModel) || models[0]?.key || "");
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [editing, setEditing] = useState<string>();
  const [search, setSearch] = useState("");
  const [trash, setTrash] = useState(false);
  const [busy, setBusy] = useState(false);
  const [rename, setRename] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [showList, setShowList] = useState(true);
  const [defaultModel, setDefaultModel] = useState(modelKey(settings.chatDefaultModel));
  const [newWeb, setNewWeb] = useState(false);
  const end = useRef<HTMLDivElement>(null);
  const active = chat.chats.find(c => c.id === chat.active);
  const users = chat.messages.filter(m => m.role === "user");
  const lastUser = users[users.length - 1];
  const supportsImages = models.find(m => m.key === model)?.supportsImages !== false;
  const disabled = busy || chat.running;
  const nativeDrop = useRef<(paths: string[]) => void>(() => {});
  useEffect(() => subscribeWindowDragDrop(event => {
    if (event.payload.type === "drop") nativeDrop.current(event.payload.paths ?? []);
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
  const send = () => { if (disabled || (!draft.trim() && !attachments.length)) return; void run(async () => {
    const text = [draft, ...attachments.flatMap(a => a.text ? [a.text] : [])].filter(Boolean).join("\n\n");
    await chat.send(text, attachments.flatMap(a => a.url ? [a.url] : []), model || null, editing, newWeb);
    clearDraft();
  }); };
  const mutate = (action: string, id: string, params: Record<string, unknown> = {}) => run(async () => {
    await chatRequest(action, { threadId: id, ...params });
    if (action === "trash" && chat.active === id) { chat.setActive(null); clearDraft(); }
    await chat.refresh();
  });
  const selectModel = (next: string) => { void run(async () => {
    if (attachments.some(a => a.url) && models.find(m => m.key === next)?.supportsImages === false) throw new Error("Remove images before selecting a text-only model.");
    if (chat.active) { await chat.fork(chat.active, next); setEditing(undefined); }
    setModel(next);
  }); };

  return <BrowserDock searchUrl={settings.browser?.searchUrl} onInterrupt={() => chatRequest("stop", { threadId: chat.active }).then(() => {})}><div className={`chat-layout${showList ? " chat-list-open" : ""}`}>
    <aside className="sidebar chat-sidebar" aria-label="Chats">
      <ModeSwitcher />
      <button className="ghost sidebar-labeled-button mode-new" disabled={busy} onClick={() => { chat.setActive(null); clearDraft(); setNewWeb(false); setModel(defaultModel || modelKey(settings.agentPolicy?.defaultModel) || models[0]?.key || ""); setTrash(false); setShowList(false); }}><Plus size={14} aria-hidden />New chat</button>
      <input type="search" aria-label="Search chats" placeholder="Search chats" value={search} onChange={e => setSearch(e.target.value)} />
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
            </form> : <button className={`thread-row has-secondary-line${chat.active === entry.id ? " active" : ""}${entry.chat.pinned ? " is-pinned" : ""}`} aria-label={chatTitle(entry)} aria-current={chat.active === entry.id ? "page" : undefined} disabled={busy || trash} onClick={() => { void run(async () => { await chat.open(entry.id); clearDraft(); setShowList(false); }); }}>
              <span className={`thread-status ${entry.running ? "processing" : "ready"}`} aria-label={entry.running ? "Running" : "Ready"} />
              <span className="thread-content"><span className="thread-name">{chatTitle(entry)}</span><span className="thread-details">{trash ? "Restore within 30 days" : entry.model || "Chat"}</span></span>
            </button>}
          </RowMenu>)}
        </section>)}
        {!chat.chats.length && <p className="chat-muted">Your conversations will appear here.</p>}
      </div>
      <SidebarBottomRail onOpenSettings={onOpenSettings} onOpenDebug={() => {}} showDebugButton={false}><button className="ghost sidebar-utility-button" aria-label={trash ? "Back to chats" : "Trash"} title={trash ? "Back to chats" : "Trash"} aria-pressed={trash} onClick={() => setTrash(!trash)}><Trash size={14} aria-hidden /></button></SidebarBottomRail>
    </aside>
    <main className="chat-main">
      <ModeHeader title={active ? chatTitle(active) : "New chat"} navigation={<button className="ghost main-header-action chat-list-toggle" aria-expanded={showList} onClick={() => setShowList(!showList)}>☰ Chats</button>}>
        {active && <button className="ghost main-header-action" title="Export as Markdown" aria-label="Export as Markdown" disabled={busy} onClick={() => { void run(async () => { await exportMarkdownFile(chatMarkdown(chatTitle(active), chat.messages), "chat.md", "Export chat as Markdown"); }); }}><Download size={14} aria-hidden /></button>}
      </ModeHeader>
      <div className="chat-scroll" role="log" aria-label="Conversation" aria-live="polite">
        <div className="chat-column">
          {!chat.messages.length && <div className="chat-empty"><span className="chat-eyebrow">A little room to think</span><h1>What’s on your mind?</h1><p>Ask a question, explore an idea, or bring something to read.</p><div className="chat-examples">{examples.map(example => <button key={example} onClick={() => setDraft(example)}>{example}<span aria-hidden>↗</span></button>)}</div></div>}
          {chat.messages.map(message => <article className={`message chat-message ${message.role}`} key={message.id}>
            <h2>{message.role === "user" ? "You" : "Assistant"}</h2>
            {message.images?.map((url, index) => <img key={index} src={url} alt={`Attachment ${index + 1}`} />)}
            <Markdown codeBlockStyle="message" className="markdown" value={message.text} showFilePath={false} />
            {message.id === lastUser?.id && <button disabled={disabled} onClick={() => { setDraft(message.text); setAttachments(message.images?.map((url, i) => ({ name: `Image ${i + 1}`, url })) ?? []); setEditing(message.id); }}>Edit and resend</button>}
          </article>)}
          {lastUser && <button disabled={disabled} onClick={() => { void run(() => chat.send(lastUser.text, lastUser.images ?? [], model || null, lastUser.id)); }}>Regenerate last answer</button>}
          {chat.running && <p role="status" className="chat-muted">Responding…</p>}
          <div ref={end} />
        </div>
      </div>
      <form className="chat-composer chat-column" onSubmit={e => { e.preventDefault(); send(); }} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); if (!disabled) addFiles(Array.from(e.dataTransfer.files)); }}>
        <div className="chat-model-controls"><label>Model <select aria-label="Chat model" value={model} disabled={disabled} onChange={e => selectModel(e.target.value)}>
          {!models.length && <option value="">Enable a provider in Code → Settings</option>}
          {models.map(m => <option value={m.key} key={m.key}>{m.label}</option>)}
        </select></label>
        {settings.mcpServers?.some(c => c.id === "taucode-browser" && c.enabled) && <label><input type="checkbox" aria-label="Web" checked={active ? Boolean(active.chat.web) : newWeb} disabled={disabled} onChange={e => { if (active) { void mutate("web", active.id, { web: e.target.checked }); } else setNewWeb(e.target.checked); }} />Web</label>}
        <button type="button" disabled={disabled || !model} onClick={() => { void run(async () => {
          const value = defaultModel === model ? "" : model;
          const updated = await updateAppSettings({ ...await getAppSettings(), chatDefaultModel: modelRef(value) });
          updateSettings?.(updated);
          setDefaultModel(value);
        }); }}>{defaultModel === model ? "Use policy default" : "Make chat default"}</button></div>
        {editing && <div className="chat-editing">Editing the last message · creates a new branch <button type="button" onClick={clearDraft}>Cancel</button></div>}
        <textarea aria-label="Message" placeholder="Message…" value={draft} disabled={busy} onChange={e => setDraft(e.target.value)} onPaste={e => { const files = Array.from(e.clipboardData.files); if (files.length) { e.preventDefault(); if (!disabled) addFiles(files); } }} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }} />
        {!!attachments.length && <ul className="chat-attachments">{attachments.map((a, index) => <li key={index}>{a.url && <img src={a.url} alt="" />}{a.name}<button type="button" disabled={busy} aria-label={`Remove ${a.name}`} onClick={() => setAttachments(current => current.filter((_, i) => i !== index))}>×</button></li>)}</ul>}
        {chat.error && <p role="alert">{chat.error}</p>}
        <div className="chat-composer-actions"><label className="chat-attach">Attach<input type="file" aria-label="Attach files" multiple disabled={disabled} onChange={e => { addFiles(Array.from(e.target.files ?? [])); e.target.value = ""; }} /></label><span className="chat-muted">Shift + Enter for a new line</span>
          {chat.running ? <button type="button" onClick={() => { void run(() => chatRequest("stop", { threadId: chat.active })); }}>Stop</button> : <button type="submit" disabled={busy || !model || (!draft.trim() && !attachments.length)}>Send ↑</button>}
        </div>
      </form>
    </main>
  </div></BrowserDock>;
}
