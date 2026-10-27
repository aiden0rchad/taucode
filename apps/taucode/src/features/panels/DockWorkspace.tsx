import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { flushSync } from "react-dom";
import { ArrowLeft, ArrowRight, Folder, GitBranch, Globe, Maximize2, Minimize2, PanelRight, Plus, Terminal, MessageSquare, X } from "lucide-react";
import { IconButton } from "@/features/design-system/components/IconButton";
import { DiagnosticBoundary } from "@/features/diagnostics/DiagnosticBoundary";
import { RowMenu } from "@/features/chat/RowMenu";
import { BrowserPane, type BrowserContext } from "@/features/browser/BrowserPane";
import { SidebarCollapseButton } from "@/features/layout/components/SidebarResizeHandle";
import { WindowCaptionControls } from "@/features/layout/components/WindowCaptionControls";
import { usePanelResize } from "@/features/layout/hooks/usePanelResize";
import { useFeatures } from "@settings/features";
import type { useSidePanelState } from "@app/hooks/useSidePanelState";
import type { WorkspaceShellProps } from "./SidePanel";
import { dockGeometry, dockPanes, hideDockPane, initialDock, moveDockPane, panelReducer, removeDockPane, resizeDockSplit, showDockPane, type DockBox, type DockLayout, type DockSide } from "./panelState";

const quickTools = [{ id: "terminal", label: "Terminal", icon: Terminal }, { id: "git", label: "Changes", icon: GitBranch }, { id: "browser", label: "Browser", icon: Globe }, { id: "files", label: "Files", icon: Folder }];
const full: DockBox = { x: 0, y: 0, w: 100, h: 100 };
const position = (box: DockBox): CSSProperties => ({ "--dock-box-x": box.x / 100, "--dock-box-width": box.w / 100, top: `${box.y}%`, height: `${box.h}%` } as CSSProperties);

type Controller = ReturnType<typeof useSidePanelState>;
export function DockWorkspace({ children, scope, title, tools = [], searchUrl, onInterrupt, titleActions, actions = [], onNavigate, sessionRail, onBrowserContext, controller }: WorkspaceShellProps & { controller: Controller }) {
  const { state, update, resize, layout, mainRef, openTool, openPage, close, browser, busy, error, history, historyIndex, navigate } = controller;
  const features = useFeatures();
  const saved = state.dock ?? initialDock(state, Boolean(sessionRail));
  const setDock = (fn: (layout: DockLayout) => DockLayout) => update(state => ({ ...state, open: true, dock: fn(state.dock ?? initialDock(state, Boolean(sessionRail))) }));
  const available = new Set(["conversation", ...(sessionRail ? ["session"] : []), ...tools.map(tool => tool.id), ...(features.browser ? ["browser"] : [])]);
  let tree = state.open ? saved.tree : { type: "pane" as const, id: "conversation" };
  for (const id of dockPanes(tree)) if (!available.has(id)) tree = removeDockPane(tree, id) ?? { type: "pane", id: "conversation" };
  const geometry = dockGeometry(tree), visible = new Set(Object.keys(geometry.panes));
  const expanded = saved.expanded && visible.has(saved.expanded) ? saved.expanded : null;
  const stage = useRef<HTMLDivElement>(null);
  const [contentWidth, setContentWidth] = useState<number | null>(null);
  useLayoutEffect(() => {
    const element = stage.current; if (!element) return;
    const measure = () => setContentWidth(element.getBoundingClientRect().width);
    const observer = new ResizeObserver(measure); observer.observe(element); measure();
    return () => observer.disconnect();
  }, [layout.conversationWidth]);
  const compact = (contentWidth ?? layout.conversationWidth) < 760;
  const focused = expanded ?? (saved.focused && visible.has(saved.focused) ? saved.focused : "conversation");
  const covered = Boolean(expanded || compact && focused !== "conversation");
  const conversation = compact ? full : geometry.panes.conversation;
  const panes = useRef(new Map<string, HTMLElement>());
  const mounted = useRef({ scope, ids: new Set<string>() });
  if (mounted.current.scope !== scope) mounted.current = { scope, ids: new Set() };
  for (const id of visible) mounted.current.ids.add(id);
  const { startResize } = usePanelResize();
  const toggle = (id: string) => {
    if (visible.has(id) && (compact ? focused === id : !expanded || expanded === id)) { setDock(layout => hideDockPane(layout, id)); return; }
    if (id === "browser") {
      const tab = state.tabs.find(tab => tab.kind === "browser" && tab.id === state.browserActive) ?? state.tabs.find(tab => tab.kind === "browser");
      if (tab) update(state => panelReducer(state, { type: "select", id: tab.id }));
      else void openPage();
    } else if (id === "session") setDock(layout => ({ ...showDockPane(layout, id), expanded: null }));
    else openTool(id);
  };
  useEffect(() => {
    if (!covered) return;
    const fill = expanded ?? focused;
    const root = mainRef.current;
    const targets = [...(root?.querySelectorAll<HTMLElement>(":scope > :is(.app, .chat-layout) > :is(.main, .chat-main, .tablet-main), :scope > .app > .compact-shell") ?? [])];
    if (!targets.length && root) targets.push(root);
    const previous = document.activeElement as HTMLElement | null;
    const restore = previous && (targets.some(target => target.contains(previous)) || [...panes.current].some(([id, element]) => id !== fill && element.contains(previous)));
    const inert = targets.map(target => target.hasAttribute("inert"));
    targets.forEach(target => target.setAttribute("inert", ""));
    if (restore) panes.current.get(fill)?.focus();
    return () => {
      targets.forEach((target, index) => { if (!inert[index]) target.removeAttribute("inert"); });
      if (restore && previous.isConnected && panes.current.get(fill)?.contains(document.activeElement)) previous.focus();
    };
  }, [covered, expanded, focused, mainRef, scope]);
  const menu = (id: string) => [
    ...(["left", "right", "top", "bottom"] as DockSide[]).map(side => ({ label: `Move ${side}`, onClick: () => setDock(layout => moveDockPane(layout, id, side)) })),
    { label: "Fill", onClick: () => setDock(layout => ({ ...layout, expanded: id })) },
    { label: "Tile", onClick: () => setDock(layout => ({ ...layout, expanded: null, ...(compact ? { focused: "conversation" } : {}) })) },
  ];
  const addBrowserContext = onBrowserContext ? (context: BrowserContext) => {
    if (expanded === "browser" || compact && focused === "browser") flushSync(() => setDock(layout => ({ ...layout, expanded: null, focused: "conversation" })));
    onBrowserContext(context);
  } : undefined;
  const browserTabs = state.tabs.filter(tab => tab.kind === "browser");
  const content = [
    ...(sessionRail ? [{ id: "session", label: "Session rail", content: sessionRail }] : []),
    ...tools,
    ...(features.browser && mounted.current.ids.has("browser") ? [{ id: "browser", label: "Browser", content: <BrowserPane embedded onBrowserContext={addBrowserContext} searchUrl={searchUrl} agentActive={browser.agentActive} onTakeOver={browser.agentActive ? browser.takeOver : onInterrupt} onClose={() => setDock(layout => hideDockPane(layout, "browser"))} /> }] : []),
  ];
  const measurement = <div key="dock-measurement" className="dock-stage" ref={stage} aria-hidden="true" />;
  return <div className={`workspace-shell dock-workspace${compact ? " is-compact" : ""}${layout.sidebarOverlay ? " sidebar-overlay" : ""}${covered ? " panel-expanded" : ""}`} ref={resize.appRef} style={{
    "--right-panel-width": "0px", "--sidebar-width": `${layout.sidebarWidth}px`, "--layout-sidebar-column": `${layout.sidebarColumn}px`,
    "--dock-conversation-x": (conversation?.x ?? 0) / 100, "--dock-conversation-y": `${conversation?.y ?? 0}%`, "--dock-conversation-width": (conversation?.w ?? 100) / 100, "--dock-conversation-height": `${conversation?.h ?? 100}%`,
  } as CSSProperties}>
    <header id="titlebar" className="workspace-titlebar" data-tauri-drag-region>
      <div className="titlebar-navigation" data-tauri-drag-region="false"><IconButton className="ghost" aria-label="Back" disabled={!onNavigate || historyIndex === 0} onClick={() => navigate(historyIndex - 1)}><ArrowLeft aria-hidden /></IconButton><IconButton className="ghost" aria-label="Forward" disabled={!onNavigate || historyIndex >= history.current.length - 1} onClick={() => navigate(historyIndex + 1)}><ArrowRight aria-hidden /></IconButton><SidebarCollapseButton /></div>
      <div className="titlebar-title" data-tauri-drag-region><span title={title}>{title}</span></div>
      {titleActions && <div className="titlebar-actions">{titleActions}</div>}
      {actions.length > 0 && <RowMenu label="thread" size="md" actions={actions}><span /></RowMenu>}
      <div className="dock-toolbar" role="toolbar" aria-label="Workspace views" data-tauri-drag-region="false">
        {compact && <IconButton className="ghost" aria-label="Show conversation" title="Conversation" aria-pressed={focused === "conversation"} onClick={() => setDock(layout => ({ ...layout, expanded: null, focused: "conversation" }))}><MessageSquare aria-hidden /></IconButton>}
        {sessionRail && <IconButton className="ghost" aria-label="Toggle session rail" title="Session rail" aria-pressed={visible.has("session") && (!compact || focused === "session")} onClick={() => toggle("session")}><PanelRight aria-hidden /></IconButton>}
        {quickTools.filter(tool => available.has(tool.id)).map(tool => <IconButton className="ghost" key={tool.id} aria-label={`Toggle ${tool.label}`} title={tool.label} aria-pressed={visible.has(tool.id) && (!compact || focused === tool.id)} disabled={tool.id === "browser" && busy} onClick={() => toggle(tool.id)}><tool.icon aria-hidden /></IconButton>)}
        <RowMenu label="View options" size="md" portal actions={[
          ...tools.filter(tool => !quickTools.some(quick => quick.id === tool.id)).map(tool => ({ label: tool.label, onClick: () => toggle(tool.id) })),
          { label: "Tile panes", onClick: () => setDock(layout => ({ ...layout, expanded: null, ...(compact ? { focused: "conversation" } : {}) })) },
        ]}><span /></RowMenu>
      </div>
      <WindowCaptionControls />
    </header>
    <div className="workspace-body dock-body">
      <div className="workspace-main" ref={mainRef}>{children}</div>
      {/* Moving existing panes past this keyed anchor refreshes WebKit's native AX children on expansion without remounting live content. */}
        {[...(expanded ? [measurement] : []), ...content.filter(pane => mounted.current.ids.has(pane.id)).map(pane => {
          const shown = visible.has(pane.id) && (compact ? focused === pane.id : !expanded || expanded === pane.id);
          const box = compact || expanded === pane.id ? full : geometry.panes[pane.id] ?? full;
          return <section className={`dock-pane${expanded === pane.id ? " is-expanded" : ""}${pane.id === "session" ? " dock-session-rail" : ""}`} key={pane.id} data-dock-pane={pane.id} aria-label={pane.label} hidden={!shown} tabIndex={-1} ref={element => { if (element) panes.current.set(pane.id, element); else panes.current.delete(pane.id); }} style={position(box)}>
            <header className="dock-pane-header" draggable onDragStart={event => event.dataTransfer.setData("application/x-taucode-pane", pane.id)} onDragOver={event => event.preventDefault()} onDrop={event => {
              event.preventDefault(); const source = event.dataTransfer.getData("application/x-taucode-pane");
              if (visible.has(source) && source !== "conversation") setDock(layout => moveDockPane(layout, source, "bottom", pane.id));
            }}><strong>{pane.label}</strong>
              <RowMenu label={`${pane.label} pane`} portal actions={menu(pane.id)}><IconButton className="ghost" aria-label={`${expanded === pane.id ? "Restore" : "Expand"} ${pane.label}`} onClick={() => setDock(layout => ({ ...layout, expanded: layout.expanded === pane.id ? null : pane.id }))}>{expanded === pane.id ? <Minimize2 aria-hidden /> : <Maximize2 aria-hidden />}</IconButton></RowMenu>
              <IconButton className="ghost" aria-label={`Close ${pane.label} pane`} onClick={() => setDock(layout => hideDockPane(layout, pane.id))}><X aria-hidden /></IconButton>
            </header>
            {pane.id === "browser" && <div className="dock-browser-tabs" role="tablist" aria-label="Browser pages">{browserTabs.map(tab => <div className="dock-browser-tab" key={tab.id}><button role="tab" aria-selected={(state.browserActive ?? browserTabs[0]?.id) === tab.id} onClick={() => update(state => panelReducer(state, { type: "select", id: tab.id }))}>{tab.title}</button><IconButton aria-label={`Close ${tab.title}`} onClick={() => { void close(tab); }}><X aria-hidden /></IconButton></div>)}<IconButton aria-label="New browser tab" disabled={busy} onClick={() => { void openPage(); }}><Plus aria-hidden /></IconButton></div>}
            <div className="dock-pane-content"><DiagnosticBoundary panel={pane.id}>{pane.content}</DiagnosticBoundary></div>
          </section>;
        }), ...(!expanded ? [measurement] : [])]}
        {!expanded && !compact && geometry.splits.map(({ node, box }, index) => {
          const horizontal = node.axis === "horizontal";
          const style: CSSProperties = horizontal
            ? { ...position({ ...box, x: box.x + box.w * node.ratio }), width: 8, transform: "translateX(-4px)" }
            : { ...position({ ...box, y: box.y + box.h * node.ratio }), height: 8, transform: "translateY(-4px)" };
          return <div key={node.id} className={`dock-divider ${horizontal ? "is-horizontal" : "is-vertical"}`} role="separator" aria-label={`Resize dock split ${index + 1}`} aria-orientation={horizontal ? "vertical" : "horizontal"} aria-valuemin={15} aria-valuemax={85} aria-valuenow={Math.round(node.ratio * 100)} tabIndex={0} style={style} onKeyDown={event => {
            if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
            event.preventDefault(); const ratio = event.key === "Home" ? .15 : event.key === "End" ? .85 : node.ratio + (["ArrowLeft", "ArrowUp"].includes(event.key) ? -.05 : .05);
            setDock(layout => ({ ...layout, tree: resizeDockSplit(layout.tree, node.id, ratio) }));
          }} onPointerDown={event => {
            const bounds = stage.current?.getBoundingClientRect(); if (!bounds || !bounds.width || !bounds.height) return;
            startResize(event, horizontal ? "col-resize" : "row-resize", event => {
              const ratio = horizontal ? ((event.clientX - bounds.left) / bounds.width * 100 - box.x) / box.w : ((event.clientY - bounds.top) / bounds.height * 100 - box.y) / box.h;
              setDock(layout => ({ ...layout, tree: resizeDockSplit(layout.tree, node.id, ratio) }));
            });
          }} />;
        })}
      {error && <p className="dock-error" role="alert">{error}</p>}
    </div>
  </div>;
}
