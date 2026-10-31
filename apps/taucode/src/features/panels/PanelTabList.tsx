import { ChevronDown, CircleAlert, Globe, LoaderCircle } from "lucide-react";
import type { PanelTab } from "./panelState";

export type PageStatus = { loading: boolean; error?: string };
export type PageStatuses = Record<string, PageStatus>;
export const pageStatusText = (status?: PageStatus) => status?.error ? `Failed to load: ${status.error}` : status?.loading ? "Loading" : "";

export function BrowserTabIcon({ status }: { status?: PageStatus }) {
  const Icon = status?.error ? CircleAlert : status?.loading ? LoaderCircle : Globe;
  return <Icon aria-hidden className={`browser-tab-icon${status?.error ? " is-error" : status?.loading ? " is-loading" : ""}`} />;
}

export function PanelTabList({ tabs, active, statuses, onSelect, label }: { tabs: PanelTab[]; active?: string | null; statuses: PageStatuses; onSelect: (id: string) => void; label: string }) {
  if (tabs.length < 2) return null;
  return <label className="panel-tab-list" title={label}>
    <span aria-hidden>{tabs.length}<ChevronDown /></span>
    <select aria-label={label} value={active ?? ""} onChange={event => onSelect(event.target.value)}>
      {!active && <option value="" disabled>Select a tab</option>}
      {tabs.map((tab, index) => <option key={tab.id} value={tab.id}>{[`${index + 1}. ${tab.title}`, tab.kind === "browser" ? tab.url : "", pageStatusText(statuses[tab.target ?? ""])].filter(Boolean).join(" — ")}</option>)}
    </select>
  </label>;
}
