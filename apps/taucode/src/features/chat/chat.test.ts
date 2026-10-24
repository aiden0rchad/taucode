// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { chatGroups, chatMarkdown, readAttachment, type Chat } from "./chat";

const now = new Date(2026, 9, 23, 12);
const entry = (id: string, days: number, pinned = false, deletedAt: number | null = null): Chat => ({ id, model: "m", modelProvider: "p", createdAt: 0, updatedAt: now.getTime() / 1000 - days * 86400, chat: { title: id, pinned, deletedAt } });
describe("chat data", () => {
  it("groups, searches and pins without mixing Trash into chats", () => {
    const chats = [entry("today", 0), entry("week", 4), entry("older", 20), entry("pinned", 50, true), entry("deleted", 0, false, 100)];
    const groups = chatGroups(chats, "", false, now);
    expect(Object.fromEntries(Object.entries(groups).map(([key, list]) => [key, list.map(c => c.id)]))).toEqual({ Pinned: ["pinned"], Today: ["today"], "Previous 7 days": ["week"], Older: ["older"] });
    expect(chatGroups(chats, "WEEK", false, now)["Previous 7 days"]).toHaveLength(1);
    expect(chatGroups(chats, "", true, now).Trash.map(c => c.id)).toEqual(["deleted"]);
  });
  it("inlines text safely and rejects large or binary attachments", async () => {
    const file = new File(["hello"], "notes.md", { type: "text/plain" });
    Object.defineProperty(file, "text", { value: async () => "a\n```\nb" });
    expect((await readAttachment(file, true)).text).toBe("notes.md\n````\na\n```\nb\n````");
    await expect(readAttachment(new File([new Uint8Array(1024 * 1024 + 1)], "large.txt"), true)).rejects.toThrow("1 MB");
    await expect(readAttachment(new File(["binary"], "binary.zip"), true)).rejects.toThrow("text-like");
    await expect(readAttachment(new File(["image"], "image.png", { type: "image/png" }), false)).rejects.toThrow("does not support");
    expect((await readAttachment(new File(["image"], "image.png", { type: "image/png" }), true)).url).toMatch(/^data:image\/png;base64,/);
  });
  it("exports only local message data", () => {
    expect(chatMarkdown("Example", [{ id: "u", kind: "message", role: "user", text: "Question" }, { id: "a", kind: "message", role: "assistant", text: "Answer" }])).toBe("# Example\n\n## You\n\nQuestion\n\n## Assistant\n\nAnswer");
  });
});
