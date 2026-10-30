import { IconButton } from "@/features/design-system/components/IconButton";
import { diagnosticContext, record } from "@/features/diagnostics/client";
import { DiagnosticBoundary } from "@/features/diagnostics/DiagnosticBoundary";
import { useFeatures } from "@settings/features";
import { Squircle } from "@/features/design-system/components/squircle/Squircle";
import { useSidePanelState } from "@app/hooks/useSidePanelState";
import { createContext, useContext, useEffect, useRef, type CSSProperties, type ReactNode } from "react";
import { ArrowLeft, ArrowRight, Folder, Globe, GitBranch, Terminal, Users, ScrollText, MessageSquare, ListTodo, Plus, X, PanelRight, Maximize2, Minimize2 } from "lucide-react";
import { WindowCaptionControls } from "@/features/layout/components/WindowCaptionControls";
import { SidebarCollapseButton } from "@/features/layout/components/SidebarResizeHandle";
import { RowMenu } from "@/features/chat/RowMenu";
import { BrowserPane, type BrowserContext } from "@/features/browser/BrowserPane";
import { defaultBrowserSettings } from "@/features/browser/browser";
import { MIN_RIGHT_PANEL_WIDTH, panelReducer, type PanelTab } from "./panelState";
import { DockWorkspace } from "./DockWorkspace";
import "./side-panel.css";

export type PanelTool = { id: string; label: string; shortcut?: string; content: ReactNode; target?: string; onOpen?: (tab: PanelTab) => void; onClose?: () => void };
const icons = { git: GitBranch, files: Folder, terminal: Terminal, agents: Users, prompts: ScrollText, sidechat: MessageSquare, task: ListTodo, previews: Globe, browser: Globe };
const PanelContext = createContext<{ scope: string; openTool: (id: string, preserveActive?: boolean) => void; closeTool: (id: string) => void; openPage: (url?: string) => Promise<void> } | null>(null);
export const useSidePanel = () => useContext(PanelContext);
export function PanelToolRequest({ id, requestKey }: { id: string; requestKey: unknown }) {
  const { scope, openTool, closeTool } = useSidePanel() ?? {};
  const previous = useRef({ requestKey, scope });
  useEffect(() => {
    if (requestKey) openTool?.(id, previous.current.scope !== scope);
    else if (previous.current.requestKey && previous.current.scope === scope) closeTool?.(id);
    previous.current = { requestKey, scope };
  }, [requestKey, id, scope, openTool, closeTool]);
  return null;
}
export type WorkspaceShellProps = { dock?: boolean; sessionRail?: ReactNode; onBrowserContext?: (context: BrowserContext) => void; children: ReactNode; scope: string; title: string; tools?: PanelTool[]; searchUrl?: string; onInterrupt: () => void | Promise<void>; titleActions?: ReactNode; actions?: { label: string; disabled?: boolean; onClick: () => void }[]; onNavigate?: (scope: string) => void; reservedShortcuts?: string[] };
export function WorkspaceShell({ children, scope, title, tools = [], searchUrl = defaultBrowserSettings.searchUrl, onInterrupt, titleActions, actions = [], onNavigate, reservedShortcuts = [], dock = false, sessionRail, onBrowserContext }: WorkspaceShellProps) {
  const features = useFeatures();
  tools = tools.filter(t => ({ git: features.git, agents: features.subagents, terminal: features.terminal, sidechat: features.sideChats } as Record<string, boolean>)[t.id] ?? true);
  const controller = useSidePanelState({ scope, tools, searchUrl, onNavigate, reservedShortcuts, dock, sessionRail });
  const { state, update, browser, resize, layout, overlay, panelRef, mainRef, expanded, setExpanded, address, setAddress, error, busy, recent, pending, history, historyIndex, openTool, closeTool, active, openPage, close, navigate } = controller;
  useEffect(() => {if (features.diagnostics) {const panel=state.open ? active?.kind ?? "conversation" : "conversation";diagnosticContext({panel});record({kind:"ux",area:"ui",code:"ui.panel_open",message:"Panel selection changed",context:{panel}});}},[features.diagnostics,state.open,active?.kind]);
  if (dock) return <PanelContext.Provider value={{ scope, openTool, closeTool, openPage }}><DockWorkspace {...{ children, scope, title, tools, searchUrl, onInterrupt, titleActions, actions, onNavigate, sessionRail, onBrowserContext }} controller={controller} /></PanelContext.Provider>;
  return <PanelContext.Provider value={{ scope, openTool, closeTool, openPage }}><div className={`workspace-shell${layout.sidebarOverlay ? " sidebar-overlay" : ""}${expanded && state.open ? " panel-expanded" : ""}`} ref={resize.appRef} style={{ "--right-panel-width": `${resize.rightPanelWidth}px`, "--sidebar-width": `${layout.sidebarWidth}px`, "--layout-sidebar-column": `${layout.sidebarColumn}px` } as CSSProperties}>
    <header id="titlebar" className="workspace-titlebar" data-tauri-drag-region>
      <div className="titlebar-navigation" data-tauri-drag-region="false"><IconButton className="ghost" aria-label="Back" disabled={!onNavigate || historyIndex === 0} onClick={() => navigate(historyIndex - 1)}><ArrowLeft aria-hidden /></IconButton><IconButton className="ghost" aria-label="Forward" disabled={!onNavigate || historyIndex >= history.current.length - 1} onClick={() => navigate(historyIndex + 1)}><ArrowRight aria-hidden /></IconButton><SidebarCollapseButton /></div>
      <div className="titlebar-title" data-tauri-drag-region><span data-tauri-drag-region title={title}>{title}</span></div>
      {titleActions && <div className="titlebar-actions">{titleActions}</div>}
      {actions.length > 0 && <RowMenu label="thread" size="md" actions={actions}><span /></RowMenu>}
      <IconButton className="ghost" data-tauri-drag-region="false" aria-label="Toggle side panel" aria-expanded={state.open} onClick={() => update(s => ({ ...s, open: !s.open }))}><PanelRight aria-hidden /></IconButton>
      <WindowCaptionControls />
    </header>
    <div className="workspace-body"><div className="workspace-main" ref={mainRef}>{children}</div>
    {overlay && state.open && <button className="side-panel-scrim" aria-label="Dismiss side panel" onClick={() => update(s => ({ ...s, open: false }))} />}
    {<aside ref={panelRef} tabIndex={-1} role={overlay ? "dialog" : undefined} aria-modal={overlay && state.open ? true : undefined} hidden={!state.open} className={`tabbed-side-panel${overlay ? " is-overlay" : ""}${expanded ? " is-expanded" : ""}`} style={{ width: expanded ? undefined : "var(--right-panel-width)" }} aria-label="Side panel">
      {!expanded && <div className="side-panel-resizer" role="separator" aria-label="Resize side panel" aria-orientation="vertical" aria-valuemin={MIN_RIGHT_PANEL_WIDTH} aria-valuemax={resize.maxRightPanelWidth} aria-valuenow={resize.rightPanelWidth} tabIndex={0} onPointerDown={resize.onRightPanelResizeStart} onKeyDown={e => { if (["ArrowLeft", "ArrowRight"].includes(e.key)) { e.preventDefault(); resize.setRightPanelWidth(resize.rightPanelWidth + (e.key === "ArrowLeft" ? 20 : -20)); } }} />}
      <div className="side-panel-tabbar"><div className="side-panel-tabs" role="tablist" aria-label="Side panel tabs">
        {state.tabs.map((tab, index) => { const Icon = icons[tab.kind as keyof typeof icons] ?? Folder; return <div className="side-panel-tab" key={tab.id} draggable onDragStart={e => e.dataTransfer.setData("text/plain", tab.id)} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); update(s => panelReducer(s, { type: "reorder", id: e.dataTransfer.getData("text/plain"), before: tab.id })); }}>
          <button role="tab" aria-selected={tab.id === state.active} tabIndex={tab.id === state.active ? 0 : -1} onClick={() => update(s => panelReducer(s, { type: "select", id: tab.id }))} onKeyDown={e => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
            e.preventDefault(); const next = e.key === "Home" ? 0 : e.key === "End" ? state.tabs.length - 1 : (index + (e.key === "ArrowLeft" ? -1 : 1) + state.tabs.length) % state.tabs.length;
            if (e.altKey) update(s => panelReducer(s, { type: "reorder", id: tab.id, before: state.tabs[next].id, after: e.key === "ArrowRight" || e.key === "End" }));
            else { update(s => panelReducer(s, { type: "select", id: state.tabs[next].id })); e.currentTarget.closest('[role="tablist"]')?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus(); }
          }}><Icon aria-hidden /><span>{tab.title}</span></button><IconButton disabled={busy && pending.current?.id === tab.id} aria-label={`Close ${tab.title}`} onClick={() => { void close(tab); }}><X aria-hidden /></IconButton>
        </div>; })}
      </div><IconButton className="ghost" aria-label="New panel tab" onClick={() => update(s => panelReducer(s, { type: "select", id: null }))}><Plus aria-hidden /></IconButton><IconButton className="ghost" aria-label={expanded ? "Contract side panel" : "Expand side panel"} onClick={() => setExpanded(!expanded)}>{expanded ? <Minimize2 aria-hidden /> : <Maximize2 aria-hidden />}</IconButton></div>
      {error && <p role="alert">{error}</p>}
      <div className="side-panel-content" role="tabpanel" aria-label={active?.title || "New tab"}>
        {!active ? <div className="side-panel-new">{features.browser && <form onSubmit={e => { e.preventDefault(); void openPage(address); }}><input aria-label="URL or search" placeholder="Search or enter a URL" value={address} onChange={e => setAddress(e.target.value)} disabled={busy} /></form>}<h2>Tools</h2><div className="side-panel-tools">{tools.map(tool => { const Icon = icons[tool.id as keyof typeof icons] ?? Folder; return <Squircle as="button" type="button" key={tool.id} onClick={() => openTool(tool.id)}><Icon aria-hidden /><span>{tool.label}</span><kbd>{tool.shortcut}</kbd></Squircle>; })}{features.browser && <Squircle as="button" type="button" disabled={busy} onClick={() => { void openPage(); }}><Globe aria-hidden /><span>New page</span><kbd>⌘⇧B</kbd></Squircle>}</div>{features.browser && <><h2>Suggested</h2><div className="side-panel-suggested">{recent.map(page => <button key={page.url} onClick={() => { void openPage(page.url); }}>{typeof page.favicon === "string" && page.favicon.startsWith("data:image/") ? <img src={page.favicon} alt="" /> : <Globe aria-hidden />}<span>{page.title}</span></button>)}{!recent.length && <p>Recently visited sites will appear here.</p>}</div></>}</div> : features.browser && active.kind === "browser" && state.open ? <BrowserPane embedded onBrowserContext={onBrowserContext} searchUrl={searchUrl} agentActive={browser.agentActive} onTakeOver={browser.agentActive ? browser.takeOver : onInterrupt} onClose={() => { void close(active); }} /> : null}
        {tools.filter(tool => tool.id === "terminal" || state.tabs.some(tab => tab.kind === tool.id)).map(tool => <div className="side-panel-tool-content" key={tool.id} hidden={active?.kind !== tool.id}><DiagnosticBoundary panel={tool.id}>{tool.content}</DiagnosticBoundary></div>)}
      </div>
    </aside>}
    </div>
  </div></PanelContext.Provider>;
}
