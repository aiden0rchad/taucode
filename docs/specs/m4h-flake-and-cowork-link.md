# M4h: deterministic checkpoint tests; imported Cowork tasks become Work tasks

## 1. Checkpoint test flakiness (must fix)

Under the normal parallel `cargo test` (not `--test-threads=1`), these fail
intermittently on the merged `m1-wiring`, a different one per run:

- `shared::checkpoints::tests::lifecycle_binds_engine_ids_and_snapshots_after_completion`
  (`tests.rs:241`, row count)
- `shared::acp::tests::router_checkpoints_sidechats_and_task_transcripts` (`tests.rs:551`,
  `rows.len() == 6`)

Both pass alone. **Likely cause:**
- the M4b design waits at most **2 s** for the `before` snapshot, then proceeds and marks
  the turn "checkpoint unavailable"
- the 30 s / 60 s git timeouts may also apply
- under a loaded parallel test run, git spawning exceeds those limits, so snapshots are
  skipped and rows go missing

Verify this hypothesis first (e.g., log or assert the skip reason in the failing tests).

**Fix:** make the time limits injectable on the checkpoint service (a config struct with
production defaults unchanged: before-wait 2 s, snapshot 30 s, restore 60 s). In tests,
construct the service with generous limits (e.g. 60 s), and await completion
deterministically. Also check for other shared state between tests: shared temp dirs,
fixed workspace ids under one data dir, the per-workspace lock file, `HOME`-dependent
paths (`repository.rs:58` reads `HOME`; tests must not mutate process env). Fix any
collisions with unique temp dirs per test.

**Acceptance:**
```
cd apps/taucode/src-tauri && for i in 1 2 3 4 5; do cargo test -q -- --skip listen_addr_preflight_fails_when_port_is_in_use || exit 1; done
```
passes all five runs with default parallelism.

## 2. Imported Cowork tasks → Work mode

M4e imports Cowork tasks as read-only "Work history", shown only under Settings → Import
(the M4e placeholder Work view was dropped in the merge in favour of the real M4f
`WorkView`).

**Wire them into Work mode:**
- each imported Cowork task appears as a Work task with status `done` (or `stopped` if
  archived/incomplete) in a space for its folder (create the space if needed, same rules
  as M4f; folder missing → an "Imported (folder missing)" pseudo-space, read-only)
- transcript from the imported history
- `outputs/` and `uploads/` copies listed in the Outputs pane
- "Continue" uses the M4e continuation fallback, producing a new running Work task with
  the summarized context

Undo import removes them too.

## Files

`apps/taucode/src-tauri/src/shared/{checkpoints,acp}/**` (tests + config injection),
`shared/imports*` / `import/**`, `shared/work*` / `work/**`, `src/features/work/**`,
`src/features/settings/components/sections/SettingsImportSection.tsx`, `src/types.ts`.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md`, **with Rust tests at
  default parallelism**, plus the five-run loop above. The known sandbox-only tailscale
  test may be excluded.
- Don't run `tauri build`.
