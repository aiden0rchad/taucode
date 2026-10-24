# M3h: tolerate transient window/AX state without failing the run

## Goal

The live suite after M3g passes TextEdit and the System Settings pane denylist. Two runs
now die on **transient** state that should cause a re-capture, not a failed run:

1. **Notes** (window just reopened): step 1 hotkey →
   `Window validation: window=6730, owner before=26557 after=26557, bounds before=[-1.0, 65.0, 1730.0, 986.0] after=Some([0.0, 66.0, 1728.0, 984.0]), on-screen=true; action cancelled`.
   The window settled by 1–2 px after capture (a zoom/restore animation).
2. **System Settings**:
   - row selection to General (`ax-select`) and AXPress on About both worked; the window
     reached "About"
   - the next pre-flight capture failed:
     `Pane role: pid=…, window=Some(7010), depth=6, AXRole: ax_error(-25202)`
     (`kAXErrorInvalidUIElement`). An element died mid-walk during the pane transition.

## Required

1. **Window geometry drift**
   - In window validation (between capture and action), treat a same-id, same-owner,
     on-screen window whose origin/size moved by ≤ 4 pt per edge as unchanged.
   - Any larger move/resize, or a different id/owner, is a **stale observation**:
     - discard the step's prediction
     - re-capture and re-plan that step
     - this must not fail the run
   - Allow at most 2 stale retries per step and 6 per run, then fail with the existing
     diagnostic.
   - Record `staleRetries` on the step.
   - An owner/pid change or a window closing still fails immediately, as today.
2. **AX walk transients**
   - In the pane inspection walk and the AX snapshot walk, `kAXErrorInvalidUIElement`
     (-25202) or `kAXErrorCannotComplete` on a node during the walk restarts the walk
     (up to 3 attempts, ~150 ms apart).
   - After that, fail closed with the existing error (pane inspection stays fail-closed:
     no pane known means deny).
   - Record attempts in the diagnostic.
3. The post-action settle: after an `ax-select` / `ax-element` / `ax-menu` action, wait
   for the AX tree to settle before the next capture:
   - poll the window's AX title plus a cheap child count, ~100 ms apart
   - settled = two equal reads, max 1 s

## Tests

- drift ≤ 4 pt accepted; > 4 pt → re-plan path (fake env shows a second prediction is
  requested and the run continues); retry caps; owner change still fails
- walk restart on -25202 then success; three failures → fail closed (pane denied)
- settle polling: stable after two equal reads, timeout at 1 s

## Files

`apps/taucode/src-tauri/src/computer/**` only.

## Constraints and verification

- No new crates. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of M3–M3g rules.

## Out of scope

Everything outside `computer/`.
