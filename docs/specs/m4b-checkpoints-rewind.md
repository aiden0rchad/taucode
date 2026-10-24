# M4b: checkpoints and rewind (shadow git)

## Goal

Every user turn is checkpointed, so the user can **rewind** the workspace files, the
conversation, or both, to before any of their messages. That works like Claude Code's
rewind and Codex's undo. It never touches the user's own git repository (`.git`, index,
stash, branches) and never loses data: every rewind is itself undoable.

## Design (settled; implement as written)

### Shadow repository

- One bare repo per workspace at `<app data dir>/checkpoints/<workspaceId>.git`.
- Always invoke git with `GIT_DIR=<that>` and `GIT_WORK_TREE=<workspace path>`, plus
  `GIT_INDEX_FILE=<that>/index`, so the user's repo and index are never used.
- Also set `-c core.hooksPath=/dev/null`, `-c commit.gpgsign=false`, and a fixed author
  `taucode <checkpoints@taucode.local>`.
- Use the system `git` binary (already required by the app's git features; follow how
  existing code locates it).
- **Excludes:**
  - always `.git/` and `.codegraph*/`
  - the workspace's `.gitignore` rules (`core.excludesFile` plus info/exclude, via
    `--exclude-standard` semantics)
  - an optional `.taucodeignore`
  - files > 10 MB (skipped and listed in the checkpoint metadata)
- If a snapshot would add > 200 MB of new content, it's skipped with a visible warning on
  the turn (the "checkpoint skipped: too large" state). Never block the turn.

### When to snapshot

- **before** each user turn starts (before the engine receives the prompt)
- **after** each turn completes, fails or is interrupted
- before any rewind (`pre-rewind` safety snapshot)

Each snapshot is a commit on a per-thread ref:
- ref `refs/taucode/<threadId>`
- the message carries JSON trailers: `threadId`, `turnId`, `userItemId`,
  `kind (before|after|pre-rewind)`, `timestamp`, `engine`

Snapshots run asynchronously off the UI thread. A turn waits at most 2 s for its
"before" snapshot; on timeout the turn proceeds, and that turn is marked "checkpoint
unavailable".

### Rewind (only when the thread isn't running)

**UI entry:** a "Rewind…" action on each user message.

**Dialog:**
- **Mode**, one of:
  - *Files and conversation* (default)
  - *Files only*
  - *Conversation only*
- **File list:** changed since that message's `before` snapshot, from a diff of the
  current work tree against that commit. Each row: path, status (modified / added since /
  deleted since), a checkbox (default all checked), and an expandable diff.
  - Files **added since** will be **deleted**; show that explicitly in red text.
  - Files ignored by the excludes are never touched.
- **Confirm** button with a summary ("Restore 4 files, delete 1, rewind conversation to
  before 'fix the parser…'").

**Files:**
1. Take the `pre-rewind` snapshot.
2. For checked paths only: restore content from the target commit, or delete files that
   didn't exist then.
3. Never touch paths outside the workspace or excluded paths.
4. Leave the user's git index as it is (it's the same working files; only file contents
   change).

**Conversation:**
- **pi:**
  - fork the session at the entry **before** that user message: the new branch/leaf
    excludes the message and everything after it
  - make it the thread's active session, keeping the old branch reachable (pi sessions are
    trees; follow the existing fork code in `shared/pi/mod.rs`)
  - put the rewound user message text back into the composer (not sent)
- **codex:**
  - use app-server `thread/rollback` (number of turns to drop) on the same thread
  - if unsupported by the running codex version, offer *Files only* and disable the
    conversation modes with a tooltip

**Undo rewind:** after a rewind, show a toast/banner "Rewound. Undo". Undo restores files
from the `pre-rewind` snapshot (same rules), and for pi switches the active leaf back.
Available until the next turn starts.

### Checkpoints panel (small)

In the thread's menu, "Checkpoints…" lists that thread's checkpoints: time, turn preview,
files changed count, skipped/too-large markers. Each row offers "Rewind to here" (opens
the same dialog). Retention: the last 300 checkpoints per thread; prune older ones, plus
`git gc --auto`.

## Safety

- Workspace path canonicalized. Refuse if the workspace is `/`, `$HOME` itself, or a
  system dir. Show "Checkpoints disabled for this folder."
- All git invocations use argument arrays (no shell), with timeouts (snapshot 30 s,
  restore 60 s).
- A failing checkpoint never fails a turn. A failing restore aborts before deleting
  anything, and reports per-path errors.
- Concurrency: one checkpoint operation per workspace at a time (lock file in the shadow
  dir).

## Tests

**Rust (temp dirs, real git):**
- a snapshot never modifies the user's `.git` (hash the `.git` dir before and after) or
  their index
- `.gitignore` honoured; the large-file skip; the >200 MB skip
- rewind restores modified files, deletes added files, recreates deleted files, and leaves
  unchecked and excluded paths alone; a `pre-rewind` snapshot is created first
- undo restores the pre-rewind state
- refuses `$HOME` / `/`
- lock contention; timeout handling
- **pi** conversation rewind on a fixture session: the active leaf excludes the message;
  the old branch is retained; undo switches back
- **codex** rollback request shape (fixture)

**Frontend:** the rewind dialog (modes, file list, deletion warning, diff expand, confirm
summary), the undo banner, the checkpoints list, disabled states while running and for
unsupported engines.

## Files

- New `apps/taucode/src-tauri/src/shared/checkpoints/**`
- Hooks into the turn lifecycle where turns start and complete for both engines:
  `shared/pi/mod.rs`, codex turn paths in `shared/codex_core.rs` or the engine router
- Tauri commands
- `src/features/checkpoints/**`, plus `src/features/messages/**` for the action on user
  messages, `src/services/tauri.ts`, `src/types.ts`
- Don't touch `computer/**`, the browser, or engines' policy extensions.

## Constraints and verification

- No new crates or npm deps; use `std::process::Command` with `git`.
- Offline. Always set `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (§Verification). The known
  sandbox-only failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`
  may be excluded.
- Don't run `tauri build`.

## Out of scope

Side chats / agent tree (M4a), the ACP engine (M4c; it gets file checkpoints for free from
the turn hooks, and conversation rewind later).
