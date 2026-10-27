import type { PointerEvent as ReactPointerEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";

import { usePanelResize } from "./usePanelResize";
import { useSidebarState } from "./useSidebarState";
import { MIN_RIGHT_PANEL_WIDTH, workspaceLayout } from "@/features/panels/panelState";
const STORAGE_KEY_RIGHT_PANEL = "codexmonitor.rightPanelWidth";
const STORAGE_KEY_CHAT_DIFF_SPLIT_POSITION_PERCENT =
  "codexmonitor.chatDiffSplitPositionPercent";
const STORAGE_KEY_PLAN_PANEL = "codexmonitor.planPanelHeight";
const STORAGE_KEY_TERMINAL_PANEL = "codexmonitor.terminalPanelHeight";
const STORAGE_KEY_DEBUG_PANEL = "codexmonitor.debugPanelHeight";
const MIN_CHAT_DIFF_SPLIT_POSITION_PERCENT = 20;
const MAX_CHAT_DIFF_SPLIT_POSITION_PERCENT = 80;
const MIN_PLAN_PANEL_HEIGHT = 140;
const MAX_PLAN_PANEL_HEIGHT = 420;
const MIN_TERMINAL_PANEL_HEIGHT = 140;
const MAX_TERMINAL_PANEL_HEIGHT = 480;
const MIN_DEBUG_PANEL_HEIGHT = 120;
const MAX_DEBUG_PANEL_HEIGHT = 420;
const DEFAULT_CHAT_DIFF_SPLIT_POSITION_PERCENT = 50;
const DEFAULT_RIGHT_PANEL_WIDTH = 360;
const DEFAULT_PLAN_PANEL_HEIGHT = 220;
const DEFAULT_TERMINAL_PANEL_HEIGHT = 220;
const DEFAULT_DEBUG_PANEL_HEIGHT = 180;

type ResizeState = {
  type:
    | "right-panel"
    | "chat-diff-split"
    | "plan-panel"
    | "terminal-panel"
    | "debug-panel";
  startX: number;
  startY: number;
  startWidth: number;
  startHeight: number;
  startContainerWidth?: number;
  startContainerLeft?: number;
};

const CSS_VAR_MAP: Record<
  ResizeState["type"],
  { prop: string; unit: string }
> = {
  "right-panel": { prop: "--right-panel-width", unit: "px" },
  "chat-diff-split": {
    prop: "--chat-diff-split-position-percent",
    unit: "%",
  },
  "plan-panel": { prop: "--plan-panel-height", unit: "px" },
  "terminal-panel": { prop: "--terminal-panel-height", unit: "px" },
  "debug-panel": { prop: "--debug-panel-height", unit: "px" },
};

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function getRightPanelMaxWidth(windowWidth: number, sidebarWidth: number, sidebarCollapsed: boolean) {
  return workspaceLayout(windowWidth, sidebarWidth, sidebarCollapsed, MIN_RIGHT_PANEL_WIDTH, false).maxPanelWidth;
}

function readStoredWidth(key: string, fallback: number, min: number, max: number) {
  if (typeof window === "undefined") {
    return fallback;
  }
  const raw = window.localStorage.getItem(key);
  if (!raw) {
    return fallback;
  }
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return clamp(parsed, min, max);
}

function getContainerPointerPercent(event: PointerEvent, resize: ResizeState) {
  const containerWidth = resize.startContainerWidth ?? 1;
  const containerLeft = resize.startContainerLeft ?? 0;
  return ((event.clientX - containerLeft) / containerWidth) * 100;
}

export function useResizablePanels(availableWindowWidth?: number) {
  const { sidebarWidth, sidebarCollapsed, onSidebarResizeStart, isResizing: sidebarResizing } = useSidebarState();
  const [chatDiffSplitPositionPercent, setChatDiffSplitPositionPercent] =
    useState(() =>
      readStoredWidth(
        STORAGE_KEY_CHAT_DIFF_SPLIT_POSITION_PERCENT,
        DEFAULT_CHAT_DIFF_SPLIT_POSITION_PERCENT,
        MIN_CHAT_DIFF_SPLIT_POSITION_PERCENT,
        MAX_CHAT_DIFF_SPLIT_POSITION_PERCENT,
      ),
    );
  const [preferredRightPanelWidth, setPreferredRightPanelWidth] = useState(() =>
    readStoredWidth(
      STORAGE_KEY_RIGHT_PANEL,
      DEFAULT_RIGHT_PANEL_WIDTH,
      MIN_RIGHT_PANEL_WIDTH,
      Infinity,
    ),
  );
  const [planPanelHeight, setPlanPanelHeight] = useState(() =>
    readStoredWidth(
      STORAGE_KEY_PLAN_PANEL,
      DEFAULT_PLAN_PANEL_HEIGHT,
      MIN_PLAN_PANEL_HEIGHT,
      MAX_PLAN_PANEL_HEIGHT,
    ),
  );
  const [terminalPanelHeight, setTerminalPanelHeight] = useState(() =>
    readStoredWidth(
      STORAGE_KEY_TERMINAL_PANEL,
      DEFAULT_TERMINAL_PANEL_HEIGHT,
      MIN_TERMINAL_PANEL_HEIGHT,
      MAX_TERMINAL_PANEL_HEIGHT,
    ),
  );
  const [debugPanelHeight, setDebugPanelHeight] = useState(() =>
    readStoredWidth(
      STORAGE_KEY_DEBUG_PANEL,
      DEFAULT_DEBUG_PANEL_HEIGHT,
      MIN_DEBUG_PANEL_HEIGHT,
      MAX_DEBUG_PANEL_HEIGHT,
    ),
  );
  const [viewportWidth, setWindowWidth] = useState(() => window.innerWidth);
  const windowWidth = availableWindowWidth ?? viewportWidth;
  useEffect(() => {
    const resize = () => setWindowWidth(window.innerWidth);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  const maxRightPanelWidth = getRightPanelMaxWidth(windowWidth, sidebarWidth, sidebarCollapsed);
  // Window and sidebar changes constrain rendering without overwriting the preference.
  const rightPanelWidth = clamp(preferredRightPanelWidth, MIN_RIGHT_PANEL_WIDTH, maxRightPanelWidth);
  const setRightPanelWidth = useCallback((value: number) => {
    setPreferredRightPanelWidth(clamp(value, MIN_RIGHT_PANEL_WIDTH, maxRightPanelWidth));
  }, [maxRightPanelWidth]);
  const appRef = useRef<HTMLDivElement | null>(null);
  const { isResizing, startResize } = usePanelResize();

  useEffect(() => {
    window.localStorage.setItem(
      STORAGE_KEY_CHAT_DIFF_SPLIT_POSITION_PERCENT,
      String(chatDiffSplitPositionPercent),
    );
  }, [chatDiffSplitPositionPercent]);

  useEffect(() => {
    window.localStorage.setItem(
      STORAGE_KEY_RIGHT_PANEL,
      String(preferredRightPanelWidth),
    );
  }, [preferredRightPanelWidth]);

  useEffect(() => {
    window.localStorage.setItem(
      STORAGE_KEY_PLAN_PANEL,
      String(planPanelHeight),
    );
  }, [planPanelHeight]);

  useEffect(() => {
    window.localStorage.setItem(
      STORAGE_KEY_TERMINAL_PANEL,
      String(terminalPanelHeight),
    );
  }, [terminalPanelHeight]);

  useEffect(() => {
    window.localStorage.setItem(
      STORAGE_KEY_DEBUG_PANEL,
      String(debugPanelHeight),
    );
  }, [debugPanelHeight]);

  const beginResize = useCallback((event: ReactPointerEvent, resize: ResizeState, cursor: "col-resize" | "row-resize") => {
    let liveValue: number | null = null;
    startResize(event, cursor, event => {
      const el = appRef.current;
      if (!el) return;
      event.preventDefault();
      let next: number;
      if (resize.type === "chat-diff-split") {
        const pointerPercent = getContainerPointerPercent(event, resize);
        next = clamp(
          pointerPercent,
          MIN_CHAT_DIFF_SPLIT_POSITION_PERCENT,
          MAX_CHAT_DIFF_SPLIT_POSITION_PERCENT,
        );
      } else if (resize.type === "right-panel") {
        const delta = event.clientX - resize.startX;
        next = clamp(
          resize.startWidth - delta,
          MIN_RIGHT_PANEL_WIDTH,
          maxRightPanelWidth,
        );
      } else if (resize.type === "plan-panel") {
        const delta = event.clientY - resize.startY;
        next = clamp(
          resize.startHeight - delta,
          MIN_PLAN_PANEL_HEIGHT,
          MAX_PLAN_PANEL_HEIGHT,
        );
      } else if (resize.type === "terminal-panel") {
        const delta = event.clientY - resize.startY;
        next = clamp(
          resize.startHeight - delta,
          MIN_TERMINAL_PANEL_HEIGHT,
          MAX_TERMINAL_PANEL_HEIGHT,
        );
      } else {
        const delta = event.clientY - resize.startY;
        next = clamp(
          resize.startHeight - delta,
          MIN_DEBUG_PANEL_HEIGHT,
          MAX_DEBUG_PANEL_HEIGHT,
        );
      }

      liveValue = next;
      const { prop, unit } = CSS_VAR_MAP[resize.type];
      el.style.setProperty(prop, `${next}${unit}`);
    }, () => {
      if (liveValue !== null) {
        switch (resize.type) {
          case "chat-diff-split":
            setChatDiffSplitPositionPercent(liveValue);
            break;
          case "right-panel":
            setRightPanelWidth(liveValue);
            break;
          case "plan-panel":
            setPlanPanelHeight(liveValue);
            break;
          case "terminal-panel":
            setTerminalPanelHeight(liveValue);
            break;
          case "debug-panel":
            setDebugPanelHeight(liveValue);
            break;
        }
      }
    });
  }, [startResize, maxRightPanelWidth, setRightPanelWidth]);

  const onChatDiffSplitPositionResizeStart = useCallback(
    (event: ReactPointerEvent) => {
      const content = event.currentTarget.closest(".content-split") as
        | HTMLDivElement
        | null;
      beginResize(event, {
        type: "chat-diff-split",
        startX: event.clientX,
        startY: event.clientY,
        startWidth: chatDiffSplitPositionPercent,
        startHeight: 0,
        startContainerWidth: content?.clientWidth,
        startContainerLeft: content?.getBoundingClientRect().left,
      }, "col-resize");
    },
    [beginResize, chatDiffSplitPositionPercent],
  );

  const onRightPanelResizeStart = useCallback(
    (event: ReactPointerEvent) => {
      beginResize(event, {
        type: "right-panel",
        startX: event.clientX,
        startY: event.clientY,
        startWidth: event.currentTarget.closest(".tabbed-side-panel")?.getBoundingClientRect().width ?? rightPanelWidth,
        startHeight: planPanelHeight,
      }, "col-resize");
    },
    [beginResize, planPanelHeight, rightPanelWidth],
  );

  const onPlanPanelResizeStart = useCallback(
    (event: ReactPointerEvent) => {
      beginResize(event, {
        type: "plan-panel",
        startX: event.clientX,
        startY: event.clientY,
        startWidth: rightPanelWidth,
        startHeight: planPanelHeight,
      }, "row-resize");
    },
    [beginResize, planPanelHeight, rightPanelWidth],
  );

  const onTerminalPanelResizeStart = useCallback(
    (event: ReactPointerEvent) => {
      beginResize(event, {
        type: "terminal-panel",
        startX: event.clientX,
        startY: event.clientY,
        startWidth: rightPanelWidth,
        startHeight: terminalPanelHeight,
      }, "row-resize");
    },
    [beginResize, rightPanelWidth, terminalPanelHeight],
  );

  const onDebugPanelResizeStart = useCallback(
    (event: ReactPointerEvent) => {
      beginResize(event, {
        type: "debug-panel",
        startX: event.clientX,
        startY: event.clientY,
        startWidth: rightPanelWidth,
        startHeight: debugPanelHeight,
      }, "row-resize");
    },
    [beginResize, debugPanelHeight, rightPanelWidth],
  );

  return {
    appRef,
    isResizing: isResizing || sidebarResizing,
    sidebarWidth,
    rightPanelWidth,
    maxRightPanelWidth,
    setRightPanelWidth,
    planPanelHeight,
    terminalPanelHeight,
    debugPanelHeight,
    onSidebarResizeStart,
    chatDiffSplitPositionPercent,
    onChatDiffSplitPositionResizeStart,
    onRightPanelResizeStart,
    onPlanPanelResizeStart,
    onTerminalPanelResizeStart,
    onDebugPanelResizeStart,
  };
}
