// @vitest-environment jsdom
import { createRef, type ComponentProps } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ModeShell } from "@/features/chat/ModeShell";
import { MainAppShell } from "./MainAppShell";
import { MainHeader } from "./MainHeader";
import { MainHeaderActions } from "./MainHeaderActions";
import { SidebarHeader } from "./SidebarHeader";
import { SidebarBottomRail } from "./SidebarBottomRail";
import { PrimaryAction } from "./AppShell";
import { AppShell } from "./AppShell";
import { MainTopbar } from "./MainTopbar";
import { AppLayout } from "./AppLayout";
vi.mock("@/features/chat/ChatView", () => ({ ChatView: () => null }));
vi.mock("@/features/work/WorkView", () => ({ WorkView: () => null }));
vi.mock("@/features/layout/hooks/useWindowLabel", () => ({ useWindowLabel: () => "main" }));
vi.mock("@services/tauri", () => ({ getAppSettings: async () => ({ windowModes: { main: "code" } }), setWindowMode: vi.fn() }));
vi.mock("@services/events", () => ({ subscribeAppServerEvents: () => () => {} }));
vi.mock("./AppModals", () => ({ AppModals: () => null }));
vi.mock("@/features/browser/BrowserPane", () => ({ BrowserPane: ({ onClose }: { onClose: () => void }) => <button onClick={onClose}>Close browser</button> }));
afterEach(() => { cleanup(); localStorage.clear(); vi.restoreAllMocks(); });

it("renders Code in the shared shell with ordered header actions and no non-git branch chip", async () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  const noop = vi.fn();
  const toggle = { isCompact: false, sidebarCollapsed: false, rightPanelCollapsed: false, onCollapseSidebar: noop, onExpandSidebar: noop, onCollapseRightPanel: noop, onExpandRightPanel: noop };
  const header = <MainHeader workspace={{ id: "w", name: "Workspace", path: "/folder", connected: true, settings: { sidebarCollapsed: false } }} threadTitle="Thread title" branchName="" branches={[]} openTargets={[]} openAppIconById={{}} selectedOpenAppId="" onSelectOpenAppId={noop} onCheckoutBranch={noop} onCreateBranch={noop} onToggleTerminal={noop} isTerminalOpen={false} showWorkspaceTools={false} extraActionsNode={<MainHeaderActions centerMode="chat" gitDiffViewStyle="split" onSelectDiffViewStyle={noop} isCompact={false} rightPanelCollapsed={false} sidebarToggleProps={toggle} />} />;
  const layout: ComponentProps<typeof AppLayout> = {
    isPhone: false, isTablet: false, showHome: false, showGitDetail: false, activeTab: "codex", tabletTab: "codex", centerMode: "chat", preloadGitDiffs: false, splitChatDiffView: false, hasActivePlan: false, activeWorkspace: true,
    sidebarNode: <aside className="sidebar"><SidebarHeader primaryAction={<PrimaryAction onClick={noop}>New thread</PrimaryAction>} onSelectHome={noop} onAddWorkspace={noop} onToggleSearch={noop} isSearchOpen={false} threadListSortKey="updated_at" onSetThreadListSortKey={noop} threadListOrganizeMode="by_project" onSetThreadListOrganizeMode={noop} onRefreshAllThreads={noop} /><SidebarBottomRail onOpenSettings={noop} onOpenDebug={noop} showDebugButton={false} /></aside>,
    mainHeaderNode: header, desktopTopbarLeftNode: header,
    messagesNode: null, composerNode: null, approvalToastsNode: null, updateToastNode: null, errorToastsNode: null, homeNode: null, tabletNavNode: null, tabBarNode: null, gitDiffPanelNode: null, gitDiffViewerNode: null, planPanelNode: null, debugPanelNode: null, debugPanelFullNode: null, terminalDockNode: null, compactEmptyCodexNode: null, compactEmptyGitNode: null, compactGitBackNode: null,
    onSidebarResizeStart: noop, onChatDiffSplitPositionResizeStart: noop, onRightPanelResizeStart: noop, onPlanPanelResizeStart: noop,
  };
  type Props = ComponentProps<typeof MainAppShell>;
  const { container } = render(<ModeShell><MainAppShell appClassName="app layout-desktop" appStyle={{}} appRef={createRef()} isResizing={false} sidebarToggleProps={toggle} shouldLoadGitHubPanelData={false} gitHubPanelDataProps={{} as Props["gitHubPanelDataProps"]} appLayoutProps={layout} appModalsProps={{} as Props["appModalsProps"]} showMobileSetupWizard={false} mobileSetupWizardProps={{} as Props["mobileSetupWizardProps"]} /></ModeShell>);
  await screen.findByText("Thread title");
  expect(container.querySelector(".app-shell")).toBeTruthy();
  expect(screen.getByRole("navigation", { name: "App mode" })).toBeTruthy();
  expect(container.querySelector(".shell-footer")?.contains(screen.getByRole("button", { name: "Open settings" }))).toBe(true);
  const topbar = container.querySelector(".main-topbar") as HTMLElement;
  const buttons = within(topbar).getAllByRole("button");
  expect(buttons.slice(-2).map(button => button.getAttribute("aria-label"))).toEqual(["Browser", "Hide git sidebar"]);
  expect(container.querySelector(".workspace-branch-menu")).toBeNull();
  expect(container.querySelector(".browser-toggle")).toBeNull();
  fireEvent.click(within(topbar).getByRole("button", { name: "Browser" }));
  expect(await screen.findByRole("button", { name: "Close browser" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Close browser" }));
  await waitFor(() => expect(container.querySelector(".browser-dock")).toBeNull());
});

it("collapses header actions into a keyboard-accessible menu on phones", () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  const { container } = render(<AppShell><MainTopbar leftNode="Chat" actionsNode={<button>Export</button>} /></AppShell>);
  const menu = container.querySelector("details")!;
  expect(menu.open).toBe(false);
  fireEvent.click(screen.getByLabelText("Header actions"));
  expect(menu.open).toBe(true);
  expect(screen.getByRole("button", { name: "Browser" })).toBeTruthy();
  fireEvent.keyDown(screen.getByRole("button", { name: "Browser" }), { key: "Escape" });
  expect(menu.open).toBe(false);
  expect(document.activeElement).toBe(screen.getByLabelText("Header actions"));
});
