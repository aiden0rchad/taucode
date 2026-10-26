# M8a — Approval cards: never stale, always answerable, human-readable

## Goal

Live testing showed three defects in the Code-mode approval card (`ApprovalToasts`):

1. **Stale cards that can't be cleared.** After the user stops a turn (or it finishes) while an
   approval is pending, the card stays on screen forever, on every thread. Pressing a decision
   does nothing, because `handleApprovalDecision` only dispatches `removeApproval` after
   `respondToServerRequest` succeeds, and the reply to a dead request throws.
2. **Buttons cut off.** The card dumps every raw param (approval ID, item ID, thread ID, turn ID,
   available decisions, `None` values), so it grows taller than the viewport cap and the action
   row ends up below the visible area.
3. **Wrong label for connector calls from omp/pi.** omp calls MCP tools by writing JSON to a
   virtual path like `xd://mcp__taucode_browser_browser_navigate`. The pi translator only checks
   `approval["tool"].starts_with("mcp__")`, so these become `item/fileChange/requestApproval` with
   the reason "Pi requests permission to change xd://mcp__…".

Done means:
- A decision always removes its card, even if the reply fails. A failure is reported via the
  existing debug channel and never leaves a card behind.
- When a turn completes (any status, including interrupted/failed), every pending approval whose
  `params.threadId` matches that thread (and `params.turnId` matches, when both are present) is
  removed.
- The card shows a short readable summary:
  - a plain-language title per kind (Run a command / Change a file / Use a connector tool)
  - the reason
  - the command or path or tool name in a code chip when present
  - cwd as a muted line
  - the diff preview (scrollable) when present
  Internal fields (approvalId, itemId, threadId, turnId, availableDecisions, grantRoot) and
  null/`None` values are hidden. Unknown non-null fields can stay in a collapsed
  `<details>More details</details>`.
- The action row (Decline / Allow for session / Always allow… / Approve) is always visible: the
  details area scrolls inside the card, the actions don't.
- The pi translator treats a write/edit whose path starts with `xd://mcp__` as a connector tool
  call:
  - method `item/commandExecution/requestApproval`
  - reason `Connector requests permission to call <server> · <tool>`, derived from the path
    (`mcp__taucode_browser_browser_navigate` → server `taucode_browser`, tool
    `browser_navigate`). If the split is ambiguous, show the full name after `mcp__`.
  - the same decisions as the existing mcp branch

## Files

Modify only:
- `apps/taucode/src/features/threads/hooks/useThreadApprovals.ts` (remove the card in `finally`
  for both decision handlers; surface errors through the existing `onDebug`)
- `apps/taucode/src/features/threads/hooks/threadReducer/threadQueueSlice.ts` (+ action type in
  `useThreadsReducer.ts`): add a `removeThreadApprovals` action
  (`{ workspaceId, threadId, turnId? }`)
- `apps/taucode/src/features/threads/hooks/useThreadEventHandlers.ts`: on `turn/completed`,
  dispatch `removeThreadApprovals`. Find where `turn/completed` is handled; the event carries
  `params.threadId` and `params.turn.id`.
- `apps/taucode/src/features/app/components/ApprovalToasts.tsx`
- `apps/taucode/src/styles/approval-toasts.css`
- `apps/taucode/src-tauri/src/shared/pi/translate.rs` (the `ui_request` function only)
- Tests next to these files: existing `*.test.ts(x)` for the reducer/hooks/ApprovalToasts, and the
  existing pi translate tests in `apps/taucode/src-tauri/src/shared/pi/tests.rs`.

Off-limits: everything else, especially `ChatView.tsx`, `WorkView.tsx`, `visual-refresh.css`,
browser code, and `ComputerApprovals.tsx` (a separate toast).

## Constraints

- Follow the surrounding style: React function components, the existing Toast primitives
  (`ToastCard`, `ToastBody`, `ToastActions` …), plain CSS with the existing design tokens
  (`--text-faint`, `--border-subtle`, `--surface-card`, `--radius-chip` …). No new dependencies.
- Keep the Enter-to-approve behavior and its guard for focused inputs.
- Keep `pi:`-prefixed requests ineligible for "remember" (existing logic).
- Keep the cap at `max-height: min(520px, calc(100vh - 140px))`. Make the card a flex column with
  a scrolling details region, so the actions stay pinned at the bottom.
- Rust: match the existing compact style in `translate.rs`; keep the existing branches for real
  `bash` and `mcp__` tools unchanged.
- Tests to add:
  1. Reducer: `removeThreadApprovals` removes only the matching thread/turn.
  2. Hook: a decision whose reply rejects still removes the card.
  3. `ApprovalToasts` render test: internal IDs are not shown, and the reason, command and actions
     are.
  4. Rust: an `xd://mcp__taucode_browser_browser_navigate` write yields
     `item/commandExecution/requestApproval` and a reason containing `browser_navigate`.

## Verification

Run from `/Users/rolandleyco/taucode-m1w/apps/taucode`:

```
npx tsc --noEmit && npx vitest run && (cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home cargo test --lib)
```

All of it must pass. Expected baseline: 1,558+ vitest tests and 496 Rust tests.

## Out of scope

- Backend changes to emit `serverRequest/resolved` on interrupt (the frontend cleanup covers it).
- Computer Use approvals (`ComputerApprovals.tsx`).
- Any visual change outside the approval card.
- Committing. Don't commit; the reviewer commits.
