# M1: wire the codegraph proxy into the taucode app, retire the M0 spike

## Goal

The M0 spike (`apps/desktop`) proved a codegraph setup that gives codex threads codegraph
with safe cross-call dedup. It works by routing codegraph through `crates/tau-mcp-proxy`:

- the proxy keeps one upstream codegraph session per codex thread, keyed by
  `_meta.threadId`
- it resets a thread's session when that thread's epoch file changes
- the host bumps the epoch on compaction
- codegraph runs from a taucode-private index (`CODEGRAPH_DIR=.codegraph-taucode`) with
  `CODEGRAPH_EXPLORE_DEDUP=1`

Move this into the real app (`apps/taucode`, the CodexMonitor-based app) for both the desktop
app and its daemon binary. Then delete `apps/desktop`.

Done = the verification command passes. Live tests against real codex are run outside your
sandbox, not by you.

## Reference implementation (M0, to port, then delete)

- `apps/desktop/src-tauri/src/codegraph.rs`: `CodegraphSetup`, `codex_config_args()` (TOML
  `-c` overrides with careful quoting), `on_engine_message()` (bump on
  `item/completed` where `item.type == "contextCompaction"`, plus legacy `thread/compacted` /
  `thread/reverted`), `discover()`, and their tests
- `crates/tau-mcp-proxy/src/{main.rs,lib.rs,epoch.rs}`: the proxy and epoch files
- `docs/specs/m0-codegraph-proxy.md`: full rationale

Use codegraph (`codegraph explore "<names>"`) to read these and the app code instead of
listing folders.

## Required changes

1. **The proxy runs inside the taucode executables.** No separate binary to ship.
   - In `crates/tau-mcp-proxy`, move the body of `src/main.rs` into a public
     `pub fn run_cli(args: Vec<OsString>) -> anyhow::Result<()>` in `lib.rs`. It builds its
     own tokio runtime and blocks. Keep the `tau-mcp-proxy` binary as a thin wrapper.
   - In `apps/taucode/src-tauri/src/main.rs` and in the `codex_monitor_daemon` binary: if
     `argv[1] == "--tau-mcp-proxy"`, call `tau_mcp_proxy::run_cli(rest of args)` and exit with
     its status, **before** any Tauri/daemon initialisation (and before `fix_path_env`, which
     is not needed there). The remaining args start with `--`, as the proxy CLI expects.
   - Engine-neutral session key: if the env var `TAUCODE_SESSION_KEY` is set, the proxy uses
     it instead of `"default"` as the key for calls without `_meta.threadId`, for epochs too.
     Engines other than codex (pi, next milestone) run one process per session and don't send
     `threadId`, so this keeps their resets independent. Add a test.
2. **Codegraph setup in the backend** (new module `src-tauri/src/codegraph.rs`, shared by the app
   and the daemon like the other `shared`/`backend` modules):
   - `CodegraphSetup { proxy_exe: PathBuf /* std::env::current_exe() */, upstream: ["codegraph","serve","--mcp"], state_dir: <app data dir>/codegraph }`
   - `codex_config_args()` as in M0. Register the server as `codegraph`, with
     `args = ["--tau-mcp-proxy", "--", <upstream...>]` and
     `env = { TAUCODE_STATE_DIR = …, CODEGRAPH_EXPLORE_DEDUP = "1", CODEGRAPH_DIR = ".codegraph-taucode" }`.
   - `discover()`: `codegraph` must be on PATH (after `fix_path_env`); otherwise log once and
     disable.
3. **Spawn integration.** Where `spawn_workspace_session` builds the `codex app-server`
   command:
   - append `codex_config_args()` when codegraph is enabled globally and for that workspace
   - the user's own `codex_args` must still be honoured, and come first
4. **Epoch bumps.** In `route_app_server_message`, call the equivalent of
   `on_engine_message` for every message so compaction bumps that thread's epoch. Verify it
   with a test that replays `src-tauri/tests/fixtures/codex-0.164/lifecycle.jsonl` (it contains
   a real `contextCompaction` item) and asserts that exactly that thread's epoch went from 0
   to 1.
5. **Index lifecycle per workspace.** When a workspace connects and codegraph is enabled:
   - if `<workspace>/.codegraph-taucode` is missing, run
     `CODEGRAPH_DIR=.codegraph-taucode codegraph init` in the background
     (non-blocking, one at a time per workspace, with a timeout)
   - add `.codegraph-taucode/` to `<workspace>/.git/info/exclude` when the workspace is a
     git repo and the line isn't there; never edit `.gitignore`
   - report status `{ state: "indexing" | "ready" | "failed" | "unavailable" | "disabled", detail }`
     through a Tauri command `codegraph_status(workspaceId)` and an event when it changes
6. **Settings and UI** (follow existing settings patterns):
   - `AppSettings.codegraphEnabled` (default true) and
     `WorkspaceSettings.codegraphEnabled: Option<bool>` (unset = follow app setting)
   - In Settings → Projects (or the closest existing per-workspace place): a per-workspace
     toggle, the status line from (5), and a "Rebuild index" button that re-runs
     `codegraph init --force` with the same env
   - In Settings → Codex (or Features): the global toggle and a one-line explanation
   - Changing a toggle takes effect on the workspace's next app-server spawn. Say so in the UI.
7. **Retire the M0 spike.**
   - Delete `apps/desktop` entirely (its UI, engine module, `m0_smoke` example and tests are
     superseded).
   - Keep `crates/tau-mcp-proxy` and its tests.
   - Update any root files that reference `apps/desktop` (e.g. `.gitignore`, `AGENTS.md`, the
     `CLAUDE.md` files).
   - Leave `docs/PLAN.md` and `docs/specs/*` untouched; they're history.

## Tests to add

- `codegraph.rs`: config-arg quoting (port the M0 tests and include the new
  `--tau-mcp-proxy` arg), the enable/disable matrix (global × workspace), and
  `.git/info/exclude` handling (idempotent, creates the file if missing, never touches
  `.gitignore`).
- The fixture-replay epoch test from (4).
- `main` dispatch: a test that the `--tau-mcp-proxy` path is taken without starting Tauri.
  Put the arg check in a small function and unit-test it.
- Frontend: vitest for the settings toggles and status rendering.
- `crates/tau-mcp-proxy` tests must still pass, and the proxy must work when invoked as
  `<exe> --tau-mcp-proxy -- <upstream>`. Add one integration test in the crate that
  calls `run_cli` through the thin binary path.

## Constraints

- Offline sandbox: dependencies are pre-fetched. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. **Add no dependencies.**
  `tau-mcp-proxy` (path) and `rmcp` are already in `apps/taucode/src-tauri/Cargo.toml`.
  Remove `rmcp` from the app's dependencies if you don't need it.
- Follow `apps/taucode/AGENTS.md`.
- Don't touch: `packages/protocol/**`, the fixtures, `.mcp.json`, `.claude/**`, `.codex/**`,
  `docs/**`.
- Never write API keys or secrets anywhere (not relevant here, but don't add any).

## Verification

From `/Users/rolandleyco/taucode-m1w`:

```
export CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home TMPDIR=$PWD/.tmp && mkdir -p $TMPDIR && (cd crates/tau-mcp-proxy && cargo clippy --all-targets -- -D warnings && cargo test) && (cd apps/taucode/src-tauri && cargo build --bins && cargo test) && (cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint)
```

One app test, `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`, can't bind
localhost inside your sandbox. If it is the only failure, report it and stop; it passes
outside. Lint: 0 errors, no new warnings (baseline 5).

## Out of scope

Providers/model switching (next spec), the browser pane, computer use, ACP engines, visual
redesign, running the real codex.
