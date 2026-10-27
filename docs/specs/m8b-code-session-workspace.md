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
| Browser chrome | Tabs and New tab; Dev servers menu; Back/Forward/Reload; URL field; selection toggle. |
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
- Managed commands: shared desktop/daemon service with validated session cwd,
  owned process groups, real status/timestamps, bounded output, local URL
  detection, start/stop/restart/clear, and explicit shutdown on app/daemon exit.
  Terminal feature gates starts; cleanup remains available when disabled.
- Browser: screenshot-backed select/pen/rectangle/text, colors/line width,
  undo/redo, URL and selected-element descriptions, and PNG draft attachment.
  Add to chat restores conversation and never sends. URL/tab/document/viewport
  guards reject stale selection. Form values are excluded from descriptions.
- PNG export uses a native Save dialog and validated atomic file writes.
  Backend capability flags gate unsupported controls. Native macOS and CDP
  screenshot/evaluation retain origin policy and tab/URL guards.
- OMP virtual dispatch is translated as tool/agent work, not file edits; stored
  rows containing only virtual paths are repaired without changing real edits.
- Saved-model restore now survives late model-list/default responses and still
  respects manual model changes. Real useModels integration tests cover this.

## Latest validation

- Frontend: **1,664 passed**, 216 files, full suite exit 0.
  `/tmp/taucode-m8b-frontend-final2.log`.
- Rust library: **520 passed, 1 ignored**, zero failures, exit 0.
  `/tmp/taucode-m8b-rust-final.log`.
- WebKit layout: **74 passed**, exit 0, including dock movement, resizing,
  expansion, browser DOM continuity, desktop/tablet/phone layouts.
  `/tmp/taucode-m8b-webkit-final.log`; screenshots in
  `/tmp/taucode-m8b-webkit-final`.
- Typecheck and focused lint passed. App/daemon cargo checks passed.
- Latest signed debug bundle build passed:
  `/tmp/taucode-m8b-native-final2.log`.
- Native CUA: demo sign-in, annotation drawing/undo, element selection/highlight,
  draft image and selected-element text, draft removal, screenshot Save dialog,
  task start/timer/output/server opening/stop/restart/clear verified.
- Full-width browser fills to the sidebar; hiding navigation fills the window.
  Earlier native checks covered sidebar drag, window zoom/restore and preserved
  forms. Current WebKit suite covers the new dock layouts.

Final edits additionally passed 62 message tests and 15 browser tests, global
TypeScript checking, targeted ESLint, diff checks, and a signed native build.
Native selection, draft attachment, saved model restoration, pane placement,
full-window expansion, and owned-task quit cleanup passed. Full-suite results
above precede the final label/error-message refinements; those refinements were
covered by the focused checks rather than another full-suite run.

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
  transfers to the main view only if the parked browser owned it. Final native
  h1 selection and draft metadata pass. Accessibility traversal remains a known
  limitation described below.
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
- A parked WKWebView still appears in raw accessibility inspection. Setting
  AppKit accessibility-hidden caused a viewport regression and was reverted.
  WebKit's [accessibility implementation](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebKit/UIProcess/mac/WebViewImpl.mm)
  returns its remote web-content child even when the view is parked. No public
  independent subtree suppression was verified. VoiceOver behavior has not been
  validated; do not claim complete accessibility parity with Claude.
- During native full-pane expansion, raw accessibility inspection occasionally
  omitted toolbar nodes even though the visible restore control worked with
  pointer input. This remains part of the native accessibility follow-up.

All implementation agents and owned checks have finished. Main remains unmerged.
The unrelated `docs/HANDOFF-window-resize.md` and main checkout's `.claude/launch.json`
were preserved. User UI review and explicit approval are required before merging.

## Native QA handles and evidence

App path:
`/Users/rolandleyco/taucode-m1w/apps/taucode/src-tauri/target/debug/bundle/macos/taucode.app`.
Bind by full path because other worktrees share the bundle ID. CUA input uses
screenshot pixels on this Mac (3456x2034 is not halved). Refresh full AX indices
after screenshots or app transitions; failed actions may require a fresh state.

Current native view: Plant Tracker verification conversation, Code mode, session
rail and stacked Background tasks/Browser panes. One finished disposable task,
no running managed tasks, empty command field and message draft. Browser is
signed into the disposable demo `http://localhost:47400/`.

Final screenshots (kept outside Git because they contain local conversation UI):

- `/tmp/taucode-m8b-native-workspace-final.jpg`: session rail and stacked panes.
- `/tmp/taucode-m8b-native-expanded.jpg`: Browser fills the window with sidebar hidden.
- `/tmp/taucode-m8b-native-annotation-final.jpg`: exact native h1 selection.
- `/tmp/taucode-m8b-native-export.png`: exported PNG, 888x1678.

CUA root bindings: `tau` (native app), `preview` (IAB tab 5), `claude` (reference),
`panelFiles` (fs promises for screenshot saving). After context compaction call
`cua.rewriteDocumentation()` before continuing. Root owns desktop interactions.
