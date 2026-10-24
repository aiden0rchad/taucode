import { ModeAppShell, PrimaryAction } from "@app/components/AppShell";
import { MainTopbar } from "@app/components/MainTopbar";
import { Button, Select, TextArea, Card, EmptyState, ListRow } from "@/features/design-system/components/Primitives";
import { useEffect, useRef, useState } from "react";
import type { AppSettings } from "@/types";
import { chatRequest, exportMarkdownFile, getAppSettings, updateAppSettings, readChatAttachment } from "@services/tauri";
import { subscribeWindowDragDrop } from "@services/dragDrop";
import { Markdown } from "@/features/messages/components/Markdown";
import { modelKey, modelRef } from "@/features/settings/registry";
import { useModeSettings } from "./ModeShell";
import { chatGroups, chatMarkdown, chatTitle, inlineAttachment, readAttachment, type Attachment } from "./chat";
import { useChat } from "./useChat";

const examples = ["Explain something surprising about the ocean", "Help me plan a relaxing weekend", "Give me three ideas for a short story"];

export function ChatView({ settings }: { settings: AppSettings }) {
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

  return <ModeAppShell className="chat-mode" showList={showList} footer={<Button variant="ghost" aria-pressed={trash} onClick={() => setTrash(!trash)}>{trash ? "Back to chats" : "Trash"}</Button>} sidebar={<>
      <PrimaryAction disabled={busy} onClick={() => { chat.setActive(null); clearDraft(); setNewWeb(false); setModel(defaultModel || modelKey(settings.agentPolicy?.defaultModel) || models[0]?.key || ""); setTrash(false); setShowList(false); }}>New chat</PrimaryAction>
      <input type="search" aria-label="Search chats" placeholder="Search chats" value={search} onChange={e => setSearch(e.target.value)} />
      <div className="chat-list">
        {Object.entries(chatGroups(chat.chats, search, trash)).map(([group, entries]) => entries.length > 0 && <section key={group} aria-label={group}>
          <h2>{group}</h2>
          {entries.map(entry => <ListRow className="chat-list-row" active={chat.active === entry.id} key={entry.id}>
            {rename === entry.id ? <form onSubmit={e => { e.preventDefault(); void mutate("rename", entry.id, { title: name }).then(success => { if (success) setRename(null); }); }}>
              <input autoFocus aria-label="Chat title" maxLength={200} value={name} onChange={e => setName(e.target.value)} /><Button variant="primary" type="submit" disabled={busy || !name.trim()}>Save</Button><Button type="button" onClick={() => setRename(null)}>Cancel</Button>
            </form> : <>
              <Button className="chat-list-title" aria-current={chat.active === entry.id ? "page" : undefined} disabled={busy || trash} onClick={() => { void run(async () => { await chat.open(entry.id); clearDraft(); setShowList(false); }); }}>{entry.running && <span aria-label="Running">● </span>}{chatTitle(entry)}</Button>
              <div className="chat-row-actions">
                {trash ? <><small>Restore within 30 days</small><Button disabled={busy || Date.now() / 1000 - (entry.chat.deletedAt ?? 0) > 30 * 86400} onClick={() => { void mutate("restore", entry.id); }}>Restore</Button></> : <>
                  <Button disabled={busy} aria-label={`${entry.chat.pinned ? "Unpin" : "Pin"} ${chatTitle(entry)}`} onClick={() => { void mutate("pin", entry.id, { pinned: !entry.chat.pinned }); }}>{entry.chat.pinned ? "Unpin" : "Pin"}</Button>
                  <Button disabled={busy} onClick={() => { setRename(entry.id); setName(chatTitle(entry)); }}>Rename</Button>
                  <Button disabled={busy || entry.running} onClick={() => { void mutate("trash", entry.id); }}>Delete</Button>
                </>}
              </div>
            </>}
          </ListRow>)}
        </section>)}
        {!chat.chats.length && <p className="chat-muted">Your conversations will appear here.</p>}
      </div>
    </>}>
    <main className="chat-main">
      <MainTopbar leftNode={<><Button className="chat-list-toggle" aria-expanded={showList} onClick={() => setShowList(!showList)}>☰ Chats</Button><strong>{active ? chatTitle(active) : "New chat"}</strong></>} actionsNode={active && <Button disabled={busy} onClick={() => { void run(async () => { await exportMarkdownFile(chatMarkdown(chatTitle(active), chat.messages), "chat.md", "Export chat as Markdown"); }); }}>Export as Markdown</Button>} />
      <div className="chat-scroll" role="log" aria-label="Conversation" aria-live="polite">
        <div className="chat-column">
          {!chat.messages.length && <EmptyState eyebrow="A little room to think" title="What’s on your mind?" subtitle="Ask a question, explore an idea, or bring something to read." suggestions={examples.map(example => <Card key={example}><Button variant="ghost" onClick={() => setDraft(example)}>{example}<span aria-hidden>↗</span></Button></Card>)} />}
          {chat.messages.map(message => <article className={`chat-message ${message.role}`} key={message.id}>
            <h2>{message.role === "user" ? "You" : "Assistant"}</h2>
            {message.images?.map((url, index) => <img key={index} src={url} alt={`Attachment ${index + 1}`} />)}
            <Markdown value={message.text} codeBlock showFilePath={false} />
            {message.id === lastUser?.id && <Button disabled={disabled} onClick={() => { setDraft(message.text); setAttachments(message.images?.map((url, i) => ({ name: `Image ${i + 1}`, url })) ?? []); setEditing(message.id); }}>Edit and resend</Button>}
          </article>)}
          {lastUser && <Button disabled={disabled} onClick={() => { void run(() => chat.send(lastUser.text, lastUser.images ?? [], model || null, lastUser.id)); }}>Regenerate last answer</Button>}
          {chat.running && <p role="status" className="chat-muted">Responding…</p>}
          <div ref={end} />
        </div>
      </div>
      <form className="chat-composer chat-column" onSubmit={e => { e.preventDefault(); send(); }} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); if (!disabled) addFiles(Array.from(e.dataTransfer.files)); }}>
        <div className="chat-model-controls"><label>Model <Select aria-label="Chat model" value={model} disabled={disabled} onChange={e => selectModel(e.target.value)}>
          {!models.length && <option value="">Enable a provider in Settings</option>}
          {models.map(m => <option value={m.key} key={m.key}>{m.label}</option>)}
        </Select></label>
        {settings.mcpServers?.some(c => c.id === "taucode-browser" && c.enabled) && <label><input type="checkbox" aria-label="Web" checked={active ? Boolean(active.chat.web) : newWeb} disabled={disabled} onChange={e => { if (active) { void mutate("web", active.id, { web: e.target.checked }); } else setNewWeb(e.target.checked); }} />Web</label>}
        <Button type="button" disabled={disabled || !model} onClick={() => { void run(async () => {
          const value = defaultModel === model ? "" : model;
          const updated = await updateAppSettings({ ...await getAppSettings(), chatDefaultModel: modelRef(value) });
          updateSettings?.(updated);
          setDefaultModel(value);
        }); }}>{defaultModel === model ? "Use policy default" : "Make chat default"}</Button></div>
        {editing && <div className="chat-editing">Editing the last message · creates a new branch <Button type="button" onClick={clearDraft}>Cancel</Button></div>}
        <TextArea aria-label="Message" placeholder="Message…" value={draft} disabled={busy} onChange={e => setDraft(e.target.value)} onPaste={e => { const files = Array.from(e.clipboardData.files); if (files.length) { e.preventDefault(); if (!disabled) addFiles(files); } }} onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }} />
        {!!attachments.length && <ul className="chat-attachments">{attachments.map((a, index) => <li key={index}>{a.url && <img src={a.url} alt="" />}{a.name}<Button type="button" disabled={busy} aria-label={`Remove ${a.name}`} onClick={() => setAttachments(current => current.filter((_, i) => i !== index))}>×</Button></li>)}</ul>}
        {chat.error && <p role="alert">{chat.error}</p>}
        <div className="chat-composer-actions"><label className="chat-attach">Attach<input type="file" aria-label="Attach files" multiple disabled={disabled} onChange={e => { addFiles(Array.from(e.target.files ?? [])); e.target.value = ""; }} /></label><span className="chat-muted">Shift + Enter for a new line</span>
          {chat.running ? <Button type="button" onClick={() => { void run(() => chatRequest("stop", { threadId: chat.active })); }}>Stop</Button> : <Button variant="primary" type="submit" disabled={busy || !model || (!draft.trim() && !attachments.length)}>Send ↑</Button>}
        </div>
      </form>
    </main>
  </ModeAppShell>;
}
