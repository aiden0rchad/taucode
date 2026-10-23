# M0: codegraph MCP proxy with per-thread sessions and resets

## Goal

codegraph's MCP server remembers, per MCP connection, which source it has already sent, and
replaces repeats with "Already sent earlier in this conversation" pointers. When codex
compacts or reverts a thread, that memory becomes wrong: the agent no longer has the code,
but codegraph thinks it does.

Build `tau-mcp-proxy`, a small MCP server that sits between codex and an upstream MCP server
(codegraph):

- It keeps **one upstream session per codex thread**. Codex puts the thread id in every tool
  call's `_meta.threadId` (see `with_mcp_tool_call_ids_meta` in codex
  `codex-rs/core/src/mcp_tool_call.rs`).
- It **drops and recreates** a thread's upstream session when that thread's reset epoch
  changes.

Then wire it into the desktop host. Codex gets the proxy as its `codegraph` MCP server, and
the host bumps a thread's epoch on `thread/compacted`, `thread/reverted` and successful
`thread/rollback`.

Done = the verification command passes. A real-codex run is done outside your sandbox, not by
you.

## Files

Create or modify only these:

- `crates/tau-mcp-proxy/` (already created with deps; do not add crates):
  - `src/lib.rs`: proxy core (session map, epochs, forwarding)
  - `src/main.rs`: CLI entry
  - `src/epoch.rs`: epoch file read/write, shared with the host
  - `src/bin/tau-mcp-fake-upstream.rs`: test-only fake upstream MCP server
  - `tests/proxy.rs`
- `apps/desktop/src-tauri/Cargo.toml`: add a path dependency on `tau-mcp-proxy` (lib only,
  for `epoch`). This is the only allowed dependency change. It resolves offline because the
  crate's deps are already in CARGO_HOME.
- `apps/desktop/src-tauri/src/codegraph.rs` (new): codex config args + epoch bumping.
- `apps/desktop/src-tauri/src/engine/mod.rs`: allow extra args on `EngineConfig` if needed.
- `apps/desktop/src-tauri/src/lib.rs`: enable codegraph when available; bump epochs from the
  event stream and after `thread/rollback`.
- `apps/desktop/src-tauri/examples/m0_smoke.rs`: flags below.

Off-limits: `packages/protocol/**`, `docs/PLAN.md`, `.mcp.json`, `.claude/**`, `.codex/**`,
`AGENTS.md`, other dependency lists.

## Constraints

### Reuse, don't reinvent

Use `rmcp` 3.7 (official Rust MCP SDK, already a dependency) for both sides:
- server over stdio (`transport-io`)
- client to the upstream child process (`transport-child-process`, `TokioChildProcess`)

Reference implementations are on disk and indexed by codegraph. Query them with
`codegraph explore "<names>"` run from inside that folder:

- `/Users/rolandleyco/taucode-refs/rust-sdk`: rmcp itself plus examples (`examples/servers`,
  `examples/clients`); `ServerHandler`, `ClientHandler`, `TokioChildProcess`, `RequestContext`
- `/Users/rolandleyco/taucode-refs/codex/codex-rs/rmcp-client`: how codex drives rmcp as a
  client, including `_meta` on `tools/call`

Copy patterns from these instead of inventing new ones.

### Proxy behaviour (`tau-mcp-proxy -- <upstream cmd> [args...]`)

- At startup, spawn a **default** upstream session and complete its MCP handshake. Answer
  codex's `initialize` with the upstream's server info, protocol version, `instructions` and
  tools capability. The instructions matter: codegraph uses them to steer the agent.
- `tools/list`: forward to the default session (pass pagination cursors through).
- `tools/call`: key = `_meta.threadId` (string) if present, else `"default"`.
  - Before forwarding, read the key's epoch (below). If it differs from the epoch the
    session was created at, drop that session and spawn a fresh one first.
  - Forward the request unchanged, `_meta` included, and return the upstream result
    unchanged.
  - Concurrent calls on different keys must not block each other. Concurrent calls on the
    same key share one session, and session creation per key happens once (no duplicate
    spawns).
- If an upstream session's transport dies, return an MCP error for that call and remove the
  session, so the next call on that key spawns a new one. **No automatic retry** of tool
  calls.
- At most 32 live per-thread sessions (LRU eviction), and sessions idle for 30 minutes are
  closed. Dropping a session must terminate its child process.
- Forward `notifications/tools/list_changed` from the default session to the client.
- stdout is protocol only; logs go to stderr. Exit cleanly when stdin closes, killing all
  children.
- Works with no `TAUCODE_STATE_DIR` set: epochs are then always 0, so per-thread sessions
  still apply but never reset. This makes it usable as a generic proxy, e.g. for Claude Code.

### Epochs (`src/epoch.rs`)

- Location: `$TAUCODE_STATE_DIR/codegraph-epochs/<threadId>`, containing a decimal `u64`.
  Missing file = 0. Sanitize the thread id for use as a file name: allow `[A-Za-z0-9._-]`,
  replace anything else.
- `read_epoch(dir, key) -> u64` (cheap: one small file read per tool call) and
  `bump_epoch(dir, key) -> u64` (read, +1, write temp file, rename atomically). The host uses
  `bump_epoch`.

### Host wiring (`apps/desktop/src-tauri`)

`codegraph.rs`:

- `CodegraphSetup { proxy_bin: PathBuf, upstream: Vec<String> /* default ["codegraph","serve","--mcp"] */, state_dir: PathBuf }`
- `fn codex_config_args(&self) -> Vec<OsString>`: returns `-c` overrides that register the
  server as `codegraph`:
  `mcp_servers.codegraph.command="<proxy_bin>"`,
  `mcp_servers.codegraph.args=["--", <upstream...>]`,
  `mcp_servers.codegraph.env={ TAUCODE_STATE_DIR = "<state_dir>" }`.
  Values are TOML, so quote and escape strings properly, including paths with spaces.
  Pass them after `app-server` in the codex args.
- `fn discover(app_state_dir) -> Option<CodegraphSetup>`:
  - proxy from env `TAUCODE_MCP_PROXY_BIN`, else a `tau-mcp-proxy` next to the current
    executable
  - `codegraph` must be on PATH
  - if either is missing, log one stderr line and return `None` (codegraph disabled)
- `fn on_engine_message(&self, msg: &Value)`: bump the epoch for `params.threadId` on
  notifications `thread/compacted` and `thread/reverted`.

`lib.rs`: call `discover` with the Tauri app data dir. Feed every engine notification to
`on_engine_message`. When `engine_request` succeeds for method `thread/rollback`, bump that
thread's epoch.

### Smoke example additions

- `--codegraph-proxy <path>`: enable codegraph via the proxy. State dir is a new temp dir,
  printed at start.
- `--second-prompt <text>` and `--compact-between`: after the first turn completes, send
  `thread/compact/start` for the thread, wait for `thread/compacted`, then run
  `--second-prompt` on the same thread. Without `--compact-between`, run it straight after
  the first turn.
- For every completed `mcpToolCall` item, print one line: server, tool, result text length,
  and `dedup=yes` if the result text contains `Already sent earlier in this conversation`,
  else `dedup=no`.
- `--auto-approve` must also approve MCP tool-call approval requests and elicitations, using
  the shapes in `packages/protocol/src/ServerRequest.ts` and the matching `*Response.ts`.
  Answer unknown request types with error -32601 as before.

## Verification

From `/Users/rolandleyco/taucode-m0` (offline; always set CARGO_HOME):

```
export CARGO_HOME=/Users/rolandleyco/taucode-m0/.cargo-home TMPDIR=$PWD/crates/tau-mcp-proxy/target/tmp && mkdir -p $TMPDIR && (cd crates/tau-mcp-proxy && cargo clippy --all-targets -- -D warnings && cargo test) && (cd apps/desktop/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test) && (cd apps/desktop && npm run build)
```

`tests/proxy.rs` drives the real proxy binary as an MCP client (rmcp client over
`TokioChildProcess`), with `tau-mcp-fake-upstream` as the upstream (`CARGO_BIN_EXE_*`). The
fake exposes a tool `whoami` that returns JSON `{ "session": <unique per process>, "calls":
<n>, "meta": <the _meta it received> }`, plus `instructions` and one other tool. Tests:

1. `initialize` through the proxy shows the upstream's instructions and both tools.
2. Two calls with `threadId=A` get the same session; `threadId=B` gets a different one.
3. Bumping A's epoch makes the next A call get a new session; B is unchanged.
4. A call without `_meta` uses the default session.
5. `_meta` reaches the upstream unchanged.
6. If the upstream for A exits (give the fake a `crash` tool), that call returns an error and
   the next A call works on a new session.
7. `epoch.rs` unit tests: missing → 0, bump increments, file-name sanitizing.

## Out of scope

Any UI changes, LiteLLM, other MCP servers, changes to codegraph, network access, running
the real codex.
