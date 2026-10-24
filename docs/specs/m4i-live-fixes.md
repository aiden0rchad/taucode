# M4i: fixes from the live M4 run

A live run through the daemon (real provider), 16 of 17 checks passing, found these
bugs. Fix each with a regression test.

## 1. Side chats aren't read-only at the backend (security)

**Observed:**
- `create_side_chat` → a new pi thread whose access file
  `pi/agent/<ws>/taucode/threads/<sideThreadId>.json` is `{"accessMode":"ask"}`
- the read-only default is applied only by the frontend
- a turn sent with `accessMode: "current"` (or from any client that doesn't send
  read-only) let the side chat create a file after approval

**Required:**
- at creation (`shared/sidechats.rs`, all engines), persist `read-only` as the side
  chat's access mode
- `accessMode: "current"` must resolve to that persisted value
- the user can still change it explicitly per side chat (an explicit
  `ask` / `full-access` in `send_user_message` or the access-mode setter)
- test: create a side chat, then a turn with `"current"` → the gate blocks write/edit and
  mutating bash (no approval prompt)

## 2. Checkpoints bound to the wrong user item

**Observed:** a pi thread with two user turns produced 4 checkpoints
`[before, after, before, after]`, **all with the same `userItemId`** (the thread id's
prefix: `e0e09576…` = the thread id). So per-message rewind can't tell turns apart.

**Required:**
- every `before`/`after` checkpoint carries the real user item id and turn id of its turn
  (pi, codex and claude)
- if the user item id isn't known yet when the `before` snapshot is taken, bind it when
  `item/started userMessage` arrives (M4b intended this; check `observe()`)
- the rewind dialog lookup (by user item) must select that turn's `before`
- test: two turns → distinct userItemIds and turnIds; rewinding at message 2 picks
  checkpoint 3

## 3. ACP errors lose their message

**Observed:**
- the adapter returned
  `{"code":-32603,"message":"Internal error: API Error: 400 Routing and Anthropic beta override headers are disabled for Claude."}`
  for `session/prompt`
- taucode surfaced only `send_user_message: {"message":"Internal error"}` and no turn
  error

**Required:**
- propagate the adapter's error `message` (and `data.errorKind` if present) into a
  failed turn (`turn/completed` with status failed and `turn.error.message`), and into
  the RPC error text
- redact secrets as usual
- also surface `agent_message_chunk` text that starts with `API Error:` as the turn error
  when the prompt result is an error
- test with the fake ACP agent returning that error

## 4. Undo after a files-only rewind touched the conversation flag

**Observed:**
- `rewind {mode:"files"}` returned `conversationChanged:false`
- the following `undo` returned `conversationChanged:true`

**Required:** undo reverts exactly what the rewind changed (files only → files only), and
the result flags reflect that. Test included.

## 5. Docs (no code)

In the Claude Code engine's settings help text, add: "Some Anthropic-compatible gateways
reject Claude Code's `anthropic-beta` headers (e.g. LiteLLM with beta overrides
disabled). If turns fail with a 400 about beta headers, allow those headers on the
gateway or use a different model route."

## Files

`apps/taucode/src-tauri/src/shared/{sidechats.rs,checkpoints/**,acp/**,pi/**}`,
settings help text, tests. Nothing else.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` at default Rust
  parallelism; the known sandbox-only tailscale test may be excluded.
- Don't run `tauri build`.
