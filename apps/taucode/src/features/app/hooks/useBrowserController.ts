import { useCallback, useEffect, useState } from "react";
import { subscribeAppServerEvents } from "@services/events";
import { interruptTurn } from "@services/tauri";
import { matchesShortcut } from "@utils/shortcuts";
import { isMacPlatform } from "@utils/platformPaths";

type Turn = { workspace: string; thread: string; turn: string };
export function useBrowserController() {
  const [open, setOpen] = useState(() => localStorage.getItem("taucode.browser.open") === "true");
  const [width, setWidth] = useState(() => Math.min(900, Math.max(320, Number(localStorage.getItem("taucode.browser.width")) || 520)));
  const [calls, setCalls] = useState<Record<string, Turn>>({});
  const toggle = useCallback(() => setOpen(value => !value), []);
  useEffect(() => { localStorage.setItem("taucode.browser.open", String(open)); }, [open]);
  useEffect(() => { localStorage.setItem("taucode.browser.width", String(width)); }, [width]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (matchesShortcut(e, isMacPlatform() ? "cmd+shift+b" : "ctrl+shift+b")) { e.preventDefault(); e.stopPropagation(); toggle(); } };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [toggle]);
  useEffect(() => subscribeAppServerEvents(event => {
    const message = event.message;
    const params = (message.params ?? {}) as Record<string, unknown>;
    const item = params.item as Record<string, unknown> | undefined;
    const thread = String(params.threadId ?? "");
    if (message.method === "item/started" && item?.type === "mcpToolCall" && ["taucode-browser", "taucode_browser", "Browser"].includes(String(item.server))) {
      setCalls(previous => ({ ...previous, [`${event.workspace_id}:${item.id}`]: { workspace: event.workspace_id, thread, turn: String(params.turnId ?? "") } }));
    } else if (message.method === "item/completed" && item) {
      setCalls(previous => { const next = { ...previous }; delete next[`${event.workspace_id}:${item.id}`]; return next; });
    } else if (message.method === "turn/completed" || message.method === "thread/closed") {
      setCalls(previous => Object.fromEntries(Object.entries(previous).filter(([,v]) => v.workspace !== event.workspace_id || v.thread !== thread)));
    }
  }), []);
  const takeOver = useCallback(async () => {
    const turns = new Map(Object.values(calls).map(turn => [`${turn.workspace}:${turn.thread}:${turn.turn}`, turn]));
    await Promise.all([...turns.values()].map(turn => interruptTurn(turn.workspace, turn.thread, turn.turn)));
  }, [calls]);
  return { open, setOpen, width, setWidth, toggle, agentActive: Object.keys(calls).length > 0, takeOver };
}
