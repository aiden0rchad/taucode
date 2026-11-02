# taucode handoff for Codex — November 1, 2026

This picks up where `docs/HANDOFF-Claude-2026-10-31.md` left off. Read that file
for the architecture map, the Computer Use invariants (§6) and the build details
(§10). This file covers what changed on November 1, the current settings, open
work and the traps found along the way.

## 1. State right now

- **Update (Nov 1, late):** `m1-wiring` is merged and every side worktree was removed
  (branches kept). Work in the main checkout `/Users/rolandleyco/taucode` on `main`;
  `.cargo-home/` and `.tmp/` live there, ignored via `.git/info/exclude`.
- **Worktree (historical):** `/Users/rolandleyco/taucode-m1w`, branch `m1-wiring`.
- **Main checkout:** `/Users/rolandleyco/taucode`, branch `main`, then
  `8b67d06` from separate release work.
- **Publishing:** GitHub has release `v0.7.69`. This continuation has not pushed,
  tagged, published, or merged into `main`.
- **Untracked file to leave alone:** `docs/HANDOFF-window-resize.md`.
- **App build:** signed debug bundle at
  `apps/taucode/src-tauri/target/debug/bundle/macos/taucode.app` includes the
  continuation changes; strict codesign verification passes.
- **Validation:** Rust library suite 597 passed, 2 ignored; all 229 frontend test
  files passed (1,903 tests); TypeScript and `cargo check` pass. The Rust suite was
  built with `cargo test --lib`; its final compiled test executable was rerun after
  the last regression test was added. Frontend full-suite verification uses
  `npx vitest run --maxWorkers=4` to avoid a Theme Gallery timeout during compilation.

## 2. Rules the user set (keep following them)

- Don't push, release, tag or fast-forward `main` unless the user asks. "Fast-forward
  main" means `git -C /Users/rolandleyco/taucode merge --ff-only m1-wiring`.
- Commit on `m1-wiring` when a task is done. End commit messages with a
  `Co-Authored-By:` line naming the model that did the work.
- Never use ad hoc or self-signing replacements, create certificates, delete provider
  keys, reset app data or TCC, or script Keychain changes. Use `scripts/sign-dev.sh`.
- Keep keys and secrets out of logs, commits, prompts and issue text.
- Never read OMP `spec.json` or `~/.codex/auth.json` (they hold secrets).
- Never replay uncertain input automatically in Computer Use (an AX press that
  returned `-25204` may have happened). Verify instead.
- Don't kill arbitrary processes that own ports; don't expose servers beyond 127.0.0.1.
- Quit the running app through its menu (taucode → Quit taucode) before replacing
  its bundle. Relaunch with `open -g <bundle>`.
- The user prefers autonomous work: chain steps, stop only at real blockers, and
  report outcomes plainly (including what was not verified).
- Before changing user data files (`settings.json`, `workspaces.json`), back them
  up next to the original (pattern: `settings.json.before-<reason>`).

## 3. Build, sign, run, test

From `apps/taucode`:

```bash
PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode/.cargo-home TMPDIR=/Users/rolandleyco/taucode/.tmp npx tauri build --debug --bundles app && ./scripts/sign-dev.sh && codesign --verify --deep --strict src-tauri/target/debug/bundle/macos/taucode.app
```

```bash
open -g src-tauri/target/debug/bundle/macos/taucode.app
```

Tests:

```bash
cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode/.cargo-home TMPDIR=/Users/rolandleyco/taucode/.tmp cargo test --lib
```

```bash
npx vitest run src/features/computer src/features/settings src/features/chat src/styles
```

```bash
npx tsc --noEmit -p .
```

```bash
cd engines/computer && .venv/bin/python -m pytest -q
```

- `cargo test --lib computer::` is the fast Computer Use subset (about 136 tests).
- The computer module is compiled into both the app and the daemon
  (`codex_monitor_daemon`). Code there can't use `crate::event_sink`; use
  `tauri::Emitter::emit_to(app, "main", …)`. Inside the shared module the path is
  `crate::shared::computer::…`.
- Python tests must run from `engines/computer` or collection fails.
- App data: `~/Library/Application Support/dev.taucode.desktop/` (`settings.json`,
  `workspaces.json`, `computer/runs/<id>.json`, `engines/codex-home`, `pi/agent/<workspace>`).

## 4. What landed since the Oct 31 handoff (oldest first)

| Commit | What |
| --- | --- |
| `eb02652` | Computer Use approval prompts show at once even when taucode is in the background (pushed `computer-approval` event; toast animation off). |
| `55f2aa2` | Per-step planning limit is a setting (`plannerSeconds`, default 120, 10–600). |
| `704d33b` | Codex-desktop-style app access: any installed app by name or bundle ID, per-app prompt (Decline / session / always), never-allowed safety list, user block list. |
| `d1d9c11` | Just-launched apps: windowless reopen retries while AX isn't ready; a field that resizes or is rebuilt in place stays the same target. |
| `e3a8efc` | **Approvals off by default** (`askApproval: false`); typing confirmation default `risky-only`; password/sensitive fields refused instead of prompted. |
| `9333363` | App sweep fixes: skip 0×0 windows; long lists no longer crowd toolbars out of the AX snapshot; focus wrapped/unfocused search fields; relabel tolerance; confirm an uncertain press whose element vanished; Finder reopen; never type into a focused field outside the target. |
| `cabe1c8` | **Chat queue and steer** while a reply runs (Enter / Shift+Cmd+Enter; new `steer` chat action → pi `turn/steer`). |
| `e38b7ed` | Note: the old flaky preview-bridge test no longer exists in that form; suite green. |
| `18a5c4e` | Fields that need the app in front (Notes search) use the opt-in foreground fallback; reliable activation; pointer events recheck only the target app. |
| `8b2fad0` | GitHub update check and "Report an issue" dialog (`docs/updates-and-issues.md`). |
| `2267b6c` | **Project agent roles no longer leak between projects** (Codex shared home). |
| `80600e7` | Settings ID/slug/command fields turn off macOS capitalization and autocorrect. |

Full acceptance notes for each are in `docs/dogfood-acceptance.md` (sections dated
October 31 and November 1).

## 5. Computer Use: current behavior

### Settings (defaults, and the user's current values)

| Setting (`settings.json` → `computer`) | Default | User's value |
| --- | --- | --- |
| `askApproval` | `false` | false (no prompts) |
| `appAccess` | `"ask"` (any installed app; prompts only if `askApproval`) | ask |
| `confirmTyping` | `"risky-only"` | risky-only |
| `allowForegroundFallback` | `false` | **true** (turned on for the Notes fix) |
| `plannerSeconds` | 120 | 120 |
| `deniedApps` | empty | Messages, Mail, Contacts, Photos, Journal, Phone, FaceTime, Find My, iPhone Mirroring, Privacy.com |

Backups of the user's settings are next to the file: `settings.json.before-app-access`,
`.before-risky-only`, `.before-foreground-fallback`.

Always refused, whatever the settings (`src-tauri/src/computer/policy.rs`
`ALWAYS_DENIED`, `app_blocked`): password managers, Keychain, terminals, the macOS
password/Touch ID prompts, taucode itself, plus the user's `deniedApps`. With
approvals off, typing into secure or sensitive fields (password, card, PIN, …) is
refused (`policy::decide` wraps `review`). `app-approvals.json` still lists
`com.apple.Dictionary` from a test of "Always allow"; it has no effect while
approvals are off. It can be removed in Settings once approvals are on.

### Where things live

- Runner loop and per-event checks: `src-tauri/src/computer/runner.rs`
- Policy: `src-tauri/src/computer/policy.rs`
- Background session (AX, focus, capture, foreground): `src-tauri/src/computer/background/`
  (`mod.rs`, `ax_actions.rs`, `ax_tree.rs`, `windows.rs`, `capture.rs`, `skylight.rs`, `focus_guard.rs`)
- Per-event context check: `src-tauri/src/computer/control.rs` (`post`)
- Run orchestration and app resolution: `src-tauri/src/computer/mod.rs` (`run`, `Live`)
- Sidecar planner (Python): `engines/computer/taucode_cua/`
- Frontend: `src/features/computer/`, Settings → Computer Use

### App sweep (signed build, approvals off)

Passing: Calculator, TextEdit, Dictionary (cold launch), Clock, Reminders (search),
Stickies, Weather (waits for data), Maps (search), Finder (opens a window,
Applications), System Settings (read-only), Safari (window title), Notes search
when the field is empty (foreground fallback).

Known gaps, not fixed:

1. **Notes search with a query already in the field.** Notes rebuilds the field
   continuously, so the snapshot element is dead by input time; the step is
   cancelled ("AX policy target changed after approval… role after=''") and replanned.
   A fix would need to re-resolve a dead targeted element by role and place at
   input time. Keep the no-replay rule.
2. **Preview's Open dialog** (window `AXIdentifier` = `open-panel`; Save is
   `save-panel`). It is drawn by another process. AX press returns `-25204` and does
   nothing; a background click does nothing; a foreground click also did not close
   it on this two-display setup (5120×1440 main + 2× built-in). The window bounds
   also differ by about 6 pt between SCK and CG on every capture, which flags the
   observation as stale. Routing attempts were removed; the press now records
   `ancestry:` and `window:` diagnostics. Next step: log the exact global point of
   the foreground click versus the Cancel button's AX frame on the external display.

   **Findings (Nov 1 evening, Claude):**
   - Gap 1 (Notes) is fixed in `681c3e1`: an empty-role AX read after approval is re-read,
     not treated as a changed target. Verified live: replacing an existing query works.
   - taucode's AXPress allows 50 ms (`ax_actions.rs` `apply`). On Preview's Cancel this
     returns -25204, yet an external probe pressing with 50 ms saw the panel go away
     within 0.5 s, with the button's handle still answering reads (status 0) for 600 ms.
     So "pressed element removed" confirmation by liveness can never fire for this panel;
     reachability from the window root (AXChildren links) is the signal to test.
   - Claude's own background AXPress (2 s timeout) closed the panel cleanly.
   - **Watching CG windows during a taucode run:** when the run binds Preview, the Open
     panel's window is destroyed and recreated (new window id) at x≈5544 — past the
     5120-wide main display, likely on the built-in screen. The planner then hits stale
     window validation. Some "panel closed" readings may have been this move, not a close.
   - Probe tools in `apps/taucode/scripts/ax-probes/` (build with `swiftc -O`): `press <timeout> [delay]` / `press check`, and `winwatch <s>`
     that logs Preview's window ids/positions on change. AX probes must run from a process
     with Accessibility (the Claude terminal tab), not the sandboxed shell.
   - **Resolved (later Nov 1):** the press works; the panel closes about 1 s later. The new
     panel came from `capture.rs`'s windowless reopen: with no windows left, the next capture
     reopened Preview, which shows its Open panel again. Captures no longer reopen an app
     once the run has acted (`Session::acted`); the run now ends saying the app has no open
     windows. Verified with `winwatch`: the panel closes and nothing reappears.
   - **Run end (Nov 1 night):** a capture that finds no windows after the run acted now gives
     the planner a read-only verification step (frame from before the action), so a run like
     "cancel the Open dialog" ends done instead of failing (`runner::NO_WINDOWS`).
     This also covers an uncertain press (Preview's Cancel answers -25204). Live sweep after
     these changes (debug build, approvals off): Calculator, TextEdit, Dictionary, Reminders,
     Maps, Weather pass in the background; Notes search passes via the foreground fallback;
     Preview's Open dialog cancel now ends done with one press.
3. **Notes search stays a foreground step.** Probed with `scripts/ax-probes/notessearch.swift`
   (Notes 2026-11-01, behind another app): the toolbar search field (`AXTextField`,
   subrole `AXSearchField`, only action `AXConfirm`) reports `AXValue`, `AXFocused` and
   `AXSelectedText` as not settable, and Edit › Find › Note List Search… is disabled while
   Notes is inactive (AXPress returns 0 and does nothing). No background route exists, so the
   opt-in foreground fallback stays. Searching the note list another way (AppleScript
   `notes whose name contains`) would be a separate, non-UI tool.

### Testing Computer Use live

- Use the taucode Code conversation "Use your Computer Use tool on Calculator…" in
  the `taucode-dogfood-focus-board` workspace and ask for runs in plain words, e.g.
  `Make exactly 1 Computer Use run with apps ["Notes"]: …`.
- Each run is written to `computer/runs/<id>.json` (`phase`, `error`, `summary`,
  `steps[].action`, `steps[].path`, `steps[].error`, `steps[].executionFallbackReason`,
  `steps[].grounding`). Read results there instead of from screenshots.
- `executionFallbackReason` strings explain background failures, e.g.
  `background_keyboard_target_not_focused; focus: …`, `remote_view_element…`,
  `background_text_field_unfocusable…`, `ax_press_failed(-25204); ancestry: …; window: …`.
- Lock check: `ioreg -n Root -d1 | grep CGSSessionScreenIsLocked`. Captures fail
  while the screen is locked.

## 6. Chat queue and steer (`cabe1c8`)

- Backend: `src-tauri/src/shared/chats.rs` action `steer` (same input checks as
  `send` via `Chats::input`; refused unless a reply is running) → pi `turn/steer`.
- Frontend: `src/features/chat/useChat.ts` (`queue`, `unqueue`, `steer`,
  `queued`, `pausedReason`, flush effect) and `ChatView.tsx` (keys, hint line,
  `ComposerQueue`).
- Behavior: while a reply runs, Enter follows Settings → Composer → Follow-up
  behavior (default queue); Shift+Cmd+Enter does the other. Queued messages send
  one at a time after a **completed** reply; a stopped or failed reply pauses the
  queue. A steer that arrives after the reply ended is queued instead.
- Code mode already had queue and steer (CodexMonitor). **Work mode steers but has
  no queue** — a possible follow-up.

## 7. The agents bug (pasted brief "I add an agent to a project and it also adds another agent on other projects")

### What the cause actually was

The brief's guess (the Codex-config agents editor writing to `~/.codex`) was wrong:

- `SettingsAgentsSection` / `useSettingsAgentsSection` / `agents_config_core.rs` are
  **dead code**: no settings page renders them. Settings → "Agents" shows **Models &
  Agents**, whose Roles tab edits the app policy (`settings.json` →
  `agentPolicy.subagents.roles`). Per-project roles live in Settings → Projects →
  `"<project>: agent policy"` (`workspace.settings.agentPolicy`).
- taucode's Codex never uses `~/.codex`. It uses one **shared** managed home,
  `engines/codex-home` (`src-tauri/src/codex/home.rs`).
- `prepare_codex` (`src-tauri/src/shared/engines.rs`) rewrote that home's
  `config.toml`, roles included, from the effective policy of whichever project's
  Codex started last. Each Codex process also gets its own roles through `-c`
  overrides (`codex_config_args`), but Codex **adds agent roles from every config
  layer together** (Codex source: `/Users/rolandleyco/taucode-refs/codex/codex-rs/agent-roles/src/loader.rs`).
  So a project saw its own roles plus the last-started project's.
- Ruled out: one conversation under two projects (`resolveWorkspaceIdForThreadPath`
  picks the single longest matching root with a path-boundary check).

### What changed

- `engines.rs`: `without_roles` strips `[agents.<role>]` tables from the shared
  `config.toml`; roles reach each project's Codex only via its own `-c agents=…`.
  Scalars (`max_threads`, model) stay; a higher layer overrides them.
- Test: `shared::providers::tests::one_projects_codex_roles_never_reach_another_project`
  (fails on the old code).
- UI: Models & Agents → Roles says **All projects** and points to Settings →
  Projects; a project's agent policy says **This project only**
  (`AgentPolicyEditor` `scope` prop in `SettingsRegistrySections.tsx`).
- `80600e7`: role-name and other exact-value fields no longer get capitalized by
  macOS (`SettingsTextField identifier`).

### Engines

- **pi** (default): per-project policy file under `pi/agent/<workspace-id>`
  (`shared/pi/config.rs` `sync_policies`). Isolated.
- **omp and other pi-protocol engines:** effective policy per workspace at request
  time (`engines.rs` `configuration_for`). Isolated.
- **Codex:** fixed by `2267b6c`.
- **Claude ACP:** does not use taucode roles at all.

### Current implementation

- The active Roles tab has an **Apply roles to** picker. With a current project,
  it defaults to **This project**; **All projects** edits the app policy.
- Project changes use the existing workspace-ID-based settings command and start
  from the effective policy. Only changed fields are saved; unrelated overrides
  keep their values. Project role lists replace the inherited list, matching the
  app's existing policy semantics. Each role shows its scope.
- The unused `agents_config_core` editor, hook, IPC commands and daemon handlers
  are removed. No user config or agent files are migrated, edited or deleted by
  that removal. The active app-policy path is the source of truth.
- Scope changes require saving or discarding edits first. The current workspace
  comes from the app's selection, never `projects[0]`.
- Codex, pi and omp use the effective project policy. Claude ACP still does not
  implement taucode role delegation; it ignores these roles.

### Signed-build acceptance

- Add `alphaonly` through Models & Agents → Roles in `taucode-dogfood-focus-board`:
  picker defaults to that project, and Save persists only its workspace override.
- After relaunch, that project shows the role. Habit Tracker does not. Switching
  to All projects also excludes it. All five other saved project policies and the
  global policy exclude it; the shared Codex config contains no `alphaonly`.
- The temporary override is removed through Use app policy. Its original value
  and the unchanged global policy are verified against backups next to the data files.
- Partial overrides returned by Rust contain null fields. Frontend merging ignores
  those fields, matching Rust inheritance; this prevents Settings from crashing
  after saving a project-only role. A regression test uses the serialized shape.
- Codex process launch isolation is covered by the Rust test
  `one_projects_codex_roles_never_reach_another_project`; the live A/B check verifies
  the signed UI and persisted policy, not a new model turn in each engine.

## 8. Update check and issue reports (`8b2fad0`)

See `docs/updates-and-issues.md`. Backend `src-tauri/src/github.rs` and
`github/download.rs`; frontend `src/features/update/` and `src/features/report/`.
GitHub has published `v0.7.69` with macOS ARM64 and Windows x64 installers.
Update downloads run inside taucode with progress, size/checksum verification,
retry and an **Open installer** action. The signed app downloaded `taucode_0.7.69_aarch64.dmg` (126,888,451 bytes),
verified its SHA-256 against the published digest, and retained macOS quarantine.
Open installer dispatches to macOS; installation over the local test build was
not performed. The test download remains in Downloads.

Automatic bundle replacement remains
unconfigured: the inherited updater plugin has no public key or updater artifacts.

## 9. Other open work

- Notes search with an existing query (§5 gap 1).
- Preview Open/Save dialog (§5 gap 2).
- Queue in Work mode (§6).
- Release publishing and changes to `main` require the user's authorization.

## 10. Traps found on November 1

- **WKWebView in a background window** ignores synthetic Backspace/Cmd+A and
  scrolls on Space; background clicks may only bring the window forward. For live UI
  checks use AX clicks plus typed text, or full-screen control.
- **macOS auto-capitalization** rewrote input values on blur (fixed for settings ID
  fields; other text inputs may still be affected).
- **Settings role names** must match `^[a-z0-9_-]{1,32}$` and be unique, or "Save
  project override" stays disabled with "Role names must be unique slugs."
- **Codex config layers add roles together**; scalar values override. Anything
  written into the shared `engines/codex-home/config.toml` reaches every project.
- **Two displays:** the main display is a 5120×1440 ultrawide at 1×, the built-in is
  2×; windows on the external display had inconsistent SCK/CG bounds.
- **The user's `~/.codex/config.toml`** has `[projects."/Users/rolandleyco"]
  trust_level = "trusted"`, which trusts every folder under the home directory for
  Codex CLI runs. It isn't taucode's Codex home, but worth reviewing with the user.
- A second agent session may share this worktree; commit only your own files and
  check `git status` before staging.
