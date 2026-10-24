# M4a: visible subagent tree and side chats

## Goal

Two features, matching Claude Code desktop and Codex desktop:

1. **Agents panel:** a live tree of the subagents a thread spawned (nested to any
   depth). Each node shows role, model, status, elapsed, tokens/cost, and its own
   read-only transcript.
2. **Side chats:** branch conversations opened next to the main chat. They start from the
   main thread's context, so the user can ask questions or explore without polluting the
   main thread, and can send a result back.

Engine-agnostic: works for pi (default) and codex. The ACP engine (M4c) plugs in later.

## Context (read first; use codegraph)

### Subagents

- **pi** subagents run through `apps/taucode/engines/pi/extensions/taucode-subagents.ts`:
  - A `subagent` tool runs single, parallel or chain tasks.
  - It streams `onUpdate` snapshots with `details: {mode, results[]}`. Each result has
    `agent, task, model, status, exitCode, messages[], usage{input, output, cacheRead,
    cacheWrite, cost, contextTokens, turns}, stderr, stopReason`.
  - Child pi processes run with `--mode json`. Their own `subagent` tool results, carrying
    the same `details`, appear inside `messages[]`. That gives the nesting.
- `apps/taucode/src-tauri/src/shared/pi/translate.rs` (`tool_start`, `"subagent"` arm)
  turns these into a v2 `collabAgentToolCall` item with synthetic receiver ids
  `"{toolCallId}:agent:{i}"` and per-agent states. A fixture test exists
  (`shared/pi/tests.rs` `recorded_subagents_stream_states_outputs_and_usage_without_double_counting`,
  fixture `rpc_subagents.jsonl`).
- **codex** native collab agents arrive as `collabAgentToolCall` with real receiver thread
  ids. `src/features/threads/hooks/useThreadLinking.ts` and
  `src/utils/threadItems.collab.ts` already link child threads.
- UI rendering of collab items: `src/features/messages/components/MessageRows.tsx`.
- Right panel tabs: `src/features/layout/components/PanelTabs.tsx`
  (`PanelTabId = "git" | "files" | "prompts"`).

### Forking

- `thread/fork` exists for both engines: pi in `shared/pi/mod.rs` (~646, sets
  `forkedFromId`); codex in `shared/codex_core.rs` `fork_thread_core`; frontend
  `forkThread` in `src/services/tauri.ts`.

## Required

### 1. Agent tree model (backend → frontend)

- The pi translator keeps, per `collabAgentToolCall`, a structured tree built from the
  latest `details.results`. Each node carries:
  - `id` (stable: `{toolCallId}:agent:{i}`, nested `{parentId}/{childToolCallId}:agent:{j}`)
  - `agent`, `model`, `task`, `status` (`pending | running | completed | failed | aborted`)
  - `startedAt`, `endedAt`, `usage`
  - `lastMessage` (≤ 300 chars)
  - `children[]`
- Emit it on the item, e.g. `agentsTree` on the collab item.
- **Don't** put full transcripts on every streaming update. Expose them on demand: a new
  backend command `get_subagent_transcript(workspaceId, threadId, nodeId)` returns that
  node's messages (text, tool calls, results; secrets already redacted by the existing
  redaction path) from the translator's retained state or the pi session file.
- **Codex engine:** build nodes from collab items with real thread ids. The transcript
  comes from that child thread (existing APIs).
- Bound memory: cap retained transcript per node (last 500 messages / 2 MB, with a
  `truncated` flag).

### 2. Agents panel UI

- Add an **Agents** tab to `PanelTabs`. It shows the tree for the active thread, grouped
  by turn (newest first), with:
  - a live status dot, role, model, elapsed timer while running
  - tokens and cost; failed nodes show stderr / errorMessage
- Selecting a node opens its transcript (read-only, reusing the existing message
  components where possible) inside the panel, with a back button.
- Show a running-subagents count badge on the tab and on the thread row in the sidebar.
- Empty state: "No subagents in this thread yet" plus the current policy summary (max
  concurrent / per turn / depth, allowed models) from the existing agent policy settings.
- Keyboard accessible; works in the existing light/dark themes; phone/tablet layouts don't
  break (the panel may be hidden there).

### 3. Side chats

- **Starting one:** "Side chat" action on the thread header and on any assistant message
  (and the shortcut ⌘⇧K, avoiding existing bindings).
  - The header action forks the thread at its latest point; the message action forks at
    that message (for pi: fork at the session entry for that message; for codex: fork,
    then `thread/rollback` to that turn if needed, or "latest" only if rollback isn't
    available, disabled with a tooltip).
  - Creates a new thread marked `sideOf: {threadId, workspaceId, atItemId}` and persists
    that link in taucode's thread metadata (it survives restart).
- **Defaults:**
  - Side chats default to **read-only** access mode, so they can't edit files while the
    main agent works. The user can change it per side chat.
  - Same engine and model as the parent; the model is switchable like any thread.
- **Layout:**
  - opens in a resizable split to the right of the main chat (desktop); on narrow widths,
    as a full-screen sheet with a back control
  - multiple side chats per thread are listed under the parent in the sidebar (indented,
    with a side-chat icon); they aren't top-level threads
  - closing hides the split; deleting archives the side chat
- **Send to main:** each assistant message in a side chat has "Send to main". It inserts
  a quoted block with a "From side chat: <title>" header into the **main thread's
  composer**, never auto-sent.
- **Running state:** a side chat can run while the main thread runs. Both show their own
  running state; interrupting one doesn't affect the other.

### 4. Persistence and events

Side-chat metadata and the link to the parent live in the existing workspace/thread
metadata storage (follow `storage.rs` patterns). Events carry `sideOf`, so the reducer
can nest them.

## Tests

- **Rust:**
  - tree building from the recorded fixture: nesting from nested `details`, status and
    usage aggregation, stable ids across updates
  - transcript retrieval and caps; codex child-thread mapping
  - side-chat fork at latest and at an item for pi, with the link persisted
  - read-only default access
- **Frontend (vitest + testing-library):**
  - the Agents tab renders the tree, live updates and badge counts, transcript drill-down,
    empty state
  - side-chat creation from header and message, split view open/close, sidebar nesting
  - "Send to main" inserts into the main composer without sending
  - narrow layout sheet

## Files

- `apps/taucode/src-tauri/src/shared/pi/{translate.rs,mod.rs,tests.rs}`,
  `shared/codex_core.rs`, storage/thread metadata files, Tauri command registration
- `apps/taucode/src/features/{layout,threads,messages,app}/**`, a new
  `src/features/agents/**` and a new `src/features/sidechats/**`, `src/services/tauri.ts`,
  `src/types.ts`
- Don't touch `computer/**`, `engines/computer/**` or the browser.
  `taucode-subagents.ts` only if strictly needed to expose fields; keep its policy logic
  unchanged.

## Constraints and verification

- Follow the existing code style (React function components + hooks, existing CSS
  approach, existing reducer patterns). No new npm or crates dependencies.
- Offline. Always set `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (§Verification). The known
  sandbox-only failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`
  may be excluded.
- Don't run `tauri build`.

## Out of scope

Checkpoints/rewind (M4b), the ACP engine (M4c), per-subagent stop (interrupting the turn
stops all), cross-engine handoff.
