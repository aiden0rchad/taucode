// @vitest-environment jsdom
import { expect, it } from "vitest";
import { getLayoutMode } from "./useLayoutMode";
import { sidePanelIsOverlay } from "@/features/panels/panelState";
it("keeps desktop chrome at 960px and reserves the rail for touch devices", () => {
  expect(getLayoutMode(960, false, false)).toBe("desktop");
  expect(getLayoutMode(700, false, false)).toBe("desktop");
  expect(getLayoutMode(699, false, false)).toBe("desktop");
  expect(getLayoutMode(960, false, true)).toBe("tablet");
  expect(getLayoutMode(500, false, false)).toBe("desktop");
  expect(getLayoutMode(960, true, true)).toBe("phone");
});
it("reserves 480px of conversation space, or 360px without a sidebar", () => {
  expect(sidePanelIsOverlay(960, 280, 330)).toBe(true);
  expect(sidePanelIsOverlay(1440, 280, 330)).toBe(false);
  expect(sidePanelIsOverlay(960, 0, 330)).toBe(false);
  expect(sidePanelIsOverlay(1090, 280, 330)).toBe(false);
  expect(sidePanelIsOverlay(1089, 280, 330)).toBe(true);
  expect(sidePanelIsOverlay(690, 0, 330)).toBe(false);
  expect(sidePanelIsOverlay(689, 0, 330)).toBe(true);
});

import { workspaceLayout } from "@/features/panels/panelState";
it.each([760, 960, 1180, 1440])("reserves conversation width with bounded sidebars at %ipx", width => {
  for (const sidebarWidth of [220, 280, 420]) for (const collapsed of [false, true]) for (const panelOpen of [false, true]) {
    const layout = workspaceLayout(width, sidebarWidth, collapsed, 360, panelOpen);
    expect(layout.conversationWidth).toBeGreaterThanOrEqual(collapsed ? 360 : 480);
    expect(layout.maxPanelWidth).toBeGreaterThanOrEqual(320);
    expect(layout.sidebarWidth).toBeLessThanOrEqual(sidebarWidth);
    expect(layout.sidebarColumn + layout.conversationWidth + (panelOpen && !layout.panelOverlay ? 360 : 0)).toBe(width);
    expect(layout.sidebarOverlay).toBe(false);
  }
});
it("uses the full conversation width when the sidebar reopens as a sheet below 760px", () => {
  for (const collapsed of [true, false]) {
    expect(workspaceLayout(740, 280, collapsed, 360, true)).toMatchObject({ sidebarOverlay: true, sidebarColumn: 0, conversationWidth: 740, panelOverlay: true });
  }
});
