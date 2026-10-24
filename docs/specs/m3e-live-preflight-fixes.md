# M3e: fix background pre-flight failures found in live tests

## Goal

A live suite on the user's Mac, with the signed M3d build, failed **before step 1** on every
positive task. Each run died in a pre-flight or invariant check, not in planning or grounding.
After this milestone, those checks tolerate normal background conditions: target not
frontmost, the user actively using another app, the window on another Space, slow first AX
reads. Every safety property is kept and fails closed.

| Task (target launched with `open -g`) | Error |
|---|---|
| TextEdit, doc open; type a phrase | `Focus changed while planning; action cancelled` (runner.rs ~395), 0 steps, 6 s |
| Finder window on a folder; new folder | same error, 14 s |
| Notes (launched in background); new note | `Target has no on-screen window` (background/windows.rs `select`), 1 s |
| System Settings (read-only), General → About | `System Settings read-only access: cannot inspect current pane` (background/ax_actions.rs `pane_info`), 1 s |

During these runs the user was moving the mouse in another app, and once a system
notification (`com.apple.UserNotificationCenter`) was frontmost.

## Root causes (verify each in code before fixing)

1. **Spurious "focus changed"**
   - `runner.rs` compares the whole `Context` (`app` and `secure`) from before planning with
     a fresh one after.
   - In background mode, `Session::context()` sets `secure` from `ax_actions::secure(pid)`.
     That fails closed (`true`) whenever the focused-element read returns nothing or an empty
     role, including on a 50 ms AX messaging timeout.
   - So one flaky read between the two samples flips `secure` and cancels the run.
2. **Window selection requires `is_on_screen()`**
   - A window on another Space, or not yet ordered in after a background launch, is
     rejected.
   - ScreenCaptureKit's desktop-independent-window capture works for windows that aren't
     on screen. Minimized windows can't be captured.
3. **`pane_info(pid, None)` uses only `AXFocusedWindow`**
   - A non-active app often returns no focused window, and `Session::capture` /
     `screenshot` call `check_pane(..., None)` *before* capture.
4. **`verify_typing` (runner.rs)**
   - It compares `policy_target(Hotkey{})` (the pid's *focused* element) with the policy
     target of the Type action (the *grounded* element).
   - When the agent types into a grounded element that isn't focused (the normal AX
     set-value / selected-text path in the background), the two differ and typing is
     always cancelled. This bug is latent: the run never reached it.

## Required

1. **Invariant check:** compare only `app` (target binding) between planning and acting.
   - Keep `secure` fail-closed, but recompute it at decision time (already OR'd into
     `policy_context`), so it can only *add* a confirmation, never cancel a run.
   - Keep `Session::check()` (process identity) as is.
2. **AX metadata reads** (roles, subroles, focused element/window, titles; *not* actions):
   - on `kAXErrorCannotComplete` / timeout, retry once with a longer per-element messaging
     timeout (≈250 ms), then fail closed as today
   - errors include the AX error code
3. **Window selection:**
   - prefer the front-most on-screen layer-0 window of the pid
   - otherwise the front-most non-minimized layer-0 window that SCK lists for the pid
   - minimized-only → error `Target window is minimized`
   - none → `Target has no window` with counts (listed / on-screen / minimized) for
     diagnosis
   - the overlay/HUD must cope with an off-screen target: no cursor animation; the HUD
     still shows
4. **Pane inspection root:**
   - with a window id → that window
   - without one → `AXFocusedWindow`, then `AXMainWindow`, then the first `AXWindows` entry
   - in `capture`/`screenshot`, check the pane against the captured window (the id-based
     check). Drop the pre-capture `None` check unless it adds safety the post-capture check
     lacks.
   - The pane denylist and fail-closed behaviour when no window can be inspected stay
     unchanged.
5. **Typing verification:**
   - when the Type action targets a grounded element, re-resolve **that element** and
     compare its policy metadata (role, subrole/secure, label) before input
   - compare the focused element only when typing goes to the focused element (no
     grounded target)
   - a secure or sensitive element still confirms
6. **Diagnostics:** every pre-flight error that ends a run names the check and the observed
   values (e.g. `app before=X after=Y`) without including field values or typed text.

## Tests

Unit tests with the existing fakes:

- a flipping `secure` doesn't cancel, but still forces Confirm on Type
- app change cancels
- window fallback order (on-screen > off-screen > minimized error)
- pane root fallback chain and fail-closed
- grounded-element typing verification: same element passes; changed role/label/secure
  cancels or confirms; focused-element path unchanged
- AX retry-once logic (fake provider returning CannotComplete then success; and twice →
  fail closed)

## Files

`apps/taucode/src-tauri/src/computer/{runner.rs,mod.rs,tests.rs}` and
`apps/taucode/src-tauri/src/computer/background/*`. Nothing else.

## Constraints and verification

- Offline; no new crates; always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`. The known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3/M3d rules: approvals UI-only, kill
  switch, Terminal denied, System Settings read-only plus pane denylist, secure-field
  confirms.

## Out of scope

Planner and grounding prompts, the settings UI, pi/codex engines, the browser.
