# M4g: wave-1 integration (ACP × checkpoints × side chats × agents tree)

## Goal

M4a (agents tree + side chats), M4b (checkpoints/rewind) and M4c (Claude Code ACP engine)
were built in parallel and are now merged on `m1-wiring`. Close the gaps between them, so
threads on the **claude** engine get the same features as pi/codex wherever the engine
allows, and clearly disabled states where it doesn't.

## Required

1. **Checkpoints for ACP turns**
   - Call the M4b turn-lifecycle hooks around claude-engine turns: the `before` snapshot
     before `session/prompt` is sent (≤ 2 s wait, like the others), and `after` on
     completion/failure/cancel. Find how `shared/pi/mod.rs` and the codex path call
     `shared/checkpoints`, and do the same in `shared/acp/**` (or the engine router, if
     that's where the others hook).
   - Rewind for claude threads:
     - **Files only** works
     - conversation modes are disabled with the tooltip "Conversation rewind isn't
       supported for Claude Code threads yet", unless the adapter advertises session fork
       (then implement fork-at-latest only and keep at-message disabled)
2. **Side chats on claude threads**
   - Header "Side chat" uses ACP session fork when advertised.
   - Otherwise it's disabled with the reason "Claude Code adapter doesn't support forking
     sessions".
   - Message-level side chats are disabled for claude (tooltip).
   - Use the M4a `sideChatDisabledReason` mechanism and `extractEngineCapabilities`.
3. **Agents tree for Claude Task subagents:** feed whatever M4c translates into
   `collabAgentToolCall` (Claude's Task tool) into the M4a tree model. Nodes have agent
   type/description, status, and lastMessage. Transcript on demand returns the tool
   call's updates if per-subagent messages aren't available, with `truncated` or a note.
4. **Engine capability matrix** in one place (TS + Rust doc comment): for each engine,
   whether fork-latest / fork-at-item / conversation rewind / side chats / agents tree /
   checkpoints / steer / compact are supported. The UI reads it instead of ad-hoc checks
   where practical.

5. **Flaky test fix:** `shared::checkpoints::tests::lifecycle_binds_engine_ids_and_snapshots_after_completion`
   fails intermittently in the `codex_monitor_daemon` test binary under load
   (`Checkpoint operation did not finish`; an 8 s wall-clock poll in `wait_for_rows`). It
   passes alone. Make it deterministic: await the checkpoint operation's completion via a
   handle, channel or notify from the checkpoint service, rather than polling with a
   wall-clock deadline. Apply the same pattern to any similar polling tests in
   `checkpoints/tests.rs`.

## Tests

- Rust: checkpoint hooks fire for a fake-ACP turn (`before`/`after` commits exist; a
  cancel still creates `after`); side-chat request on a claude thread without fork →
  clear error; Task subagent → tree node.
- Frontend: the capability matrix drives disabled states (rewind modes, side chat
  buttons) for claude threads.

## Files

`apps/taucode/src-tauri/src/shared/{acp,checkpoints,sidechats.rs,pi/agents.rs,engines.rs}`,
`src/features/{checkpoints,sidechats,agents,threads}/**`, `src/types.ts`. Nothing else.

## Constraints and verification

- No new deps. Offline. Worktree-relative `CARGO_HOME` (with `$HOME/.cargo/bin` on PATH).
  Verification: the command in `docs/specs/m3-computer-use.md`, run from the worktree
  root; the known sandbox-only tailscale test may be excluded.
- Don't run `tauri build`.
