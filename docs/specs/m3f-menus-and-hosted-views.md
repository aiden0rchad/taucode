# M3f: hotkeys via menu items, hosted-view membership, launch races

## Goal

The live suite after M3e: **TextEdit passes** (AX type in 13 ms, fully background). The
others fail as follows. Stored run records live in
`~/Library/Application Support/dev.taucode.desktop/computer/runs/*.json`.

| Task | Failure |
|---|---|
| Notes: new note | Planner sends hotkey ⌘N. Element-first can't handle it, so it falls to `pixel-skylight` (`executionFallbackReason: action_requires_pixel_events`) and fails `Target window no longer has the app's keyboard focus`. **Notes also became the frontmost app** during the run, with the cursor unchanged. That breaks the background guarantee. |
| Finder: new folder named X | Hotkey ⌘⇧N isn't in the safe list, so it confirms and times out after 300 s. Naming the folder also needs Return, which always confirms. |
| System Settings → General → About | Element grounding picked index 150 (snapshot complete, 86 usable). The binding check failed: `AX element binding check: pid=…, window=…, snapshot=1, index=150, belongs_to=false`. System Settings' sidebar and panes are SwiftUI / remote-view-hosted, so the AXParent / AXWindow chain from an element doesn't reliably reach the window element, though the snapshot walk reached the element from that window. |
| System Settings, just relaunched | `Target has no window; listed=0, on-screen=0, minimized=0` 3 s after `open -g`: a launch race. |

When done, all of these can work in the background, with no frontmost or cursor change and
no weaker policy.

## Required

1. **Hotkey → menu item (element-first keyboard shortcuts)**
   - Before executing a `hotkey` in background mode, resolve it against the target app's
     menu bar over AX:
     - read the app element's `AXMenuBar`
     - walk its menus within a time/element budget
     - match `AXMenuItemCmdChar` + `AXMenuItemCmdModifiers` (and `AXMenuItemCmdVirtualKey`
       for non-character keys) against the requested keys
     - use only enabled items
   - If one enabled item matches, execute by `AXPress` on that item without activating the
     app. Record `path: "ax-menu"` and the item title in the step.
   - **Policy:** a resolved hotkey is judged as a click on an `AXMenuItem` whose label is
     the menu item title plus its menu's title:
     - risky words → Confirm
     - read-only apps → still **denied** (hotkeys stay denied in read-only, unchanged)
     - otherwise Allow under `sensitive`
   - The existing risky-hotkey list (⌘Q, ⌘W, ⌘⌫ …) **always** confirms first, even when it
     resolves to a menu item.
   - Unresolved hotkeys keep today's behaviour: safe list, else Confirm, then the key-event
     path.
   - Planner observation: list the target's top-level menus and a compact set of enabled
     menu items with shortcuts (cap ~40), so the planner can also choose to click a menu
     item directly.
2. **Never steal the foreground**
   - Find what made Notes frontmost on the pixel/key-event path (activate-without-raise?
     the focus guard? SkyLight event posting?) and fix it.
   - The target must never become the frontmost app in background mode. If an injection
     path can't deliver without activation, fail that action with a clear
     `executionFallbackReason` and leave focus alone.
   - Add a post-action invariant: if the frontmost app changed to the target during an
     action, restore the previous frontmost app immediately (the existing focus-guard
     mechanism) and record it in `focusRestorations`.
3. **`confirmTyping: "risky-only"`** (opt-in; default stays `sensitive`)
   - Same as `sensitive`, plus Return/Enter (`enter: true`, or a Return key) is allowed in
     non-secure, non-sensitive AXTextField / AXTextArea / AXComboBox targets of full-access
     apps.
   - Secure fields, sensitive labels, risky hotkeys and risky clicks still confirm.
   - Settings UI: a third option with a warning that Return can submit or send in some
     apps.
   - Add decision-table tests.
4. **Hosted-view membership**
   - An element belongs to the target window if it was reached by the snapshot walk from
     that window's root in the **current** snapshot (the snapshot id still valid, same pid
     binding, element not invalidated: still answers `AXRole`).
   - The ancestor-chain check is a secondary signal, not a requirement.
   - Keep rejecting stale snapshot ids, other windows' snapshots, and dead elements.
   - Error text names which sub-check failed.
5. **Launch race:** when window selection finds `listed=0` for a running target, retry
   for up to 3 s (poll ~200 ms) before failing.
6. **MCP result:** `computer_run_task`'s JSON `computerRun.steps` is currently `[]` even
   when steps ran (the stored record has them). Include compact steps: no thumbnails, no
   typed text beyond what the planner already supplied, truncated planText.

## Tests

- menu resolution against a fake AX menu tree: char + modifiers, virtual keys, disabled
  items, ambiguous matches → unresolved
- policy for resolved hotkeys: risky title → Confirm; risky-list hotkey precedence;
  read-only deny; sensitive Allow
- frontmost-restore invariant with the fake env
- `risky-only` decision table
- membership: walked element accepted with a broken parent chain; stale snapshot, other
  window or dead element rejected
- window-launch retry
- MCP steps serialization

## Files

`apps/taucode/src-tauri/src/computer/**`, `apps/taucode/src-tauri/src/types.rs`
(confirmTyping enum), `apps/taucode/src/features/settings/components/sections/SettingsComputerSection.tsx`
and its tests. The planner sidecar's observation text only if needed for menus. Nothing
else.

## Constraints and verification

- Port from `/Users/rolandleyco/taucode-refs/cua` (MIT, `libs/cua-driver/rust/crates/platform-macos`)
  if it has menu-bar AX code (search `AXMenuBar`, `show_menu`, `menu`). Keep MIT headers;
  extend NOTICE.
- No new crates. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3/M3d/M3e rules: approvals UI-only, kill
  switch, Terminal denied, System Settings read-only with pane denylist, secure-field
  confirms, app-only invariant.

## Out of scope

Engines, browser, providers.
