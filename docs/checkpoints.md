# Checkpoints & rewind

Checkpoints record eligible workspace files around turns. Rewind lets you inspect and selectively restore a prior state, with supported conversation rollback where the engine can provide it.

They are a recovery tool, not a replacement for Git commits or backups.

## What is stored

taucode uses a separate shadow Git repository under application data for each workspace. Its own index and refs are separate from your project's `.git`, branches, stash, and staging area.

Snapshots are taken around the turn lifecycle and before a rewind. Checkpoint availability can vary if a snapshot fails, times out, or exceeds limits. The agent's turn can continue even if its checkpoint is unavailable.

| Exclusion or limit | Effect |
| --- | --- |
| Project Git metadata | Not restored through the checkpoint store. |
| CodeGraph index directories | Excluded from snapshots. |
| Git-ignore rules and optional `.taucodeignore` | Ignored files are not a complete part of recovery. |
| Individual files over 10 MB | Skipped and reported in checkpoint metadata. |
| Snapshot adding over 200 MB | Can be skipped with a visible warning. |
| Before-snapshot wait | A turn waits at most two seconds before proceeding without an available checkpoint. |

The shadow store is local. Deleting application data can delete checkpoint history, and a drive failure can affect both the source folder and its local recovery store.

## Rewind a turn

1. Finish or interrupt the active turn.
2. Open **Rewind…** on the relevant user message or checkpoint entry.
3. Choose the available mode: files and conversation, files only, or conversation only.
4. Inspect the changed-file list and diffs.
5. Select only the files you want restored and review the deletion summary.
6. Confirm the operation.

Files created after the target snapshot can be **deleted** during restore. Read the confirmation instead of assuming that rewind only changes existing file contents.

## Conversation support depends on the engine

The pi path uses session branching to move before the chosen user message while preserving the previous branch. A Codex path can use app-server rollback when the installed version supports it.

If an engine cannot restore conversation state, use the offered file-only option. Files and conversation are related but distinct; restoring one does not automatically establish that the other matches it.

## Undo a rewind

Before restoring, taucode takes a safety snapshot. The offered **Undo** action can return to that pre-rewind file/conversation state while it remains available, typically until the next turn begins.

Review the resulting files after either operation. A restore error or excluded file may leave part of the folder outside the checkpoint's coverage.

Your project's Git index stays independent. Restoring working files can therefore change the relationship between the working tree and already staged changes. Inspect `git status` and diffs before committing.

## What rewind cannot undo

- A commit or push on a remote service.
- An email, chat message, form submission, or uploaded file.
- State changed through a third-party connector or desktop application.
- Files excluded by ignore rules, size limits, or workspace boundaries.
- The financial cost of model calls already made.

Use a normal backup for source material and important output artifacts. For destructive external operations, the relevant service's own recovery mechanism is the only potential undo.

## Unexpected engine termination

Checkpoints and session persistence also help retain output after an engine stops. On the verified pi/OMP paths, the app shows **Agent stopped unexpectedly**, preserves available progress, and lets you continue with a follow-up.

Unfinished commands do not restart automatically. The last unflushed output can be lost, and power-loss durability has not been established. Review both the conversation and working files before continuing.
