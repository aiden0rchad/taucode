# M3g: row selection, menu items as elements, same-app focus, windowless launch

## Goal

The live suite after M3f:

- **TextEdit passes.**
- **System Settings** clicks run as `ax-element` but don't navigate.
- **Finder** can't create a folder.
- **Notes** has no window right after launch.

Evidence from the stored runs (`~/Library/Application Support/dev.taucode.desktop/computer/runs`):

1. **System Settings sidebar:**
   - Grounding picked "General" (an element inside a sidebar `AXOutline` row). `AXPress` ran
     without error twice, but the pane stayed "Displays".
   - The SwiftUI sidebar only navigates when the **row is selected** (`AXSelected = true` on
     the row, or `AXSelectedRows` on the outline). AXPress on a static text/cell inside the
     row is a no-op.
2. **Finder:**
   - The hotkey ⌘⇧N wasn't resolved to `File > New Folder` (no reason recorded), while ⌘⇧G
     did resolve to `Go > Go to Folder…` (`path: ax-menu`). So unresolved hotkeys fell to
     `pixel-skylight` key events, which background Finder ignores.
   - The planner never clicked a menu item directly: menu items aren't in the element list
     grounding can choose from.
   - After the Go-to-Folder panel opened, the elementless hotkey Escape failed with
     `AX policy target left the captured window`. The app's focused element was in another
     window **of the same pid** (the panel).
3. **Notes**, launched seconds earlier with `open -g`:
   - `Target has no window; listed=14, on-screen=0, minimized=0, unknown=14`; AXWindows
     count 0. The M3f launch retry only covers `listed=0`.
   - A running app with no AX window often needs a reopen
     (`kAEReopenApplication`, which NSWorkspace sends when opening an already-running app)
     to create its main window.

## Required

1. **Row selection (`ax-select`)**
   - When the grounded element is an `AXRow` / `AXOutlineRow`, or a descendant (≤ 4 levels)
     of a row inside an `AXOutline` / `AXTable` / `AXList`, execute a left single click by
     selecting that row: `AXSelected = true` on the row, else set the container's
     `AXSelectedRows`.
   - Then re-read the selection to verify. Use AXPress only if the element is itself an
     `AXButton` / `AXLink` / `AXMenuItem`-like pressable, or if selection is unsupported.
   - Record `path: "ax-select"`.
   - **Policy:**
     - row selection is a navigation click (allowed in read-only)
     - the pane denylist is checked against the **row's** label before selecting
       (System Settings: selecting "Privacy & Security" is denied)
     - after selection, re-check the pane (existing post-action checks)
2. **Menu items as elements**
   - Append the target app's enabled menu-bar items (from the M3f menu walk) to the element
     list given to grounding and the planner, as `AXMenuItem` with label `Menu > Item`, the
     shortcut, and an `enabled` flag. Cap ~60, interactive first; same snapshot id.
   - A grounded menu item executes by AXPress without activation (`path: "ax-menu"`).
   - **Policy:**
     - judged as a click on that menu item (risky words → Confirm)
     - in **read-only** apps only items under the `View`, `Go`, `Window` and `Help` menus
       are allowed; others are denied with a reason naming read-only
     - the System Settings pane denylist applies to item titles
3. **Menu resolution diagnostics:** when a hotkey isn't resolved, record why in the step
   (`menubar_unavailable` | `no_match` | `disabled` | `ambiguous` | `timeout`), plus the
   candidate item titles for `disabled` / `ambiguous`. Verify by a test that ⌘⇧N matches
   `AXMenuItemCmdChar "N"` with `AXMenuItemCmdModifiers = 1` (shift), case-insensitively.
   (Also check the walk isn't cut off before the File menu by the element/time budget.)
4. **Same-app focus**
   - For elementless keyboard actions (hotkey / hold_and_press / type without a target),
     the policy target may be the focused element in **any window owned by the target
     pid**: sheets, panels, popovers.
   - Evaluate policy on that element, plus the System Settings pane check for its window.
   - Cancel only if the focused element belongs to another pid or can't be read (fail
     closed as today).
   - Element-targeted actions keep the captured-window membership rule.
5. **Windowless launch**
   - Treat "no usable window" (none on-screen and none matched to an AX window) like
     `listed=0`.
   - If the app has zero AX windows, send one reopen without activation
     (`NSWorkspace openApplicationAtURL` with `activates = false`, or the AE equivalent),
     then poll up to 5 s (~200 ms) for a usable window.
   - Never activate or raise.
6. **Planner prompt:** tell the planner that, in background mode, menu items listed in the
   elements can be clicked directly and are more reliable than keyboard shortcuts, and
   that sidebar/list navigation means clicking the row.

## Tests

- row selection routing (row, descendant of row, button inside row → AXPress), the
  selection verification failure path, read-only allowance, and pane denylist on the row
  label
- menu items as elements: policy by menu for read-only, risky titles, AXPress routing,
  the cap
- each menu-resolution diagnostic reason; the ⌘⇧N shift case
- same-pid sheet focus allowed, other-pid focus cancelled, unreadable focus fail-closed
- windowless launch: reopen once, poll, then fail
- sidecar/planner prompt tests if the prompt changes

## Files

`apps/taucode/src-tauri/src/computer/**`; the planner/grounding sidecar
`apps/taucode/engines/computer/taucode_cua/**` and its tests, only for element-list/prompt
changes. Nothing else.

## Constraints and verification

- No new crates or Python deps. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of M3–M3f rules (approvals UI-only, kill switch,
  Terminal denied, System Settings read-only with pane denylist, secure fields, risky
  hotkeys, never activate or raise the target).

## Out of scope

Engines, browser, providers, settings UI.
