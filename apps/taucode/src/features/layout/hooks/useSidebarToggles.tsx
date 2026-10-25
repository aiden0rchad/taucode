import { useEffect, useLayoutEffect, useState } from "react";

import { useSidebarState } from "./useSidebarState";
const RIGHT_PANEL_COLLAPSED_KEY = "codexmonitor.rightPanelCollapsed";

type UseSidebarTogglesOptions = {
  isCompact: boolean;
};

function readStoredBool(key: string) {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(key) === "true";
}

export function useSidebarToggles({ isCompact }: UseSidebarTogglesOptions) {
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(() =>
    readStoredBool(RIGHT_PANEL_COLLAPSED_KEY),
  );

  useLayoutEffect(() => {
    const sync = (event: Event) => setRightPanelCollapsed(!(event as CustomEvent<boolean>).detail);
    window.addEventListener("taucode:panel-visibility", sync);
    return () => window.removeEventListener("taucode:panel-visibility", sync);
  }, []);

  useEffect(() => {
    window.localStorage.setItem(
      RIGHT_PANEL_COLLAPSED_KEY,
      String(rightPanelCollapsed),
    );
  }, [rightPanelCollapsed]);

  const collapseSidebar = () => {
    if (!isCompact) {
      setSidebarCollapsed(true);
    }
  };

  const expandSidebar = () => {
    if (!isCompact) {
      setSidebarCollapsed(false);
    }
  };

  const collapseRightPanel = () => {
    if (!isCompact) {
      setRightPanelCollapsed(true);
      window.dispatchEvent(new CustomEvent("taucode:panel-toggle", { detail: false }));
    }
  };

  const expandRightPanel = () => {
    if (!isCompact) {
      setRightPanelCollapsed(false);
      window.dispatchEvent(new CustomEvent("taucode:panel-toggle", { detail: true }));
    }
  };

  return {
    sidebarCollapsed,
    rightPanelCollapsed,
    collapseSidebar,
    expandSidebar,
    collapseRightPanel,
    expandRightPanel,
  };
}
