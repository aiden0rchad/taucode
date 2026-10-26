import { useFeatures } from "@settings/features";
import { useSidebarState } from "@/features/layout/hooks/useSidebarState";
import { useCallback, useEffect, useRef, useState } from "react";
import { browserRequest, subscribeBrowser } from "@services/tauri";
import { matchesShortcut } from "@utils/shortcuts";
import { isMacPlatform } from "@utils/platformPaths";
import { useBrowserController } from "./useBrowserController";
import { useResizablePanels } from "@/features/layout/hooks/useResizablePanels";
import { defaultBrowserSettings, normalizeBrowserUrl, type BrowserState } from "@/features/browser/browser";
import { panelReducer, readPanel, workspaceLayout, type PanelTab, type PanelState } from "@/features/panels/panelState";
import type { WorkspaceShellProps } from "@/features/panels/SidePanel";

export function useSidePanelState({ scope, tools = [], searchUrl = defaultBrowserSettings.searchUrl, onNavigate, reservedShortcuts = [] }: Pick<WorkspaceShellProps, "scope" | "tools" | "searchUrl" | "onNavigate" | "reservedShortcuts">) {
  const features = useFeatures();
  const [saved, setSaved] = useState(() => ({ scope, state: readPanel(scope, tools.some(t => t.id === "task")) }));
  if (saved.scope !== scope) setSaved({ scope, state: readPanel(scope, tools.some(t => t.id === "task")) });
  const tabs = saved.state.tabs.filter(tab => tab.kind === "browser" ? features.browser : tools.some(t => t.id === tab.kind));
  const state = { ...saved.state, tabs, active: tabs.some(t => t.id === saved.state.active) ? saved.state.active : null };
  const currentScope = useRef(scope); currentScope.current = scope;
  const update = useCallback((fn: (state: PanelState) => PanelState) => setSaved(previous => ({ ...previous, state: fn(previous.state) })), []);
  const browser = useBrowserController(false);
  const [expanded, setExpanded] = useState(false);
  const [windowWidth, setWindowWidth] = useState(window.innerWidth);
  const resize = useResizablePanels(windowWidth, expanded);
  const { sidebarWidth, sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const layout = workspaceLayout(windowWidth, sidebarWidth, sidebarCollapsed, expanded ? Math.min(windowWidth * .75, 1000) : resize.rightPanelWidth, state.open);
  const narrow = layout.sidebarOverlay;
  const collapseRef = useRef(setSidebarCollapsed); collapseRef.current = setSidebarCollapsed;
  useEffect(() => { if (narrow) collapseRef.current(true); }, [narrow]);
  useEffect(() => {
    const resizeWindow = () => setWindowWidth(resize.appRef.current?.getBoundingClientRect().width || window.innerWidth);
    const observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(resizeWindow);
    if (resize.appRef.current) observer?.observe(resize.appRef.current);
    resizeWindow();
    window.addEventListener("resize", resizeWindow);
    return () => { observer?.disconnect(); window.removeEventListener("resize", resizeWindow); };
  }, [resize.appRef]);
  const overlay = layout.panelOverlay;
  const panelRef = useRef<HTMLElement>(null);
  const mainRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!overlay || !state.open) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    const main = mainRef.current;
    main?.setAttribute("inert", "");
    panelRef.current?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") { event.preventDefault(); update(s => ({ ...s, open: false })); }
      if (event.key === "Tab" && panelRef.current?.contains(document.activeElement)) {
        const items = [...panelRef.current.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]')].filter(el => !el.closest('[hidden]') && el.getClientRects().length);
        const first = items[0], last = items[items.length - 1];
        if (event.shiftKey && (document.activeElement === first || document.activeElement === panelRef.current)) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }
    };
    window.addEventListener("keydown", key);
    return () => { main?.removeAttribute("inert"); window.removeEventListener("keydown", key); if (previousFocus?.isConnected) previousFocus.focus(); };
  }, [overlay, state.open, update]);
  const [address, setAddress] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [recent, setRecent] = useState<{ url: string; title: string; favicon?: string }[]>(() => {
    try { const items = JSON.parse(localStorage.getItem("taucode.browser.recent") ?? "[]"); return Array.isArray(items) ? items.filter(t => t && typeof t.url === "string" && /^https?:/.test(t.url) && typeof t.title === "string").slice(0, 4) : []; } catch { return []; }
  });
  const latest = useRef<BrowserState>({ tabs: [], active: "" });
  const pending = useRef<{ id: string; scope: string; previous: Set<string>; tab: PanelTab } | null>(null);
  const toolRef = useRef(tools); toolRef.current = tools;
  const history = useRef([scope]);
  const [historyIndex, setHistoryIndex] = useState(0);
  const historyPosition = useRef(0);
  useEffect(() => {
    if (history.current[historyPosition.current] !== scope) { history.current = [...history.current.slice(0, historyPosition.current + 1), scope]; historyPosition.current = history.current.length - 1; setHistoryIndex(historyPosition.current); }
  }, [scope]);
  useEffect(() => { try { localStorage.setItem(`taucode.panel.${saved.scope}`, JSON.stringify(saved.state)); } catch (e) { setError(`Could not save panel tabs: ${e}`); } }, [saved]);
  useEffect(() => {
    if (scope.startsWith("code:")) window.dispatchEvent(new CustomEvent("taucode:panel-visibility", { detail: state.open }));
  }, [scope, state.open]);
  useEffect(() => {
    if (!features.browser) return;
    let disposed = false; let stop: (() => void) | undefined;
    void subscribeBrowser(value => {
      if (disposed) return;
      const newPages = value.tabs.filter(page => !latest.current.tabs.some(old => old.id === page.id));
      const hadSnapshot = latest.current.active !== "";
      latest.current = value;
      const request = pending.current;
      const created = request && value.tabs.find(t => t.id === value.active && !request.previous.has(t.id));
      if (created && request && request.scope !== currentScope.current) {
        try {
          const restored = panelReducer(readPanel(request.scope), { type: "open", tab: { ...request.tab, target: created.id, url: request.tab.url || created.url, title: created.title || "New page" } });
          localStorage.setItem(`taucode.panel.${request.scope}`, JSON.stringify(restored));
        } catch (e) { setError(`Could not save browser tab: ${e}`); }
      }
      setSaved(previous => {
        const tabs = previous.state.tabs.map(tab => {
          if (tab.kind !== "browser") return tab;
          const page = value.tabs.find(t => t.id === tab.target) ?? (created && request?.scope === previous.scope && request.id === tab.id ? created : undefined);
          return page ? { ...tab, target: page.id, url: page.url, title: page.title || "New page" } : tab;
        });
        if (!request && hadSnapshot) {
          for (const page of newPages) if (!tabs.some(tab => tab.target === page.id)) tabs.push({ id: page.id, kind: "browser", title: page.title || "New page", url: page.url, target: page.id });
        }
        return JSON.stringify(tabs) === JSON.stringify(previous.state.tabs) ? previous : { ...previous, state: { ...previous.state, tabs } };
      });
      if (created) pending.current = null;
      const page = value.tabs.find(t => t.id === value.active);
      if (page && /^https?:/.test(page.url)) setRecent(previous => {
        if (previous[0]?.url === page.url && previous[0]?.title === page.title && previous[0]?.favicon === (page.favicon?.startsWith("data:image/") ? page.favicon : undefined)) return previous;
        const next = [{ url: page.url, title: page.title || page.url, favicon: page.favicon?.startsWith("data:image/") ? page.favicon : undefined }, ...previous.filter(t => t.url !== page.url)].slice(0, 4);
        try { localStorage.setItem("taucode.browser.recent", JSON.stringify(next)); } catch { /* Suggestions are optional when storage is full. */ }
        return next;
      });
    }).then(unsubscribe => { if (disposed) unsubscribe(); else stop = unsubscribe; }).catch(e => { if (!disposed) setError(String(e)); });
    return () => { disposed = true; stop?.(); };
  }, [features.browser]);
  const openTool = useCallback((id: string, preserveActive = false) => {
    const tool = toolRef.current.find(t => t.id === id);
    if (tool) update(s => {
      const next = panelReducer(s, { type: "open", tab: { id, kind: id, title: tool.label, ...(tool.target ? { target: tool.target } : {}) } });
      return preserveActive && s.tabs.some(t => t.id === id) ? { ...next, active: s.active, open: s.open } : next;
    });
  }, [update]);
  const closeTool = useCallback((id: string) => update(s => panelReducer(s, { type: "close", id })), [update]);
  const active = state.tabs.find(t => t.id === state.active);
  const activeId = active?.id, activeKind = active?.kind, activeTarget = active?.target;
  useEffect(() => { if (state.open && activeKind && activeKind !== "browser") toolRef.current.find(t => t.id === activeKind)?.onOpen?.({ id: activeId!, kind: activeKind, title: "", target: activeTarget }); }, [scope, state.open, activeId, activeKind, activeTarget]);
  const openPage = useCallback(async (input = "about:blank", existing?: PanelTab) => {
    if (!features.browser) return;
    if (pending.current) return;
    setBusy(true); setError("");
    try {
      const url = normalizeBrowserUrl(input, searchUrl);
      await browserRequest({ action: "attach" });
      if (currentScope.current !== scope) return;
      if (existing?.target && latest.current.tabs.some(t => t.id === existing.target)) { await browserRequest({ action: "activate", id: existing.target }); return; }
      const tab = existing ?? { id: crypto.randomUUID(), kind: "browser", title: "New page", url };
      pending.current = { id: tab.id, scope, previous: new Set(latest.current.tabs.map(t => t.id)), tab };
      update(s => panelReducer(s, { type: "open", tab }));
      await browserRequest({ action: "new" });
      if (url !== "about:blank") await browserRequest({ action: "navigate", url });
      setAddress("");
    } catch (e) { pending.current = null; setError(String(e)); }
    finally { setBusy(false); }
  }, [scope, searchUrl, update, features.browser]);
  const pageRef = useRef(openPage); pageRef.current = openPage;
  const activeRef = useRef(active); activeRef.current = active;
  useEffect(() => { const tab = activeRef.current; if (state.open && tab?.kind === "browser") void pageRef.current(tab.url, tab); }, [scope, state.open, activeId, activeKind]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.defaultPrevented || reservedShortcuts.some(shortcut => matchesShortcut(e, shortcut))) return;
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "b") { e.preventDefault(); void pageRef.current(); }
      if (matchesShortcut(e, isMacPlatform() ? "cmd+p" : "ctrl+p") && toolRef.current.some(t => t.id === "files")) { e.preventDefault(); openTool("files"); }
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "g") { e.preventDefault(); openTool("git"); }
    };
    const toggle = (event: Event) => update(s => ({ ...s, open: typeof (event as CustomEvent).detail === "boolean" ? (event as CustomEvent<boolean>).detail : !s.open }));
    window.addEventListener("taucode:panel-toggle", toggle);
    window.addEventListener("keydown", key); return () => { window.removeEventListener("keydown", key); window.removeEventListener("taucode:panel-toggle", toggle); };
  }, [openTool, update, reservedShortcuts]);
  const close = async (tab: PanelTab) => {
    try {
      if (tab.kind === "browser" && tab.target && latest.current.tabs.some(t => t.id === tab.target)) await browserRequest({ action: "close", id: tab.target });
      toolRef.current.find(t => t.id === tab.kind)?.onClose?.();
      update(s => panelReducer(s, { type: "close", id: tab.id }));
    } catch (e) { setError(String(e)); }
  };
  const navigate = (index: number) => { historyPosition.current = index; setHistoryIndex(index); onNavigate?.(history.current[index]); };
  return { state, update, browser, resize, layout, overlay, panelRef, mainRef, expanded, setExpanded, address, setAddress, error, busy, recent, pending, history, historyIndex, openTool, closeTool, active, openPage, close, navigate };
}
