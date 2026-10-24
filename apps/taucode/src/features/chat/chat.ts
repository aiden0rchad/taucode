import type { ConversationItem } from "@/types";

export type Chat = {
  id: string; model: string; modelProvider: string; updatedAt: number; createdAt: number;
  preview?: string; running?: boolean;
  chat: { title: string | null; pinned: boolean; deletedAt: number | null; web?: boolean };
};
export type ChatMessage = Extract<ConversationItem, { kind: "message" }>;
export type ChatThread = Record<string, unknown> & { id: string; model: string; modelProvider: string };
export type ChatResponse = { result: { thread: ChatThread } };
export type Attachment = { name: string; text?: string; url?: string };
export const chatTitle = (chat: Chat) => chat.chat.title || chat.preview || "New chat";

export function chatGroups(chats: Chat[], search: string, trash: boolean, now = new Date()) {
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000;
  const groups: Record<string, Chat[]> = trash ? { Trash: [] } : { Pinned: [], Today: [], "Previous 7 days": [], Older: [] };
  for (const chat of [...chats].sort((a, b) => b.updatedAt - a.updatedAt)) {
    if (Boolean(chat.chat.deletedAt) !== trash || !chatTitle(chat).toLowerCase().includes(search.toLowerCase())) continue;
    const group = trash ? "Trash" : chat.chat.pinned ? "Pinned" : chat.updatedAt >= today ? "Today" : chat.updatedAt >= today - 7 * 86400 ? "Previous 7 days" : "Older";
    groups[group].push(chat);
  }
  return groups;
}

export async function readAttachment(file: File, supportsImages: boolean): Promise<Attachment> {
  if (/^image\/(png|jpeg|gif|webp)$/.test(file.type)) {
    if (!supportsImages) throw new Error("This model does not support images.");
    if (file.size > 20 * 1024 * 1024) throw new Error("Images must be 20 MB or smaller.");
    const url = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error(`Cannot read ${file.name}`));
      reader.readAsDataURL(file);
    });
    return { name: file.name, url };
  }
  if (file.size > 1024 * 1024) throw new Error("Text attachments must be 1 MB or smaller.");
  if (!file.type.startsWith("text/") && !/\.(txt|md|csv|tsv|json|jsonl|xml|yaml|yml|toml|log|js|jsx|ts|tsx|py|rs|css|html|sh|sql)$/i.test(file.name)) throw new Error("Attach an image or a text-like file.");
  const text = await file.text();
  if (text.includes("\0")) throw new Error("Binary files are not supported.");
  return { name: file.name, text: inlineAttachment(file.name, text) };
}

export function inlineAttachment(name: string, text: string) {
  let fenceLength = 3;
  for (const match of text.matchAll(/`+/g)) fenceLength = Math.max(fenceLength, match[0].length + 1);
  const fence = "`".repeat(fenceLength);
  return `${name.replace(/[\r\n]/g, " ")}\n${fence}\n${text}\n${fence}`;
}

export function chatMarkdown(title: string, messages: ChatMessage[]) {
  return `# ${title}\n\n` + messages.map(m => `## ${m.role === "user" ? "You" : "Assistant"}\n\n${m.text}${m.images?.map(url => `\n\n![Attachment](${url})`).join("") ?? ""}`).join("\n\n");
}
