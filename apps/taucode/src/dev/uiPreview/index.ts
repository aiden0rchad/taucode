import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { WorkspaceInfo } from "@/types";
import type { SettingsSection } from "@app/hooks/useSettingsModalState";
import { createPreviewHandler } from "./handler";

let preview: ReturnType<typeof createPreviewHandler>;
const selected = new Set<string>();

export function installUiPreview(search = window.location.search) {
  selected.clear();
  preview = createPreviewHandler((workspace_id, method, params) => {
    void emit("app-server-event", { workspace_id, message: { method, params } });
  }, search);
  mockWindows("main");
  // Tauri's built-in event mock expects `id` on unlisten, while the API sends
  // `eventId`; it also omits event.id (needed by once). Keep the event bridge here.
  const listeners = new Map<number, { event: string; handler: number }>();
  let nextEventId = 0;
  mockIPC((command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    if (command === "plugin:event|listen") {
      const id = ++nextEventId;
      listeners.set(id, { event: String(args.event), handler: Number(args.handler) });
      return id;
    }
    if (command === "plugin:event|unlisten") { listeners.delete(Number(args.eventId)); return null; }
    if (command === "plugin:event|emit" || command === "plugin:event|emit_to") {
      const internals = (window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, data: unknown) => void } }).__TAURI_INTERNALS__;
      for (const [id, listener] of listeners) {
        if (listener.event === args.event) internals.runCallback(listener.handler, { event: args.event, id, payload: args.payload });
      }
      return null;
    }
    return preview.handler(command, payload);
  });
  const tauri = window as unknown as {
    __TAURI_INTERNALS__: { unregisterCallback: (id: number) => void };
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: (event: string, id: number) => void };
  };
  tauri.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener = (_event, id) => {
    const listener = listeners.get(id);
    if (listener) tauri.__TAURI_INTERNALS__.unregisterCallback(listener.handler);
    listeners.delete(id);
  };
}

// Called only from mode-guarded dynamic imports, once the real views are ready.
export function selectConversation(kind: "chat" | "task", select: (id: string) => void) {
  if (!preview || selected.has(kind)) return;
  selected.add(kind);
  const entries = kind === "chat" ? preview.state.chats : preview.state.work.tasks;
  const requested = new URLSearchParams(window.location.search).get(kind);
  const entry = entries.find(e => e.id === requested) ?? entries[0];
  if (entry) select(entry.id);
}

export function selectCode(
  workspaces: WorkspaceInfo[],
  select: (workspaceId: string, threadId: string) => void,
  openSettings: (section?: SettingsSection) => void,
) {
  if (!preview || selected.has("code") || !workspaces.length) return;
  selected.add("code");
  const query = new URLSearchParams(window.location.search);
  const thread = preview.state.threads.find(t => t.id === query.get("thread")) ?? preview.state.threads[0];
  if (thread && workspaces.some(w => w.id === thread.workspaceId)) select(thread.workspaceId, thread.id);
  const section = query.get("settings");
  const sections: SettingsSection[] = ["projects", "display", "about", "dictation", "shortcuts", "open-apps", "git", "codex", "features"];
  if (section) openSettings(sections.find(s => s === section) ?? "display");
}
