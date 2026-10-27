export type PanelTab = { id: string; kind: string; title: string; url?: string; target?: string };
export type PanelState = { tabs: PanelTab[]; active: string | null; open: boolean; browserActive?: string; dock?: DockLayout };
export type PanelAction = { type: "open"; tab: PanelTab } | { type: "close"; id: string } | { type: "select"; id: string | null } | { type: "reorder"; id: string; before: string; after?: boolean };
export function panelReducer(state: PanelState, action: PanelAction): PanelState {
  if (action.type === "open") return { ...state, ...(state.dock ? { dock: showDockPane(state.dock, action.tab.kind) } : {}), ...(state.dock && action.tab.kind === "browser" ? { browserActive: action.tab.id } : {}), tabs: state.tabs.some(t => t.id === action.tab.id) ? state.tabs.map(t => t.id === action.tab.id ? { ...t, ...action.tab } : t) : [...state.tabs, action.tab], active: action.tab.id, open: true };
  if (action.type === "select") {
    const kind = state.tabs.find(tab => tab.id === action.id)?.kind;
    return { ...state, ...(state.dock && kind === "browser" ? { browserActive: action.id! } : {}), ...(state.dock && kind ? { dock: showDockPane(state.dock, kind) } : {}), active: action.id, open: true };
  }
  if (action.type === "close") {
    const index = state.tabs.findIndex(t => t.id === action.id);
    if (index < 0) return state;
    const tabs = state.tabs.filter(t => t.id !== action.id);
    const browserActive = state.browserActive === action.id ? tabs.find(tab => tab.kind === "browser")?.id : state.browserActive;
    const kind = state.tabs[index].kind;
    return { ...state, ...(state.browserActive ? { browserActive } : {}), ...(state.dock && !tabs.some(tab => tab.kind === kind) ? { dock: hideDockPane(state.dock, kind) } : {}), tabs, active: state.active === action.id ? tabs[Math.max(0, index - 1)]?.id ?? null : state.active };
  }
  const tab = state.tabs.find(t => t.id === action.id);
  if (!tab || action.id === action.before || !state.tabs.some(t => t.id === action.before)) return state;
  const tabs = state.tabs.filter(t => t.id !== action.id);
  tabs.splice(tabs.findIndex(t => t.id === action.before) + (action.after ? 1 : 0), 0, tab);
  return { ...state, tabs };
}
export function readPanel(key: string, task = false): PanelState {
  if (import.meta.env.MODE === "uipreview") {
    const kind = new URLSearchParams(window.location.search).get("panel");
    if (kind === "new") return { tabs: [], active: null, open: true };
    if (kind === "terminal" && key.startsWith("code:")) return { tabs: [{ id: "terminal", kind, title: "Terminal" }], active: "terminal", open: true };
    if (kind === "browser") return { tabs: [{ id: "preview-page", kind, title: "Example", url: "https://example.com/", target: "preview-page" }], active: "preview-page", open: true };
  }
  const fallback = { tabs: task ? [{ id: "task", kind: "task", title: "Task" }] : [], active: task ? "task" : null, open: task };
  try {
    const value = JSON.parse(localStorage.getItem(`taucode.panel.${key}`) ?? "null");
    if (!value || !Array.isArray(value.tabs) || typeof value.open !== "boolean") return fallback;
    const tabs = value.tabs.filter((t: PanelTab) => t && typeof t.id === "string" && typeof t.kind === "string" && typeof t.title === "string" && (t.url === undefined || typeof t.url === "string") && (t.target === undefined || typeof t.target === "string"));
    const dock = readDock(value.dock);
    const browserActive = tabs.find((tab: PanelTab) => tab.kind === "browser" && tab.id === (value.browserActive ?? value.active))?.id;
    return { ...(dock ? { dock } : {}), ...(dock && browserActive ? { browserActive } : {}), tabs: [...new Map<string, PanelTab>(tabs.map((t: PanelTab) => [t.id, t])).values()], active: tabs.some((t: PanelTab) => t.id === value.active) ? value.active : null, open: value.open };
  } catch { return fallback; }
}

export const MIN_RIGHT_PANEL_WIDTH = 320;
const dockedPanelSpace = (windowWidth: number, sidebarWidth: number) => windowWidth - sidebarWidth - (sidebarWidth ? 480 : 360);
export const sidePanelIsOverlay = (windowWidth: number, sidebarWidth: number, panelWidth: number) => dockedPanelSpace(windowWidth, sidebarWidth) < Math.max(MIN_RIGHT_PANEL_WIDTH, panelWidth);

export function workspaceLayout(width: number, preferredSidebarWidth: number, sidebarCollapsed: boolean, panelWidth: number, panelOpen: boolean) {
  const sidebarOverlay = width < 760;
  const sidebarWidth = Math.min(preferredSidebarWidth, sidebarOverlay ? width : Math.max(0, width - 520));
  const sidebarColumn = sidebarCollapsed || sidebarOverlay ? 0 : sidebarWidth;
  const panelOverlay = sidebarOverlay || sidePanelIsOverlay(width, sidebarColumn, panelWidth);
  const maxPanelWidth = Math.max(MIN_RIGHT_PANEL_WIDTH, panelOverlay ? width - 80 : dockedPanelSpace(width, sidebarColumn));
  const conversationWidth = width - sidebarColumn - (panelOpen && !panelOverlay ? panelWidth : 0);
  return { sidebarWidth, sidebarColumn, sidebarOverlay, panelOverlay, maxPanelWidth, conversationWidth };
}

export type DockSide = "left" | "right" | "top" | "bottom";
export type DockNode = { type: "pane"; id: string } | { type: "split"; id: string; axis: "horizontal" | "vertical"; ratio: number; first: DockNode; second: DockNode };
export type DockLayout = { tree: DockNode; expanded: string | null; focused?: string };
export type DockBox = { x: number; y: number; w: number; h: number };
export const dockPanes = (node: DockNode): string[] => node.type === "pane" ? [node.id] : [...dockPanes(node.first), ...dockPanes(node.second)];
export function removeDockPane(node: DockNode, id: string): DockNode | null {
  if (node.type === "pane") return node.id === id ? null : node;
  const first = removeDockPane(node.first, id), second = removeDockPane(node.second, id);
  return first && second ? { ...node, first, second } : first ?? second;
}
function splitDock(node: DockNode, pane: string, side: DockSide, share?: number): DockNode {
  const before = side === "left" || side === "top", horizontal = side === "left" || side === "right";
  const leaf: DockNode = { type: "pane", id: pane };
  return { type: "split", id: crypto.randomUUID(), axis: horizontal ? "horizontal" : "vertical", ratio: share === undefined ? horizontal ? (before ? .32 : .68) : .5 : before ? share : 1 - share, first: before ? leaf : node, second: before ? node : leaf };
}
export function moveDockPane(layout: DockLayout, pane: string, side: DockSide, target?: string): DockLayout {
  if (pane === "conversation" || pane === target) return layout;
  if (!target) {
    const reorder = (node: DockNode): DockNode => {
      if (node.type === "pane") return node;
      const firstIsPane = node.first.type === "pane" && node.first.id === pane;
      const secondIsPane = node.second.type === "pane" && node.second.id === pane;
      if (firstIsPane || secondIsPane) {
        const leaf = firstIsPane ? node.first : node.second, sibling = firstIsPane ? node.second : node.first;
        const share = firstIsPane ? node.ratio : 1 - node.ratio;
        const before = side === "left" || side === "top";
        return { ...node, axis: side === "left" || side === "right" ? "horizontal" : "vertical", ratio: before ? share : 1 - share, first: before ? leaf : sibling, second: before ? sibling : leaf };
      }
      const first = reorder(node.first), second = reorder(node.second);
      return first === node.first && second === node.second ? node : { ...node, first, second };
    };
    const tree = reorder(layout.tree);
    if (tree !== layout.tree) return { ...layout, tree, focused: pane, expanded: null };
  }
  const previous = dockGeometry(layout.tree).panes[pane];
  const share = previous ? Math.min(.85, Math.max(.15, Math.min(previous.w, previous.h) / 100)) : undefined;
  const rest = removeDockPane(layout.tree, pane) ?? { type: "pane", id: "conversation" };
  const insert = (node: DockNode): DockNode => node.type === "pane" ? node.id === target ? splitDock(node, pane, side, share) : node : { ...node, first: insert(node.first), second: insert(node.second) };
  return { focused: pane, tree: target && dockPanes(rest).includes(target) ? insert(rest) : splitDock(rest, pane, side, share), expanded: null };
}
export function showDockPane(layout: DockLayout, pane: string): DockLayout {
  if (dockPanes(layout.tree).includes(pane)) return { ...layout, focused: pane, expanded: layout.expanded === pane ? pane : null };
  if (pane === "session") return moveDockPane(layout, pane, "right", "conversation");
  // New tools stack beside the conversation and summary rail.
  const append = (node: DockNode): DockNode => {
    const ids = dockPanes(node);
    if (!ids.some(id => id === "conversation" || id === "session")) return splitDock(node, pane, "bottom", Math.max(.15, 1 / (ids.length + 1)));
    if (node.type === "split") {
      if (!dockPanes(node.first).some(id => id === "conversation" || id === "session")) return { ...node, first: append(node.first) };
      if (!dockPanes(node.second).some(id => id === "conversation" || id === "session")) return { ...node, second: append(node.second) };
    }
    return splitDock(node, pane, "right");
  };
  return { expanded: null, focused: pane, tree: append(layout.tree) };
}
export function hideDockPane(layout: DockLayout, pane: string): DockLayout {
  if (pane === "conversation") return layout;
  return { focused: layout.focused === pane ? "conversation" : layout.focused, tree: removeDockPane(layout.tree, pane) ?? { type: "pane", id: "conversation" }, expanded: layout.expanded === pane ? null : layout.expanded };
}
export function resizeDockSplit(node: DockNode, id: string, ratio: number): DockNode {
  if (node.type === "pane" || !Number.isFinite(ratio)) return node;
  return node.id === id ? { ...node, ratio: Math.min(.85, Math.max(.15, ratio)) } : { ...node, first: resizeDockSplit(node.first, id, ratio), second: resizeDockSplit(node.second, id, ratio) };
}
export function dockGeometry(node: DockNode, box: DockBox = { x: 0, y: 0, w: 100, h: 100 }) {
  const panes: Record<string, DockBox> = Object.create(null);
  const splits: { node: Extract<DockNode, { type: "split" }>; box: DockBox }[] = [];
  const visit = (node: DockNode, box: DockBox) => {
    if (node.type === "pane") { panes[node.id] = box; return; }
    splits.push({ node, box });
    if (node.axis === "horizontal") {
      visit(node.first, { ...box, w: box.w * node.ratio });
      visit(node.second, { ...box, x: box.x + box.w * node.ratio, w: box.w * (1 - node.ratio) });
    } else {
      visit(node.first, { ...box, h: box.h * node.ratio });
      visit(node.second, { ...box, y: box.y + box.h * node.ratio, h: box.h * (1 - node.ratio) });
    }
  };
  visit(node, box); return { panes, splits };
}
export function initialDock(state: PanelState, rail: boolean): DockLayout {
  let layout: DockLayout = { tree: { type: "pane", id: "conversation" }, expanded: null };
  if (rail) layout = showDockPane(layout, "session");
  const active = state.tabs.find(tab => tab.id === state.active);
  return state.open && active ? showDockPane(layout, active.kind) : { ...layout, focused: "conversation" };
}
function readDock(value: unknown): DockLayout | undefined {
  const seen = new Set<string>(); let count = 0;
  const read = (value: unknown, depth = 0): DockNode | null => {
    if (!value || typeof value !== "object" || depth > 12 || ++count > 63) return null;
    const node = value as Record<string, unknown>;
    if (typeof node.id !== "string" || !/^[\w-]{1,80}$/.test(node.id) || seen.has(node.id)) return null;
    seen.add(node.id);
    if (node.type === "pane") return { type: "pane", id: node.id };
    if (node.type !== "split" || !["horizontal", "vertical"].includes(String(node.axis)) || typeof node.ratio !== "number" || !Number.isFinite(node.ratio)) return null;
    const first = read(node.first, depth + 1), second = read(node.second, depth + 1);
    return first && second ? { type: "split", id: node.id, axis: node.axis as "horizontal" | "vertical", ratio: Math.min(.85, Math.max(.15, node.ratio)), first, second } : null;
  };
  const layout = value as DockLayout | null;
  const tree = read(layout?.tree);
  if (!tree || !dockPanes(tree).includes("conversation")) return undefined;
  return { tree, ...(layout?.focused && dockPanes(tree).includes(layout.focused) ? { focused: layout.focused } : {}), expanded: typeof layout?.expanded === "string" && layout.expanded !== "conversation" && dockPanes(tree).includes(layout.expanded) ? layout.expanded : null };
}
