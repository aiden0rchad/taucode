# M3l: no false "Target process exited"; accurate activation verification

## Goal

Process-liveness checks never report a running target as exited. Activation-restore
verification reports `verified: true` when the restore actually landed.

## Evidence (live, after M3k)

1. **False "Target process exited"**
   - System Settings runs died twice with `Target process exited`: at step 2, right after
     a successful `ax-select` on General, and in an earlier run at step 7.
   - The process **never exited**: `pgrep` shows the same pid 62631 still running hours
     later. There's no crash report and no termination in the unified log.
   - The message has no site prefix, so the failing check is unknown.
   - Candidates, all using `NSRunningApplication::runningApplicationWithProcessIdentifier`
     and/or `isTerminated` from non-main threads:
     - `background/mod.rs` `Session::check` (~166) and `begin_foreground` (~603)
     - `background/capture.rs` (~20, ~37)
2. **Activation verify window too short**
   - The M3k activation restore works: a probe saw Notes take frontmost at 5.70 s and
     taucode get it back at 6.07 s, via the `skylight` escalation.
   - But the record says `verified: false`: the 300 ms verify window ended before the
     escalation landed.

## Required

1. **One liveness helper** (`background/process.rs` or similar), used by every site
   above:
   - Truth source: `libc::kill(pid, 0) == 0` (or `ESRCH` = gone), plus
     `proc_pidpath(pid)` resolving inside the bound app bundle path (captured at bind
     time) → **alive**. A path outside the bundle → **binding changed**.
   - `NSRunningApplication` is secondary (bundle id, terminated). If it returns nil or
     terminated while the kernel checks say alive with the same executable path, retry
     once after 100 ms, then trust the kernel checks.
   - Error strings name the site and the observed values, e.g.
     `Process check (pre-flight capture): pid=62631, kill=ESRCH` or
     `Process binding changed: pid=…, path=…, expected bundle=…`.
2. **Activation verify window:** in `focus_guard.rs`:
   - after the escalation, verify for up to 500 ms more (one verify window per method)
   - record `verified` from the final observation
   - keep the one-escalation-per-action rule
3. **Tests:**
   - the liveness helper with fakes: alive + NSRunningApplication nil → alive after
     retry; ESRCH → exited; path outside bundle → binding changed
   - each call site uses the helper (compile-level: one function)
   - the verify window covers a restore that lands after 370 ms

## Files

`apps/taucode/src-tauri/src/computer/background/**` only. `libc` is already a dependency.

## Constraints and verification

- No new crates. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3–M3k rules: a genuinely exited or
  rebound target still fails the run immediately.
