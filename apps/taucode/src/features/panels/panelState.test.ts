// @vitest-environment jsdom
import { beforeEach, expect, it } from "vitest";
import { dockGeometry, dockPanes, hideDockPane, initialDock, moveDockPane, panelReducer, readPanel, resizeDockSplit, showDockPane, type DockLayout, type PanelState } from "./panelState";

beforeEach(() => localStorage.clear());
const legacy: PanelState = { open: true, active: "web-2", tabs: [{ id: "terminal", kind: "terminal", title: "Terminal" }, { id: "web-1", kind: "browser", title: "One" }, { id: "web-2", kind: "browser", title: "Two" }] };
it("migrates the legacy selection while retaining browser tabs and stacks additional tools once", () => {
  let dock = initialDock(legacy, true);
  expect(dockPanes(dock.tree)).toEqual(["conversation", "session", "browser"]);
  dock = showDockPane(showDockPane(dock, "terminal"), "files");
  const panes = dockGeometry(dock.tree).panes;
  expect(Object.keys(panes)).toEqual(["conversation", "session", "browser", "terminal", "files"]);
  expect(panes.browser.x).toBe(panes.terminal.x);
  expect(panes.terminal.x).toBe(panes.files.x);
  expect(panes.browser.y).toBeLessThan(panes.terminal.y);
  expect(panes.terminal.y).toBeLessThan(panes.files.y);
  expect(dockPanes(showDockPane(dock, "browser").tree).filter(id => id === "browser")).toHaveLength(1);
  expect(legacy.tabs).toHaveLength(3);
});
it.each(["left", "right", "top", "bottom"] as const)("moves a pane %s without duplicating it or losing its share", side => {
  const before = initialDock(legacy, false), original = dockGeometry(before.tree).panes.browser.w;
  const moved = moveDockPane(before, "browser", side), boxes = dockGeometry(moved.tree).panes;
  expect(dockPanes(moved.tree).sort()).toEqual(["browser", "conversation"]);
  if (side === "left" || side === "right") {
    expect(boxes.browser.w).toBeCloseTo(original);
    expect(boxes.browser.x < boxes.conversation.x).toBe(side === "left");
  } else {
    expect(boxes.browser.h).toBeCloseTo(original);
    expect(boxes.browser.y < boxes.conversation.y).toBe(side === "top");
  }
});
it("reorders by drop target, constrains both split axes, and closes only the requested pane", () => {
  let dock = showDockPane(initialDock(legacy, false), "terminal");
  dock = moveDockPane(dock, "terminal", "top", "browser");
  const splits = dockGeometry(dock.tree).splits;
  expect(splits.map(split => split.node.axis)).toEqual(["horizontal", "vertical"]);
  for (const { node } of splits) {
    expect(dockGeometry(resizeDockSplit(dock.tree, node.id, 99)).splits.find(split => split.node.id === node.id)?.node.ratio).toBe(.85);
    expect(dockGeometry(resizeDockSplit(dock.tree, node.id, -99)).splits.find(split => split.node.id === node.id)?.node.ratio).toBe(.15);
  }
  const filled = { ...dock, expanded: "browser" };
  expect(hideDockPane(filled, "terminal").expanded).toBe("browser");
  expect(dockPanes(hideDockPane(filled, "browser").tree)).toEqual(["conversation", "terminal"]);
  expect(hideDockPane(filled, "browser").expanded).toBeNull();
  expect(hideDockPane(filled, "conversation")).toBe(filled);
});
it("keeps browser selection independent from the active tool, then selects a remaining page on close", () => {
  let state = { ...legacy, dock: initialDock(legacy, false) };
  state = panelReducer(state, { type: "select", id: "web-2" }) as typeof state;
  state = panelReducer(state, { type: "select", id: "terminal" }) as typeof state;
  expect(state).toMatchObject({ active: "terminal", browserActive: "web-2" });
  state = panelReducer(state, { type: "close", id: "web-2" }) as typeof state;
  expect(state).toMatchObject({ active: "terminal", browserActive: "web-1" });
  expect(dockPanes(state.dock.tree)).toContain("browser");
  state = panelReducer(state, { type: "close", id: "web-1" }) as typeof state;
  expect(dockPanes(state.dock.tree)).not.toContain("browser");
  expect(dockPanes(state.dock.tree)).toContain("terminal");
});
it("round-trips a scoped layout and rejects duplicate, deep and nonfinite saved trees without losing tabs", () => {
  const dock = initialDock(legacy, true);
  const write = (layout: unknown) => localStorage.setItem("taucode.panel.code:one", JSON.stringify({ ...legacy, dock: layout }));
  write(dock); expect(readPanel("code:one").dock).toEqual(dock);
  expect(readPanel("code:other").dock).toBeUndefined();
  write({ ...dock, tree: { type: "split", id: "split", axis: "horizontal", ratio: .5, first: { type: "pane", id: "conversation" }, second: { type: "pane", id: "conversation" } } });
  expect(readPanel("code:one")).toMatchObject({ tabs: legacy.tabs });
  expect(readPanel("code:one").dock).toBeUndefined();
  let deep: DockLayout = initialDock({ tabs: [], open: false, active: null }, false);
  for (let i = 0; i < 15; i++) deep = moveDockPane(deep, `tool-${i}`, "right");
  write(deep); expect(readPanel("code:one").dock).toBeUndefined();
  write({ tree: { ...dock.tree, ratio: null }, expanded: null });
  expect(readPanel("code:one").dock).toBeUndefined();
});

it("swaps a 33/67 right-hand stack locally while keeping conversation, rail and pane sizes", () => {
  let dock = showDockPane(initialDock(legacy, true), "background");
  const stack = dockGeometry(dock.tree).splits.find(({ node }) => node.axis === "vertical")!;
  dock = { ...dock, tree: resizeDockSplit(dock.tree, stack.node.id, .33) };
  const before = dockGeometry(dock.tree).panes;
  const moved = moveDockPane(dock, "background", "top"), after = dockGeometry(moved.tree).panes;
  expect(after.conversation).toEqual(before.conversation);
  expect(after.session).toEqual(before.session);
  expect(after.background.x).toBe(before.background.x);
  expect(after.background.w).toBe(before.background.w);
  expect(after.background.h).toBeCloseTo(before.background.h);
  expect(after.browser.h).toBeCloseTo(before.browser.h);
  expect(after.background.y).toBe(before.browser.y);
  expect(after.browser.y).toBeCloseTo(before.background.h);
  const restored = dockGeometry(moveDockPane(moved, "background", "bottom").tree).panes;
  for (const id of Object.keys(before)) {
    expect(restored[id].x).toBeCloseTo(before[id].x); expect(restored[id].y).toBeCloseTo(before[id].y);
    expect(restored[id].w).toBeCloseTo(before[id].w); expect(restored[id].h).toBeCloseTo(before[id].h);
  }
});

it("balances new tools through six panes instead of exponentially shrinking the old stack", () => {
  let dock = initialDock(legacy, false);
  for (const id of ["terminal", "git", "files", "background", "sidechat"]) dock = showDockPane(dock, id);
  const boxes = dockGeometry(dock.tree).panes;
  for (const id of ["browser", "terminal", "git", "files", "background", "sidechat"]) expect(boxes[id].h).toBeCloseTo(100 / 6);
  let custom = showDockPane(initialDock(legacy, false), "terminal");
  const stack = dockGeometry(custom.tree).splits.find(({ node }) => node.axis === "vertical")!;
  custom = { ...custom, tree: resizeDockSplit(custom.tree, stack.node.id, .33) };
  const expanded = dockGeometry(showDockPane(custom, "files").tree).panes;
  expect(expanded.browser.h / expanded.terminal.h).toBeCloseTo(.33 / .67);
  expect(expanded.files.h).toBeCloseTo(100 / 3);
});
