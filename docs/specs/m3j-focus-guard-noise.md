# M3j: stop spurious focus-guard restorations

## Goal

The focus guard must restore the user's focus **only** when taucode or the target app took
it, never because of the user's own key-focus state. When nothing was taken, it must make
zero activation calls. Every restoration must be attributable from the step log.

## Evidence

- Live suite runs where the user's frontmost app was **Notes** (pid 26557), while
  taucode drove TextEdit, Finder or System Settings:
  - every step recorded many `focusRestorations` to Notes
  - mostly `reason: "keyFocus"`, some `"backgroundKeyWindow"`
  - lists grow across steps: 2–6+ entries per step, including on pure `ax-element` /
    `ax-select` steps, which don't touch key focus
- Same tasks with **ChatGPT** frontmost:
  - **zero** restorations
  - a SkyLight probe sampling `SLPSGetKeyFocusProcess` / `_SLPSGetFrontProcess` every
    20 ms saw key focus stay on the user's app for the whole run
- Each `keyFocus` restoration isn't just a log line. `restore_locked` calls
  `activateWithOptions` on the user's app plus `ax_actions::restore_window`: real focus
  churn in the app the user is working in.

## Code (`apps/taucode/src-tauri/src/computer/background/focus_guard.rs`, `tick`)

The `keyFocus` branch fires whenever `skylight::key_focus_pid()` differs from
`s.previous` **and** any of these holds:
- it is the target
- it is the host
- **`Instant::now() < s.armed_until`**: *any* pid during the 250 ms after every
  `arm()` / `begin_action()`

It then releases `release_newest_theft_below(own)`, which can be **another process's**
theft, and re-activates the previous app. This repeats every 10 ms tick while armed.

Likely spurious path: some apps (Notes) report key focus under a pid ≠ their front pid,
or hold a legitimate non-taucode theft. During each armed window the clause matches,
and the guard fires repeatedly.

## Required

1. **Baseline, not "anything else"**
   - Capture the WindowServer key-focus pid as a **baseline** at run start and at each
     `begin_action`, alongside the front pid.
   - The `keyFocus` branch fires only when the current key-focus pid is the **target** or
     the **host**, or a pid that differs from the action's baseline **and** became key
     after the action began.
   - A key-focus pid equal to the baseline (whatever process it is) is the user's state:
     never restore it.
   - Drop the bare `now < armed_until` clause.
2. **Only release taucode-attributable thefts**
   - Release a key-focus theft only if its id was created by this session (track the ids
     from `steal_window_key_focus` / `steal_key_focus`), or if the key-focus pid is the
     target or host.
   - Never release another process's theft.
3. **Debounce and no-op avoidance**
   - At most one `keyFocus` restoration per action and per 1 s.
   - If a restoration didn't change the key-focus pid, don't retry within that action;
     record `keyFocusUnresolved` once.
   - `restore_locked` skips `activateWithOptions` when the previous app is already
     frontmost. For the `keyFocus` case, restoring key focus (theft release / focus
     record) is enough. Apply the same no-op check to `backgroundKeyWindow`.
4. **Per-step attribution**
   - `focusRestorations` on a step contains only restorations made during that step
     (drain per step; today it looks cumulative).
   - Each entry adds `keyPid`, `keyBundle`, `frontPid`, `baselineKeyPid` and
     `ms_since_action_start`.
5. Keep every real protection:
   - target or host activation → restore
   - taucode HUD/overlay taking key focus → restore
   - a real user switch to a third app → respected
   - foreground mode unaffected

## Tests (`background/tests.rs`, fake `Focus` / workspace)

- persistent key-focus pid ≠ front pid and ∉ {target, host} (the "Notes helper" case),
  with actions armed → **zero** restorations and zero activations
- key focus moves to the target during an action → exactly one restoration; repeat
  ticks don't add more; the no-effect case records `keyFocusUnresolved` once
- key focus moves to the host (HUD) → restored once
- a foreign theft id isn't released; this session's theft id is
- previous app already frontmost → no `activateWithOptions` call
- per-step draining: step 2 doesn't contain step 1's entries
- existing focus-guard tests still pass (user switch respected, target activation
  suppressed)

## Files

`apps/taucode/src-tauri/src/computer/background/{focus_guard.rs,delivery.rs,skylight.rs,mod.rs,tests.rs}`
and `apps/taucode/src-tauri/src/computer/runner.rs` (step draining) only.

## Constraints and verification

- No new crates. Offline. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Same verification command as `docs/specs/m3-computer-use.md`; the known sandbox-only
  failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` may be
  excluded.
- Don't run `tauri build`. No weakening of the M3–M3i rules.

## Out of scope

Everything else.
