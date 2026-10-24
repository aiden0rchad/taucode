# M3k: make activation restore actually work (cooperative activation, taucode frontmost)

## Goal

When a target app activates itself during a background run (Notes does this on
File › New Note), the user's previous frontmost app gets activation back within ~300 ms
and is verified. That includes when the previous app is **taucode itself**, the common
case, since the user types the instruction in taucode.

## Evidence (live, after M3j)

- The user's frontmost app was taucode (`codex-monitor`, the host).
- Step 1, AXPress on "File > New Note" → Notes activated itself (frontmost → Notes at
  4.4 s).
- The step recorded `keyFocus` then `keyFocusUnresolved`. A SkyLight probe shows Notes
  **stayed frontmost for the rest of the run** (> 200 s): nothing restored it.
- Restore is `NSRunningApplication::activateWithOptions(0)` on the previous pid
  (`focus_guard.rs` `Workspace::activate`, ~line 382) plus the SkyLight focus-record
  path (`restore_key`).
- Under macOS 14+ cooperative activation, a non-active app can't pull activation that
  way; the call returns without effect or is refused.
- For the host pid, the guard tries to activate taucode through its own
  `NSRunningApplication` from a background thread.
- M3j's noise fix itself works: other steps now record 0 restorations, where before they
  recorded many.

## Required

1. **Activation transfer from the app that took it**
   - To restore pid P after target T (or another taucode-attributable app) activated, use
     `-[NSRunningApplication activateFromApplication:options:]` (macOS 14+) on P's running
     application, with `from` = T's running application (the currently active app).
   - Feature-detect with `respondsToSelector`; fall back to `activateWithOptions` on older
     systems.
2. **Host as previous:** when P is the host (taucode), activate on the **main thread**:
   - dispatch to the main queue
   - use `NSApplication.sharedApplication().activate()` (macOS 14+), falling back to
     `activateIgnoringOtherApps(true)`
   - then make taucode's previous key window key again without changing its order (keep
     what the user had)
   - must not crash tao: use `NSApp` APIs only; no reparenting
3. **Verify and escalate**
   - After a restore, poll the frontmost pid for up to 300 ms (~20 ms).
   - If still not P, try the SkyLight path once: the reference's
     `_SLPSSetFrontProcessWithOptions` with P's PSN and P's previous key window, as cua's
     focus restore does, if present in `/Users/rolandleyco/taucode-refs/cua`. Otherwise
     the existing `restore_focus_after_without_raise`.
   - Record `activation` with `verified: true|false` and the method used
     (`activateFrom` | `nsapp` | `skylight` | `legacy`).
   - At most one escalation per action.
4. **Key-focus restore when the previous app is the host:** `restore_key` with
   `from_window = 0` must still target the host's key window. Use the host's key-window id
   from AppKit (`NSApp.keyWindow.windowNumber`, read on the main thread) when
   `key_window_of_pid(host)` is `None`.
5. Don't change M3j's attribution rules (baseline, taucode-owned thefts, debounce,
   per-step draining), or the guarantees: never raise the target, respect real user
   switches.

## Tests

- Fake `Focus`: target activates while previous = host → nsapp path invoked once;
  verified when front flips back; escalation once when it doesn't
- previous = third-party app → `activateFrom` with `from` = target
- no escalation loop
- user switch during the window respected
- feature-detect fallback

## Files

`apps/taucode/src-tauri/src/computer/background/{focus_guard.rs,skylight.rs,tests.rs}`, and
`delivery.rs` only if its activate path shares the code. Nothing else.

## Constraints and verification

- No new crates. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3–M3j rules.
