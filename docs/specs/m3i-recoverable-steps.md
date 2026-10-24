# M3i: recoverable step errors, lenient grounding parse, partial snapshots, focus restore on all paths

## Goal

Live suite after M3h:

- **TextEdit passes.** **The System Settings pane denylist passes.**
- **Notes** now creates the note: ⌘N → `ax-menu` "File > New Note", then `ax-element`
  typing. It then fails, and Notes became frontmost.
- **Finder** and **Settings → About** fail on single bad steps that should have been
  recoverable.

Evidence (stored runs in `~/Library/Application Support/dev.taucode.desktop/computer/runs`):

1. **Notes, step 3:** `Pre-flight capture: app=com.apple.Notes, AXActions: ax_error(-25204); walk_attempts=3`.
   - A big tree (element cap reached). One node's `AXActions` read timed out, and the M3h
     walk-restart rule turned that into a failed snapshot, three times.
   - A per-node timeout must not fail the snapshot.
2. **Notes became frontmost** after the AXPress on "File > New Note". The app activates
   itself on new-note. `focusRestorations` is `[]`; the M3f restore only covers the
   pixel/key-event paths.
3. **Settings → About:** the grounding model replied `**42**` (markdown bold), then
   `Expected one element index or none`, and **the run failed**.
4. **Finder:**
   - ⌘⇧N gave `menuResolution {reason: "disabled", candidates: ["File > New Folder"]}`.
     Finder disables that item while it isn't the active app.
   - The hotkey then fell to the Confirm path and timed out after 300 s.
   - In the background this can't work; the planner should get the reason and choose
     another route, e.g. the toolbar **Action** (gear) `AXMenuButton` → "New Folder",
     which is validated per window.
5. Policy **Deny** for one action (e.g. read-only Settings clicking the search field)
   ends the whole run, though M3d intended the reason to let the planner adapt.

## Required

1. **Recoverable step errors**
   - These don't end the run:
     - policy `Deny`
     - unparseable grounding (after the parse fix below)
     - a hotkey whose menu match is `disabled`, or unresolved in background mode where
       key events can't be delivered
     - AX action unsupported with no viable fallback
     - stale-observation retries exhausted for a step
   - Instead they record the step with `recovered: true` and the reason. The next
     planner observation starts with
     `Previous action was NOT executed: <reason>. Choose a different approach.`
   - Caps: 3 consecutive or 6 total recoverable errors per run, then fail with the last
     reason.
   - Denied actions are never executed. Approval timeouts / rejections, the kill switch,
     target binding changes and pane-inspection failures stay **fatal**.
2. **Background hotkey with a disabled or unresolved menu item:** don't Confirm and don't
   send key events. Make it a recoverable error: the reason names the disabled item and
   suggests window-level controls (toolbar buttons, the Action/gear menu, context menu).
   Safe hotkeys that the pixel path can deliver keep today's behaviour.
3. **Lenient grounding parse:** accept a reply whose only integer token, after stripping
   markdown (`*`, `` ` ``, `_`), whitespace and trailing punctuation, is a valid index:
   `**42**`, `` `42` ``, `42.`, `Index: 42`, `Element 42`. `none` in any case or
   formatting is none. Multiple integers or an out-of-range index → pixel fallback (as
   for none), with `model_result: "unparseable"` recorded. Never fail the run. Apply this
   to both the Rust side and the Python sidecar, wherever parsing happens.
4. **Partial snapshots**
   - In the element snapshot walk, a per-node `kAXErrorCannotComplete` / timeout or
     `kAXErrorInvalidUIElement` on a **non-root** node skips that attribute or subtree.
   - Count skips in the snapshot status (`partial: n`); don't restart.
   - Restart (as in M3h) only when the window root itself is invalid.
   - The **pane-inspection** walk for System Settings keeps restart + fail-closed.
5. **Open menus in the snapshot:** after pressing an `AXMenuButton` / `AXPopUpButton`, or
   `AXShowMenu`, include the pid's currently open `AXMenu` elements (from the app
   element's children and from the pressed element) in the next snapshot as elements, so
   their items can be grounded and AXPressed. Policy treats them as menu-item clicks
   (risky words → Confirm; read-only rules apply; context menus count as non-View/Go
   menus).
6. **Focus restore on every path:**
   - after any action (`ax-element`, `ax-select`, `ax-menu`, pixel), if the target app
     became frontmost and wasn't frontmost before the action, re-activate the app that
     was frontmost before the action (no raise of the target)
   - record it in `focusRestorations`
   - also check once ~300 ms later, since apps often activate asynchronously
7. **Planner prompt:** tell the planner that a "NOT executed" observation means the
   action was blocked or impossible in the background, and to try a different route.

## Tests

- recoverable vs fatal classification table; caps; the observation prefix reaching the
  planner
- disabled-menu hotkey → recoverable with no key events sent
- grounding parse table (Rust + Python)
- partial snapshot skips without restart; root-invalid restart; pane walk still fail-closed
- open-menu inclusion and policy
- focus restore after an `ax-menu` action that activates the target (immediate and delayed)

## Files

`apps/taucode/src-tauri/src/computer/**`, `apps/taucode/engines/computer/taucode_cua/**`
and its tests. Nothing else.

## Constraints and verification

- No new crates or Python deps. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3–M3h rules: approvals UI-only, kill
  switch, Terminal denied, System Settings read-only with pane denylist, secure fields,
  risky hotkeys, never activate or raise the target.

## Out of scope

Everything outside `computer/` and the sidecar.
