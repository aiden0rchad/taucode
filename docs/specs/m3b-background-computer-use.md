# M3b: background computer use (own virtual cursor, never take over the user's mouse)

## Goal

Today taucode's computer use posts **global** CGEvents: it moves the user's real cursor,
types into whatever has focus, and brings target apps to the front. The user can't use their
Mac during a run. Make **background mode** the default:

- the agent's clicks, scrolls and keystrokes go to the target app's process **without moving
  the user's cursor, without changing the frontmost app, and without stealing keyboard
  focus**
- screenshots capture the **target app's windows** even when they're behind other windows
- a **virtual cursor** overlay shows where the agent is acting (its own pointer, a short glide,
  a click ripple), excluded from screen capture
- the user keeps working normally during a run

Keep foreground mode (today's behaviour) as an explicit fallback.

## Reference implementation to port (MIT): trycua/cua `cua-driver`

At `/Users/rolandleyco/taucode-refs/cua` (commit `ac7a167431329ae04a09dbb7332360cb4e5f4d61`),
Rust, in `libs/cua-driver/rust/crates/platform-macos/src/`:

- `input/skylight.rs`: SkyLight SPI bridge. `SLEventPostToPid` resolved via dlopen/dlsym,
  `SLEventSetAuthenticationMessage` for keyboard on macOS 14+ (Chromium-like targets),
  `CGEventSetWindowLocation`, `SLEventSetIntegerValueField`, `CGSMainConnectionID`. Falls back
  to the public `CGEvent::post_to_pid` when symbols are missing.
- `input/mouse.rs`, `input/keyboard.rs`: background mouse/keyboard synthesis and the post
  routing.
- `input/ax_actions.rs`: Accessibility actions (AXPress etc.).
- `focus_guard.rs`, `key_focus_guard.rs`, `focus_steal.rs`: keep the user's frontmost app and
  key focus when a target app tries to activate.
- `capture.rs`, `windows.rs`: per-app/per-window capture and window lists.
- The `cursor-overlay` crate (`lib.rs`, `motion.rs`, `bezier.rs`, `capture_exclusion.rs`,
  `z_order.rs`): the virtual cursor overlay and how it stays out of captures.
- Docs: `docs/content/docs/cua-driver/reference/mcp-tools/pointer-cursor-and-browser.mdx`,
  and the blog post `blog/inside-macos-window-internals.md`.

**Port, don't depend.** `cua-driver-core` is about 94k lines, and the crates pin objc2 0.5 /
core-graphics 0.24 while taucode uses objc2 0.6 / core-graphics 0.25.
- Copy only the logic needed into `apps/taucode/src-tauri/src/computer/background/`, adapted
  to taucode's existing dependencies (objc2 0.6, objc2-app-kit, core-graphics 0.25,
  screencapturekit 11, libc).
- Keep the MIT copyright header in each ported file. Add
  `apps/taucode/src-tauri/src/computer/background/NOTICE` with the upstream URL, commit and
  license.
- **Add no new crates.**

## Behaviour

1. **Target binding.** A run binds to one allowed app (bundle id → running pid; launch it
   without activating if it isn't running: `NSWorkspace` open with `activates = false`). The
   policy's "frontmost must be allowed" rule no longer applies in background mode; the
   **target pid** must be allowed and not denylisted.
2. **Capture.** Capture only the target app's on-screen windows, at their own size, even when
   occluded. Use ScreenCaptureKit window filters (`SCContentFilter` with the app's windows;
   check the current `screencapturekit` crate API).
   - The planner and grounding see that window image (downscaled per the pixel-grounding
     rules).
   - Map grounding coordinates → window-local points → global points for the event location.
   - If the app has several windows, use the frontmost window of that app (by its own window
     order) and report which.
3. **Action execution order** (a per-action decision function, unit-tested):
   1. **AX first.** Find the element at the point inside the target app
      (`AXUIElementCopyElementAtPosition` on the app's AX element).
      - Single left click on an element supporting `AXPress` → `AXUIElementPerformAction(AXPress)`.
      - `type` into a text field with a settable `AXValue`, and `overwrite` → set the value;
        otherwise insert via `AXSelectedText` when supported.
   2. **Background events** via the SkyLight bridge to the target pid: mouse down/up with
      window location set, scroll, drag, key events (unicode typing, hotkeys) with
      authentication as in the reference. The user's cursor must not move.
      - **Never** call `CGWarpMouseCursorPosition`, never post to the HID tap, never activate
        the target in this path.
   3. **Foreground fallback** (today's global events) only if the setting
      `allowForegroundFallback` is on (default **off**). Before using it, request an approval
      ("This app ignored background input; take over the mouse for this step?"), and record it
      in the run log.
4. **Focus guard.** While a run is active, if the target app (or any process because of our
   events) becomes frontmost or takes key focus, restore the user's previous frontmost app
   and key window immediately (port the reference guard). Record each restoration in the run
   log.
5. **Virtual cursor overlay.**
   - A click-through, non-activating, transparent always-on-top panel per display, drawing the
     agent's pointer at the action point: an eased glide from the last point, a ripple on
     click, a subtle label "taucode".
   - Excluded from capture (window sharing type none, and excluded in the SC filter).
   - Hidden when no run is active. Toggle in settings.
6. **HUD and kill switch:** unchanged. Stop must also hide the overlay and release any
   synthesized held keys/buttons in the target process.
7. **Settings → Computer Use:**
   - mode: "Background (recommended): you can keep using your Mac" / "Foreground: takes over
     the mouse"
   - `allowForegroundFallback`; virtual cursor on/off
8. **Telemetry in the run log:** per step, which path executed the action (`ax`, `skylight`,
   `postToPid`, `foreground`), plus focus-guard restorations.

## Tests

- Execution-path decision table (AX available / not, click vs type vs scroll vs hotkey,
  fallback on/off), using fakes for AX and posting.
- Coordinate mapping grounding → window → global, with Retina scale and multi-display offsets.
- Focus guard state machine with a fake workspace.
- Policy: target-pid allowlist replaces the frontmost rule in background mode;
  foreground-fallback approval required.
- Overlay geometry and capture-exclusion flags (no real windows in tests).
- The SkyLight symbol-resolution fallback path (simulate missing symbols).
- Existing computer tests still pass. No real input or capture in tests.

## Constraints

Offline; no new crates; always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. Don't run `tauri build`. Keep all
M3 safety rules (no eval of model output, the sidecar never executes input, approvals UI-only,
the denylist).

## Verification

Same command as `m3-computer-use.md`.
