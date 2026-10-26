# M6: engine × model A/B eval harness

## Goal

A built-in, reproducible way to compare **engines** (pi, oh-my-pi, Claude Code ACP, codex,
custom) and **models** on the same coding tasks, measuring quality and cost. It's used to:
- pick default models per agent role: e.g. a cheap worker vs. a strong orchestrator
- answer "which harness burns fewer tokens for the same result", the user's pi/omp/zcode
  A/B question
- gate upgrades of pi, omp, codegraph or a model: re-run a pinned subset and flag
  regressions

Done means:
- a task suite with deterministic checks
- a runner reachable from the app (Evals page), the daemon RPC and a CLI
- stored results with comparisons
- a regression gate command
- "Set as default for role" from the results

## Existing pieces (read first)

- **Daemon RPC** (`src-tauri/src/bin/codex_monitor_daemon/**`), used by the reviewer's
  live harness: `get_app_settings`, `update_app_settings`, `add_workspace`,
  `connect_workspace`, `start_thread {workspaceId, engine, model}`, `send_user_message
  {workspaceId, threadId, text, accessMode}`, `checkpoint_request`,
  `respond_to_server_request` (approvals).
  - Events stream as JSON-RPC notifications: `item/completed`, `turn/completed` with
    params.threadId.
- **Engines:** routing by protocol in `src-tauri/src/shared/engines.rs` and the engine
  registry in `shared/engine_registry.rs`.
  - Built-ins: `pi`, `omp`, `codex`, `claude`. omp is disabled by default
    (`settings.engines[]` entry with `enabled: true` turns it on).
- **Per-turn usage** is already extracted for diagnostics (see
  `crates/taucode-diag-mcp` / the `diagnostics` module: `agent.turn` events carry
  tokensIn, cachedTokens, tokensOut, costUsd (nullable), toolCalls, durationMs; plus
  `engine.ready` and `engine.first_token`).
  - **Reuse that extraction code**, but the runner must collect usage **directly**,
    whether or not the diagnostics feature is enabled.
- **Codegraph:** a per-project setting `codegraphEnabled` (`src/types.ts`).
- **Agent roles:** `agentPolicy.subagents.roles: {name, description, model, toolAccess,
  instructions}[]` and `agentPolicy.defaultModel`.

## Design

### 1. Task suite (`apps/taucode/evals/`)

- **Fixture repos** are generated deterministically by a script into a temp/work dir.
  Never commit node_modules; only small source files. Under `evals/repos/<name>/`, write
  the source files directly (no network):
  - `ts-lib`: a tiny TypeScript library with `node --test` tests (no deps beyond Node 22+
    built-ins: use `node --experimental-strip-types` or plain JS with JSDoc if simpler)
  - `py-cli`: a small Python CLI with stdlib `unittest`
  - `web-page`: a static HTML+JS page, for the browser task
- **Tasks:** `evals/tasks/*.json`, schema-validated (`evals/task.schema.json`):
  ```json
  {"id":"ts-fix-off-by-one","repo":"ts-lib","category":"bugfix","tags":["smoke"],
   "prompt":"The `range(n)` helper returns n+1 items. Fix it and make the tests pass.",
   "setup":["optional shell steps run before the agent, e.g. apply a failing test"],
   "check":{"command":"node --test","expectExit":0},
   "extraChecks":[{"file":"src/range.js","contains":"..."}],
   "requires":[],"timeoutSec":600,"maxTurns":3}
  ```
  - `requires` may list `browser` or `computerUse`. Those tasks are skipped unless that
    feature is enabled and the engine supports it.
- **About 24 tasks:** 6 questions (answer checked by the agent's final text containing
  required tokens, with a deterministic `answerContains` check), 8 bug fixes, 6 small
  features, 2 refactors, 1 browser task (read a value from the local page served by a
  tiny local static server started by the runner on 127.0.0.1), 1 computer-use task
  (optional).
- Tag about 8 as `smoke` for the gate.
- Tasks must be solvable by a competent model in 1–3 turns. Checks must be
  deterministic. **Include, for each task, a reference solution patch**
  (`evals/solutions/<id>.patch`), and a test that applying the solution makes the check
  pass and the unmodified repo makes it fail. That proves every task is valid.

### 2. Runner (Rust, `src-tauri/src/shared/evals/**`; shared by the app and the daemon)

- **Input:**
  - a run plan: task filter (ids, tags, categories)
  - a matrix: `engines[] × models[] (provider/model refs) × codegraph[on|off] × repeats`
    (default 1)
  - concurrency (default 2, max 6)
  - optional budget caps: max total tokens, max cost (stop early when exceeded)
- **Per cell:**
  1. Materialize the fixture repo into `<data>/evals/work/<runId>/<cellId>/` with
     `git init` plus an initial commit, run `setup`, add it as a workspace (hidden from
     the sidebar: `kind: "eval"`), and set `codegraphEnabled` per the cell.
  2. Start a thread on the engine/model, send the prompt in `full-access` mode
     (auto-approve), up to `maxTurns` turns, with a follow-up nudge if the check fails
     after turn 1 ("The check `<command>` still fails: <tail of output>. Fix it.").
  3. Collect per-turn usage and timing from engine events: effective input (in +
     cached), cached, output, cost (null when unknown), tool calls, errors, engine ready
     ms, first-token ms, wall time.
  4. Run the check (and extraChecks) with a timeout and an env allowlist. Record
     pass/fail and the last 2 KB of output.
  5. Tear down: stop the thread, remove the workspace entry, and keep the work dir only
     if `keepWorkdirs` is set (default: delete on pass, keep on fail for inspection;
     capped at the 20 most recent).
- Results persist to `<data>/evals/runs/<runId>.json`, written incrementally so a crash
  keeps partial results. Each holds:
  - the plan
  - the environment: app version; the engine versions (pi/omp/acp/codex), resolved via
    the engine runtime status; codegraph version; the model ids
  - every cell result
- **Concurrency and safety:**
  - cells run in isolated work dirs, never in user projects
  - the runner refuses to start if no matrix engine is enabled and ready
  - cancellation stops in-flight threads
  - it doesn't modify user settings except temporary eval workspaces, which are removed
    afterwards; per-cell codegraph is set on the eval workspace only
- **Approvals:** eval threads auto-accept tool approvals only inside the eval work dir.
  Any request targeting a path outside it is denied and recorded as an error.

### 3. Interfaces

- **Daemon RPC:**
  - `eval_request {action: "suites" | "plan" | "start" | "status" | "cancel" | "runs" | "run" | "delete" | "gate", params}`
  - Progress notifications go out as `eval/progress` events.
- **Tauri command:** the same handler (`eval_request`) for the app.
- **CLI:** `codex_monitor_daemonctl` gains
  `evals run --engines pi,omp --models lab/GLM\ 5.3 --tags smoke [--codegraph both] [--repeats N] [--json]`
  and `evals gate --baseline <runId|latest-passing> --tags smoke [--max-pass-drop 0.1] [--max-token-increase 0.25]`.
  - The CLI talks to a running daemon or starts one in-process, using the same data dir
    semantics as the daemon.
  - `gate` exits non-zero on regression and prints a compact table.

### 4. Evals page (Settings-level or a top-level view; put it under **Settings → Evals** with a "Open full view" that uses the main area)

- **New run:**
  - choose the suite filter (tags/categories, with a count preview)
  - the engines (enabled ones), the models (from the registry, multi-select), codegraph
    on/off/both, repeats, concurrency, budget caps
  - Shows "N cells · ~X tasks · est. tokens (from previous runs if any)" and needs an
    explicit **Start** (these runs spend real provider tokens).
- **Live progress:** a cell grid (task × config) with status dots, plus a cancel button.
- **Results:**
  - a comparison table per config (engine/model/codegraph): pass rate, median effective
    tokens per task, cache hit %, total cost (or unknown), median first-token and wall
    time, tool calls, errors
  - sortable, with best values highlighted
  - Expand a config to see per-task outcomes, with the check output tail and links to
    open the eval thread transcript (read-only).
- **Compare two runs** (A/B): deltas per config and per task, with regressions in red.
- **Set as default for role:** from a config row, pick a role (orchestrator/default
  model, or a named subagent role from `agentPolicy`) and write the model into the
  policy, with a confirm.
- **History:** a run list with date, plan summary, pass rate and delete.
- Use the settings design system (SettingsPage/Group/Row, Select, Toggle, segmented),
  the existing tokens and z-index scale, and a compact tabular layout that works at
  1024px.

### 5. Docs

`docs/evals.md`:
- how to add tasks (schema, the reference patch requirement)
- how to run from the UI and the CLI
- what's measured and how (effective tokens, cache rate, cost unknown)
- the gate workflow for engine/model upgrades: run the gate before merging an updated
  pi/omp/codegraph pin

## Tests

- Task schema validation over all tasks.
- **Every task's reference patch makes its check pass, and the base fails.** Run in a
  temp dir and offline, with no model calls. (This is the key suite-validity test.)
- **Runner with a fake engine:** use the existing fake pi fixture (`tests/fixtures/`) or
  add a fake engine that applies the reference patch when prompted. Check:
  - cell lifecycle
  - usage aggregation
  - follow-up nudge on check failure
  - budget stop
  - cancellation
  - incremental persistence
  - work-dir cleanup policy
  - the approval sandbox denying outside paths
- The gate math: pass-rate drop, token increase thresholds, exit codes.
- RPC and Tauri command plumbing.
- Frontend:
  - plan count preview
  - Start requires confirmation
  - the progress grid updates from events
  - the comparison table sorting/highlighting
  - A/B deltas
  - Set as default for role writes `agentPolicy` correctly
  - preview fixtures with a finished run, an in-progress run and an A/B pair
    (`?settings=evals`)

## Constraints and verification

- Offline. No new crates or npm deps unless they're already in the offline cache; prefer
  std plus existing deps. `CARGO_HOME=/Users/rolandleyco/taucode-m6/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Don't touch packaging/bundling (`tauri.conf.json` bundle, signing scripts) or the
  theme files: parallel work streams own those.
- Verification:
  - the command in `docs/specs/m3-computer-use.md`, run from
    `/Users/rolandleyco/taucode-m6` with this worktree's `CARGO_HOME` (the permitted
    tailscale exclusion)
  - `npm run build -- --mode uipreview`
  - the evals reference-patch test
- No live model calls; the reviewer runs a live smoke eval. No browser or computer use.
  Don't commit.
