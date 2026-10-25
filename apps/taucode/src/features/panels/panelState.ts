export type PanelTab = { id: string; kind: string; title: string; url?: string; target?: string };
export type PanelState = { tabs: PanelTab[]; active: string | null; open: boolean };
export type PanelAction = { type: "open"; tab: PanelTab } | { type: "close"; id: string } | { type: "select"; id: string | null } | { type: "reorder"; id: string; before: string; after?: boolean };
export function panelReducer(state: PanelState, action: PanelAction): PanelState {
  if (action.type === "open") return { tabs: state.tabs.some(t => t.id === action.tab.id) ? state.tabs.map(t => t.id === action.tab.id ? { ...t, ...action.tab } : t) : [...state.tabs, action.tab], active: action.tab.id, open: true };
  if (action.type === "select") return { ...state, active: action.id, open: true };
  if (action.type === "close") {
    const index = state.tabs.findIndex(t => t.id === action.id);
    const tabs = state.tabs.filter(t => t.id !== action.id);
    return { ...state, tabs, active: state.active === action.id ? tabs[Math.max(0, index - 1)]?.id ?? null : state.active };
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
    return { tabs: [...new Map<string, PanelTab>(tabs.map((t: PanelTab) => [t.id, t])).values()], active: tabs.some((t: PanelTab) => t.id === value.active) ? value.active : null, open: value.open };
  } catch { return fallback; }
}

export const sidePanelIsOverlay = (windowWidth: number, sidebarWidth: number, panelWidth: number) => windowWidth - sidebarWidth - panelWidth < 520;

export function workspaceLayout(width: number, preferredSidebarWidth: number, sidebarCollapsed: boolean, panelWidth: number, panelOpen: boolean) {
  const sidebarOverlay = width < 760;
  const sidebarWidth = Math.min(preferredSidebarWidth, sidebarOverlay ? width : Math.max(0, width - 520));
  const sidebarColumn = sidebarCollapsed || sidebarOverlay ? 0 : sidebarWidth;
  const panelOverlay = sidePanelIsOverlay(width, sidebarColumn, panelWidth);
  const conversationWidth = width - sidebarColumn - (panelOpen && !panelOverlay ? panelWidth : 0);
  return { sidebarWidth, sidebarColumn, sidebarOverlay, panelOverlay, conversationWidth };
}
