# M0 spike: Tauri app drives `codex app-server`

## Goal

The Tauri desktop app in `apps/desktop` starts `codex app-server` as a child process and
drives it over JSON-RPC: start a thread in a chosen folder (optionally with a model and model
provider), send a prompt, stream the agent's reply and command/file-change items into the UI
as they happen, let the user approve or deny command and file-change requests, interrupt a
turn, and fork the thread onto another model provider. A headless example binary exercises the
same Rust engine code without the UI so it can be smoke-tested from a terminal.

Done = the verification command below passes, and `cargo run --example m0_smoke` compiles
(it will be run against a real server outside your sandbox, not by you).

## Files

Create or modify only these:

- `apps/desktop/src-tauri/src/engine/mod.rs` — `Engine` handle: spawn, request, notify,
  respond to server requests, shutdown.
- `apps/desktop/src-tauri/src/engine/rpc.rs` — wire message types, line framing, routing of
  incoming messages.
- `apps/desktop/src-tauri/src/lib.rs` — register engine state and the Tauri commands below;
  remove the scaffold `greet` command.
- `apps/desktop/src-tauri/examples/m0_smoke.rs` — headless smoke test.
- `apps/desktop/src-tauri/tests/engine.rs` and `apps/desktop/src-tauri/tests/fixtures/fake_app_server.py`
  — integration tests against a fake server.
- `apps/desktop/src/App.tsx`, `apps/desktop/src/App.css`, `apps/desktop/src/engine.ts`
  (typed wrappers over `invoke` and `listen`).
- `apps/desktop/tsconfig.json`, `apps/desktop/vite.config.ts` — only to add a path alias
  `@protocol/*` → `../../packages/protocol/src/*`.

Off-limits (do not edit): `packages/protocol/src/**` (generated), `docs/PLAN.md`, `.mcp.json`,
`.claude/**`, `.codex/**`, `AGENTS.md`, `Cargo.toml` dependency lists, `package.json`
dependency lists.

## Constraints

### Wire protocol (observed from codex-cli 0.146.0)

- One JSON object per line on stdin/stdout. No `"jsonrpc"` field in either direction.
- Client → server request: `{"id": <int>, "method": "...", "params": {...}}`.
  Client → server notification: `{"method": "...", "params": {...}}` (params may be omitted).
- Server → client: response `{"id", "result"}` or `{"id", "error": {"code","message"}}`;
  notification `{"method", "params"}`; **server request** `{"id", "method", "params"}`, which
  needs a reply `{"id", "result": {...}}` or `{"id", "error": {...}}`.
- Handshake: request `initialize` with
  `{"clientInfo": {"name": "taucode", "title": "taucode", "version": "0.0.1"}}`, wait for the
  result, then send notification `initialized`. Nothing else before that.
- Then e.g. `thread/start` `{"cwd": "...", "model"?: "...", "modelProvider"?: "..."}` →
  result `{"thread": {"id", ...}}`; `turn/start` `{"threadId", "input": [...]}`.
- stderr carries log lines, not protocol. Some are ERROR level and harmless (MCP servers
  failing to start, config warnings).
- Exact param/response shapes: use the generated types. Key ones: `ClientRequest.ts`,
  `ServerRequest.ts`, `ServerNotification.ts`, `v2/ThreadStartParams.ts`,
  `v2/TurnStartParams.ts`, `v2/UserInput.ts`, `v2/ThreadForkParams.ts`,
  `v2/TurnInterruptParams.ts`, `v2/CommandExecutionRequestApprovalParams.ts` and its
  `...Response.ts`, `v2/FileChangeRequestApprovalParams.ts` and its `...Response.ts`,
  `v2/ThreadItem.ts`. Look things up with codegraph first (`codegraph_explore` MCP tool, or
  `codegraph explore "<names>"` in the shell); the protocol folder has ~620 files, so avoid
  grepping or listing it wholesale.

### Rust engine

- tokio only (already a dependency, with process/io-util/sync/macros/rt-multi-thread/time),
  plus serde/serde_json/anyhow/tauri. **Add no new crates.** Builds are offline; if something
  can't be done without a new crate, stop and report it.
- Binary: `codex` from PATH, overridable with env `TAUCODE_CODEX_BIN`; args `["app-server"]`.
  Make the command and args injectable so tests can run the fake server instead.
  `kill_on_drop(true)`.
- One writer task fed by an mpsc channel. One reader task that parses each stdout line and
  routes: responses → the pending request's oneshot (map keyed by id); notifications and
  server requests → a broadcast/mpsc event stream the caller subscribes to. A stderr task
  forwards lines to the same event stream as a distinct event kind.
- Request ids: monotonically increasing integers. Requests time out after 120 s with an error
  (configurable).
- Keep messages as `serde_json::Value` at this layer; no hand-written structs for
  protocol payloads. Typing happens in TypeScript from the generated types.
- Unknown server requests the UI does not answer within 10 minutes: reply with error code
  `-32601` so the server never hangs. On engine exit, fail all pending requests and emit an
  exit event with the exit status.

### Tauri commands and events

Commands (all async, return `Result<serde_json::Value, String>`):

| Command | Does |
|---|---|
| `engine_start` | spawn + handshake; idempotent if already running |
| `engine_request { method, params }` | generic passthrough request |
| `thread_start { cwd, model?, modelProvider? }` | `thread/start` |
| `turn_start { threadId, text }` | `turn/start` with a single text input |
| `turn_interrupt { threadId, turnId }` | `turn/interrupt` |
| `thread_fork { threadId, model?, modelProvider? }` | `thread/fork` |
| `server_request_respond { id, result?, error? }` | reply to a server request |

Events emitted to the webview: `engine://notification` (the raw message),
`engine://server-request` (the raw message), `engine://stderr` (string),
`engine://exit` (`{ code: number | null }`).

### UI (functional, minimal)

No new npm packages, no CSS framework. One screen:

- Top bar: folder path input (default: the user's home), model input, model-provider input
  (both optional; empty = server default), "Start thread", "Fork to provider" (uses the
  current model/provider inputs), engine status.
- Transcript: render items by id from `item/started` / `item/completed`, append
  `item/agentMessage/delta` text to its agent-message item, append
  `item/commandExecution/outputDelta` to its command item. Show the user's message, agent
  messages, reasoning summaries (collapsed), command executions (command, output, exit code),
  file changes (paths), and any other item type as its `type` plus collapsed JSON.
- Approvals: when `engine://server-request` arrives for command or file-change approval,
  render it inline on the matching item (fall back to the end of the transcript) with
  Approve / Deny buttons that call `server_request_respond` with the response shape from the
  generated `...Response.ts` type.
- Composer: textarea + Send (Cmd+Enter); Stop button while a turn is running.
- Turn status line from `turn/started` / `turn/completed`; errors shown inline.
- Collapsible raw event log at the bottom, capped at the last 500 messages.
- Import protocol types with `import type` from `@protocol/...`.

### Smoke example `m0_smoke`

`cargo run --example m0_smoke -- --cwd <dir> --prompt <text> [--model M] [--provider P] [--auto-approve] [--fork-provider P2 --fork-model M2 --fork-prompt T]`

Starts the engine, starts a thread, sends the prompt, and prints one line per notification
(method + short summary; agent text deltas printed inline). With `--auto-approve`, it approves
command and file-change requests and prints them. Exits 0 when `turn/completed` reports a
completed status, non-zero on failure or timeout (10 min). If the `--fork-*` flags are given,
after the first turn it forks the thread onto that provider/model, runs `--fork-prompt` on the
fork, and requires that turn to complete too. Print the thread ids and each thread's
`modelProvider`.

### Style

Rust 2024 edition, small modules, doc comments only where behaviour isn't obvious. TS
strict mode; functional React components with hooks, no state library.

## Verification

From `/Users/rolandleyco/taucode-m0`:

```
cd apps/desktop/src-tauri && CARGO_HOME=/Users/rolandleyco/taucode-m0/.cargo-home cargo clippy --all-targets -- -D warnings && CARGO_HOME=/Users/rolandleyco/taucode-m0/.cargo-home cargo test && cd .. && npm run build
```

Always set `CARGO_HOME` as shown: dependencies are vendored there and cargo is configured
offline. `tests/engine.rs` must cover, using the fake Python server (stdlib only, run with
`python3`):

1. handshake order (`initialize` then `initialized`, and nothing before `initialize`)
2. concurrent requests resolve to the right responses when answered out of order
3. notifications reach subscribers in order
4. a server request is delivered and the client's reply reaches the server with the same id
5. an error response surfaces as an error
6. server exit fails pending requests and emits the exit event

## Out of scope

Styling beyond basic layout, sidebar/session list, persistence, settings, the browser pane,
computer use, the codegraph proxy, LiteLLM, any file outside the list above. Do not run the
real `codex app-server` or make network calls; there is no network in your sandbox.
