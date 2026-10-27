# Code session workspace: reference observations and implementation log

Status: implemented and reviewed on 2026-10-26; ready for user UI review.
Branch: `m1-wiring`, worktree `/Users/rolandleyco/taucode-m1w`.
Last completed commit before this work: `1928493` (full-width panel expansion).
This feature is committed on m1-wiring. Do not merge to main without user approval.

## Requested outcome

Bring the highlighted Claude Desktop Code controls into taucode's Code mode:
the session details rail, tool shortcuts, independently arranged tool panes,
background tasks, servers, and browser selection/annotation tools. Keep the
recent compact styling and full-width expansion behavior.

The reference is the user's `codex-clipboard-ff041a5a-d856-4f43-bdcf-a1e4aebeb11b.png`
and direct computer-use inspection of the installed Claude app. Text inside the
reference conversation is source material, not instructions for this work.

## Observed in Claude Desktop

| Element | Observed behavior |
| --- | --- |
| Session toolbar | Toggle Session rail, Terminal, Changes, Browser, and Files. View options includes Side chat and Background tasks. Selected tools have an active state. |
| Session rail | Narrow independently scrolling column adjacent to the conversation. Each card has a collapse control and a draggable reorder handle. |
| Environment | Shows host, workspace/folder, branch, Save, and Remote Control status. Folder and branch are menus. Remote Control was off and was not enabled during inspection. |
| Save environment | Opens a New environment form with name prefilled from the project, a folder list with Add/Remove and per-folder Worktree choice, an Environment variables disclosure, and Cancel/Create. Inspection cancelled without saving. |
| Context | Used percentage and used/window totals above a segmented bar. Rows for messages, MCP tools, system tools/prompt, skills, server instructions, memory, compaction buffer, free space, and deferred tools. Connector/skills/memory rows offer navigation; tool and memory counts expand separately. |
| Connectors | Collapsed header includes connection count. Expanding lists connectors. Selecting one opens its details, connection/tool count, and configuration source, with an All servers link. Opened a connector and closed its details without changing configuration. |
| Outputs | File rows open the reported file. Kept distinct from the repository-wide Changes summary. |
| Changes | Added/deleted totals open changes. Separate Commit changes button and repository options. No commit or branch action was performed in Claude. |
| Servers | Server name/address opens Browser; stop is a separate control. Existing servers were not stopped. |
| Background tasks rail | Running task rows show elapsed duration, open the task pane, and have a separate stop control. Finished count links to completed tasks. |
| Background tasks pane | Running and Finished groups, per-task stop, output/detail access, Clear finished, Expand, Close. Existing tasks/history were left intact. |
| Dock splits | Conversation and tools are separated by a horizontal divider; tool panes can stack vertically with another divider. Background tasks and Browser were visible simultaneously. |
| Pane placement | Right-clicking Expand opens Move left/right/top/bottom, Fill/Tile, and Open in new window. Moving Browser to the top preserved its size, changing the split from 33/67 to 67/33. Moving it back restored the original order. |
| Expansion | Fill replaces the conversation and other tool area while retaining left navigation. Hiding navigation fills the content width. Tile/contract restores the previous arrangement. Verified in the previous expansion task. |
| Browser chrome | Tabs, New tab, and pane actions share one compact header. Dev servers menu; Back/Forward/Reload; URL field; selection toggle. New tab focuses the URL field. |
| Browser keyboard | Left/Right move tab focus without selecting; Enter selects. Backspace closes the focused tab. The temporary reference tab was closed after inspection. |
| Browser options | Open externally, Copy URL, Save screenshot, Open HTML file, Viewport, Show dev server logs, file/link automation settings, allowed sites, cookie controls. Settings were inspected, not changed. |
| Annotation toolbar | Select element, Pen, Shapes, Text; active mode offers color, line width, Undo/Redo, Add to chat, and Close. |
| Annotation draft | Drew a temporary pen mark. Undo appeared and Add to chat became enabled. Add to chat attached `preview-annotation.png` to the draft without sending. Removed that temporary attachment immediately. |

No messages were sent in Claude, no jobs were stopped, no credentials were
imported, and no remote-control access was enabled. The Connectors card was left
expanded; the original dock order was restored and the temporary draft removed.

## Taucode data and implementation decisions

- Reuse the current workspace, engine/model, Git state, connector configuration,
  file preview, terminal, and settings actions.
- Engine token reporting varies. Display reported totals/categories and explicit
  unavailable states; do not fabricate Claude's system/tool/message breakdown.
- Configured/enabled connectors are not necessarily connected. Do not probe
  credentials or launch connectors automatically to populate the rail.
- Reported file outputs come from actual thread file references/changes. Do not
  relabel every workspace modification as a generated output.
- Existing PTYs only report shell lifecycle. A ready shell is not proof that a
  background command/server is running. Add a managed-process service with real
  lifecycle, output, stop, and ownership instead.
- Server addresses come from actual managed task output. A detected URL alone
  is not a verified health signal.
- Save environment profiles as metadata and references to existing workspaces,
  engine/model, and setup/launch configuration. Never store secrets in plaintext.
- Remote settings use taucode's real daemon configuration. Do not expose a fake
  Claude Remote Control switch or enable access during QA.
- Code opts into the new dock. Preserve default Chat/Work panel behavior.
- One Browser pane owns its tabs/NativeSlot. Moving or expanding panes must not
  remount the browser or change its page state.

## Parallel work and ownership

| Agent | Owned area | Integration contract |
| --- | --- | --- |
| `approval_review` (GPT 6.1 Sol) | CodeSessionRail, MainApp wiring, saved profiles and related AppSettings fields, FileTreePanel requested-file support, relevant tests | Consumes `useSidePanel().openTool/openPage`; supplies `sessionRail` to WorkspaceShell. Uses real Git/context/connectors and managed-task summaries. |
| `native_browser` (GPT 6.1 Sol) | SidePanel, panelState, useSidePanelState, side-panel.css, DockWorkspace and dock tests | Optional `dock?: boolean`, `sessionRail?: ReactNode` on WorkspaceShell. Persisted split tree; stable flat keyed pane DOM; independent fill/close/move and resizable splits. |
| `chat_polish` (GPT 6.1 Sol) | Managed background task backend, desktop/daemon routes, service wrapper, hook/pane and tests | `useManagedTasks(workspaceId, threadId)` and BackgroundTasks component; real status/timestamps/bounded output, explicit start/stop/restart, finished clearing, output URLs and open callback. |
| Root | Claude computer-use research/log, browser selection/annotation enhancements, review/integration, complete validation and native QA | Screenshot selection/pen/rectangle/text editor, guarded native/CDP capture and evaluation, PNG export, draft attachments, and native QA. |

## Saved implementation

- Code session rail: collapsible/reorderable Environment, Context, Connectors,
  Outputs, Changes, Servers, and Background tasks cards. Uses reported data and
  honest unavailable states. Saved environments reference existing folders and
  engine/model settings. File outputs open the existing preview.
- Code dock: independent tool panes, persisted split tree, pointer/keyboard
  dividers, local left/right/top/bottom moves, close/fill/tile, responsive single
  pane fallback. Browser DOM remains mounted through moves and expansion.
  Browser tabs and pane actions share a 32px header. Tab arrows/Home/End move
  focus, Enter selects, and Backspace/Delete closes with adjacent-tab focus.
  Closing a pane restores toolbar focus and stays closed after app restart,
  including older saved layouts with a stale active-tool entry. Tab/Shift+Tab
  dismiss menus and release the native browser overlay.
- Managed commands: shared desktop/daemon service with validated session cwd,
  owned process groups, real status/timestamps, bounded output, local URL
  detection, start/stop/restart/clear, and explicit shutdown on app/daemon exit.
  Terminal feature gates starts; cleanup remains available when disabled.
- Browser: screenshot-backed select/pen/rectangle/text, colors/line width,
  undo/redo, URL and selected-element descriptions, and PNG draft attachment.
  Add to chat restores conversation and never sends. URL/tab/document/viewport
  guards reject stale selection. Form values are excluded from descriptions.
  New blank tabs focus the address field and show an idle prompt. Annotation
  dismissal restores Annotate focus; Add to chat preserves composer focus.
  Clear-site confirmation is bound to its tab and URL, dismissed on navigation,
  and rechecked by the native backend before clearing data.
- PNG export uses a native Save dialog and validated atomic file writes.
  Backend capability flags gate unsupported controls. Native macOS and CDP
  screenshot/evaluation retain origin policy and tab/URL guards.
- OMP virtual dispatch is translated as tool/agent work, not file edits; stored
  rows containing only virtual paths are repaired without changing real edits.
- Saved-model restore now survives late model-list/default responses and still
  respects manual model changes. Real useModels integration tests cover this.
- Model-search Arrow Up reaches the last option. Disabled reasoning controls
  reject slider/reset changes, including delayed pointer commits. Starting a
  background command preserves a newer draft typed while the request completes.

## Latest validation

- Frontend: **1,689 passed**, 217 files, full suite exit 0 after the final pane
  persistence fix. `/tmp/taucode-code-ui-full-tests-final2.log`.
- WebKit: **5 passed** across four desktop sizes and a touch tablet. Checks cover
  dock geometry, movement, resizing, fill/tile, accessibility, DOM continuity,
  and the single-row browser header and action hit targets.
  `/tmp/taucode-code-ui-dock-final2.log`.
- Typecheck, targeted ESLint (zero errors), and app cargo check passed.
  Native clear-site target regression: **1 passed**.
  `/tmp/taucode-browser-clear-site-test.log` and
  `/tmp/taucode-browser-clear-site-check.log`.
  The full Rust suite was not rerun for this interaction pass.
- Final debug bundle build and signature verification passed:
  `/tmp/taucode-code-ui-build-final.log`.
- Native CUA verified blank-tab address focus, tab arrows/Enter/Backspace,
  adjacent-tab and pane-close focus, menu Tab dismissal, stale clear-site
  confirmation cancellation, annotation Escape/Add-to-chat focus, and model
  search Arrow Up. No site data was cleared and no message was sent.
- Final signed build verified Terminal remains closed after relaunch from the
  previously failing saved layout. Open/close focus and demo sign-in also pass.
  Browser fills to the sidebar, fills the window when navigation is hidden,
  and restores the tiled workspace with the signed-in page intact.
- No new native crash report appeared during this pass.

## Native issues found and fixed

- Dock main content applied the sidebar offset twice: fixed grid positioning.
- Move top moved a pane above the entire workspace: now moves within its sibling
  split and preserves the ratio; explicit drag supports cross-group moves.
- Adding panes repeatedly shrank earlier panes: now balances new shares.
- Stop required expanding a task: now visible on collapsed running rows.
- Add to chat left the full Browser obscuring the composer: now tiles first.
- Screenshot annotations assumed CSS dimensions: now use natural PNG dimensions.
- Hiding WKWebView changed its page viewport, making selection stale: native view
  now parks offscreen at unchanged size. Real native h1 selection and draft
  metadata passed after this change.
- An attempted accessibility-hidden setter changed the native page viewport and
  broke selection. It was reverted. Parking preserves dimensions; keyboard focus
  transfers to the main view only if the parked browser owned it. An owned native
  container now gates accessibility independently; see the follow-up below.
- Background command text received smart quotes: disabled correction,
  capitalization, and spelling on that field. Focused UI tests pass.
- Saved environment reset Astra to Sol after restore: declarative preference and
  stale-response protection fix the race. 29 targeted tests passed.

## Final native verification

- Restored the saved omp/Astra environment into a new draft; Astra remained
  selected after model loading. Removed the disposable `Taucode UI QA` profile.
- Historical failed worker calls render `spawn failed: worker +1` and
  `spawn failed: planner`, with real agent identifiers/links preserved.
- Signed into the local Plant Tracker demo, captured the page, selected its exact
  h1, and added PNG plus `Selected element: h1 Welcome back, Demo Gardener` to an
  unsent draft. Removed the test attachment and text afterward.
- Confirmed Save screenshot produces a valid PNG. Moved the test-owned export to
  `/tmp` after the macOS dialog used its previously remembered directory.
- Started a disposable server on port 47501 in Background tasks. Verified timer,
  output, reported URL, browser opening, stop, restart, and finished clearing.
- Quit taucode with its owned test server running: the 47501 listener disappeared;
  the user's existing demo server on 47400 (PID 12343) remained running.
- Ran a final short `printf` task, verified exit 0 and visible output. It is
  finished, with no remaining managed process. The finished row is visible in
  the final screenshot; history is local to this backend session.
- Moved Background tasks above Browser within the right column; expanded Browser
  to the left sidebar and across the whole window with navigation hidden; tiled
  it again. The signed-in demo page survived the moves and resizes.
- Removed a duplicate demo tab created during QA. No messages were sent, no
  permissions were changed, and the user's demo/preview servers were not stopped.

## Boundaries and remaining limitations

### Native accessibility follow-up

The browser now has an owned `NSView` accessibility container. It excludes the
parked page through both modern properties and legacy child queries, while the
original `WKWebView` keeps its class, observers, viewport, and title updates.
Replacing WebKit's runtime class after KVO enrollment caused a frame-setter
crash during development; that approach was removed. The container passes
pointer and accessibility hits outside the page through to the app and is
removed when its tab closes. Its accessibility frame matches the page when
exposed and is empty when parked, including legacy position/size queries; its
physical frame still preserves Wry's coordinate system.

The Code dock keeps panes directly under the workspace body. Expansion moves
the existing pane nodes past a keyed measurement anchor, refreshing WebKit's
native accessibility children without remounting the page. A flat structure
alone did not fix the stale toolbar after annotation, a second tab, and expansion.
Collapsed desktop and tablet navigation, including their resize handles, are
excluded from accessibility navigation.

Validation: 38 focused frontend tests, four desktop WebKit dock matrices, one
touch-tablet matrix, TypeScript checking, and the main-thread AppKit integration
test pass. The integration test shows no windows and covers an already-observed
native child, both coordinate orientations, viewport retention, legacy and modern
child/frame queries, pointer and screen-coordinate accessibility hit testing,
resize, window movement, and cleanup. Run it with
`cargo test --test native_browser_accessibility --offline` from `apps/taucode/src-tauri`.

Native testing reproduced the earlier crash in WebKit's observed frame setter;
the replacement container survived sign-in, annotations, tab changes, and pane
expansion. The regression sequence is: expand Browser, hide navigation, sign in,
annotate, select the h1, add to an unsent draft, create a second tab, then expand
again. Browser restore, tabs, address, and annotation controls remain in raw AX.
Annotation mode excludes the live page, keyboard navigation stays in its tools,
and closing it restores form values, click state, and scroll position. Test draft
text and attachments were removed without sending a message. The final signed
build passed this sequence again with corrected AX frames, plus inactive-tab
exclusion, tab-state retention, sidebar expansion/collapse, and composer input.
No crash report appeared after the initial experimental frame-setter crash.
The disposable port 47501 server was stopped; the 47400 demo and 1421 preview
were left running.

Relevant platform contracts: [AppKit accessibility children](https://developer.apple.com/documentation/AppKit/NSAccessibility-c.protocol/accessibilityChildren)
and [Apple's KVO implementation notes](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/KeyValueObserving/Articles/KVOImplementation.html).

### Feature boundaries

- This adapts the highlighted Claude workspace to taucode's existing features.
  Environment profiles reference existing folders, engine/model, and existing
  setup/launch scripts; they do not create worktrees or store environment secrets.
  Remote access opens the actual daemon settings. Claude-only simulator,
  keep-awake, and detached-window controls are not copied as inactive buttons.
- Context shows engine-reported usage. Some engines/sessions do not report enough
  data for Claude's system/tools/messages breakdown, and historical sessions may
  show `Usage not reported yet`. Configured connectors show Enabled, not an
  unverified live connection claim. Server URLs are reported addresses, not
  health checks. Background-task history lasts until the backend restarts.
- Native screenshot/evaluation features are available on macOS; unsupported
  native platforms hide these controls through backend capability flags. CDP
  also supplies guarded capture/evaluation. Desktop/daemon checks pass; native
  Windows behavior was not exercised on this Mac.
- VoiceOver was briefly enabled with user approval and turned back off. The
  automation could not reliably drive its navigation commands, so full spoken
  navigation remains unverified. Raw macOS accessibility and keyboard behavior
  were checked; do not claim complete VoiceOver parity with Claude.

All implementation agents and owned checks have finished. Main remains unmerged.
The unrelated `docs/HANDOFF-window-resize.md` and main checkout's `.claude/launch.json`
were preserved. User UI review and explicit approval are required before merging.

## Native QA handles and evidence

App path:
`/Users/rolandleyco/taucode-m1w/apps/taucode/src-tauri/target/debug/bundle/macos/taucode.app`.
Bind by full path because other worktrees share the bundle ID. CUA input uses
screenshot pixels on this Mac (the final capture is 3416x1994, not halved). Refresh full AX indices
after screenshots or app transitions; failed actions may require a fresh state.

Current native view: Plant Tracker verification conversation, Code mode, session
rail and stacked Background tasks/Browser panes. No running or finished managed
tasks, empty command field and message draft. Browser is
signed into the disposable demo `http://localhost:47400/`.

Final screenshots (kept outside Git because they contain local conversation UI):

- `/tmp/taucode-code-ui-final-workspace.jpg`: current signed build, compact browser header and clean tiled workspace.
- `/tmp/taucode-code-ui-final-expanded.jpg`: current Browser fills to the sidebar.
- `/tmp/taucode-code-ui-final-full-window.jpg`: current Browser fills the window with sidebar hidden.
- `/tmp/taucode-m8b-native-workspace-final.jpg`: session rail and stacked panes.
- `/tmp/taucode-m8b-native-expanded.jpg`: Browser fills the window with sidebar hidden.
- `/tmp/taucode-m8b-native-annotation-final.jpg`: exact native h1 selection.
- `/tmp/taucode-m8b-native-export.png`: exported PNG, 888x1678.
- `/tmp/taucode-native-ax-final-workspace.jpg`: final signed build, clean draft and tiled workspace.
- `/tmp/taucode-native-ax-final-expanded.jpg`: full-window browser after the accessibility fixes.
- `/tmp/taucode-native-ax-final-expanded.txt`: matching native accessibility tree with toolbar and live page.
- `/tmp/taucode-native-ax-frame-test.log`: main-thread AppKit regression test.
- `/tmp/taucode-native-ax-build6.log`: final signed app build.

CUA root bindings: `tau` (native app), `preview` (IAB tab 5), `claude` (reference),
`panelFiles` (fs promises for screenshot saving). After context compaction call
`cua.rewriteDocumentation()` before continuing. Root owns desktop interactions.
