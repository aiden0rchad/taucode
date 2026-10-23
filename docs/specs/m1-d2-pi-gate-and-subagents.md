# M1-D2: pi approval gate and policy-enforced subagents

## Goal

With pi as the default engine (M1-D1), add the two taucode-owned pi extensions that make it
safe and controllable:

1. **`taucode-gate`**: every bash/write/edit tool call is checked against the thread's
   access mode, and approvals appear in taucode's existing approval UI (the same inline
   approve/deny as codex threads, "accept for session" included).
2. **`taucode-subagents`**: a subagent tool, derived from pi's example (MIT), that enforces
   the user's **subagent policy** from M1-C:
   - max concurrent subagents
   - max per turn
   - max depth
   - allowed models
   - default subagent model
   - roles with model and tool access

   Subagent progress renders in the app's subagent view.

Done = the verification command passes. A live test is run outside your sandbox.

## Ground truth

- pi docs (`/Users/rolandleyco/taucode-refs/pi/packages/coding-agent/docs/`):
  `extensions.md`, `rpc-extension-ui.md`, `security.md`, `json.md`.
- Source examples: `examples/extensions/permission-gate.ts`, `protected-paths.ts`, and
  `examples/extensions/subagent/` (`index.ts`, `agents.ts`, README).
- Fixtures in `apps/taucode/src-tauri/tests/fixtures/pi-1.4.0/`:
  - `rpc_session.jsonl`: a real `extension_ui_request` (select) from the permission gate,
    answered with `extension_ui_response`
  - `rpc_subagents.jsonl`: a real parallel run of the example subagent tool (two scouts on
    a non-OpenAI model). It shows `tool_execution_start/update/end` for tool `subagent`.
    `partialResult.details.results[]` and `result.details` carry per-agent `agent`, `task`,
    `model`, `exitCode`, `stopReason`, `usage` (input/output/cache/cost/contextTokens/turns)
    and `messages`.
- The canonical subagent items in the generated protocol: `collabAgentToolCall` and
  `subAgentActivity` in `packages/protocol/src/v2/ThreadItem.ts`, plus the related types,
  and how the app renders them (the `agentsStates` handling in
  `src-tauri/src/backend/app_server.rs` and the item renderers added in the 0.164 port).

## Required changes

1. **Extensions ship with taucode** in `apps/taucode/engines/pi/extensions/` (TypeScript,
   loaded by pi; no new npm dependencies). `PiEngine` copies or links them into the managed
   agent dir's `extensions/` at startup.
2. **`taucode-gate`:**
   - **Access mode** per thread: `read-only` | `ask` | `full-access`. The host writes
     `<agent dir>/taucode/threads/<sessionId>.json` `{ accessMode, sessionAllow: [...] }` and
     can change it mid-thread. The extension re-reads it on each tool call (cheap, small file).
     Map the app's existing access-mode control to it.
   - On `tool_call` for `bash`, `write` and `edit`:
     - `full-access` → allow
     - `read-only` → block `write`/`edit`/mutating bash with a clear reason, and allow bash
       commands on a small read-only allowlist (e.g. ls, cat, rg, grep, git status/diff/log)
     - `ask` → anything not covered by `sessionAllow` requests approval through
       `ctx.ui.select` with options exactly `["Allow", "Allow for session", "Block"]` and a
       title that starts with the marker `taucode:approval:`, followed by compact JSON
       `{ toolCallId, tool, command | path, cwd, diffPreview? }`
     - "Allow for session" adds a normalized rule (command prefix or path) to
       `sessionAllow`
     - a non-interactive context blocks by default
   - **Adapter side** (`PiEngine`, from D1's generic mapping):
     - recognize the marker and emit the **canonical** approval server request instead of
       the generic user-input one:
       - bash → `item/commandExecution/requestApproval`, with `itemId` = the
         commandExecution item for that `toolCallId`
       - write/edit → `item/fileChange/requestApproval`
     - map the decisions: `accept` → "Allow", `acceptForSession` → "Allow for session",
       `decline`/`cancel` → "Block"
     - when the request resolves, emit `serverRequest/resolved`
   - Protected paths: always ask (even in `full-access`) for writes to `.git/`, `.env*` and
     the managed pi agent dir.
3. **`taucode-subagents`:** derived from the example; keep its single/parallel/chain modes
   and streaming.
   - **Policy source:** the host writes the effective policy (app + workspace override,
     from M1-C) to `<agent dir>/taucode/policy.json`:
     `{ maxConcurrent, maxPerTurn, maxDepth, allowedModels: ["provider/model"], defaultModel, roles: [{ name, description, model, toolAccess, instructions }] }`.
   - **Roles:** generated as agent definitions from `roles`. Don't read user or project
     agent dirs (`agentScope` user/project is disabled). `toolAccess: read-only` →
     `--tools read,grep,find,ls`.
   - **Model choice:**
     - the tool accepts an optional `model` per task
     - resolution order: task model → role model → `defaultModel` → the parent's model
     - every resolved model must be in `allowedModels` when non-empty; otherwise fail that
       task with a message listing the allowed models, so the main model can retry
     - pass models as `--provider <p> --model <m>`
   - **Limits:**
     - concurrency is capped at `maxConcurrent`
     - a per-run counter (reset on `agent_start`) rejects spawns beyond `maxPerTurn`
     - depth comes from the env `TAUCODE_SUBAGENT_DEPTH`: children get +1 and refuse to
       spawn at `maxDepth`
   - **Safety:** children inherit the parent thread's access mode, but they're
     non-interactive. In `ask`, a child with `toolAccess: full` may write only inside the
     workspace (bash still blocked unless allowlisted). In `read-only`, children are
     read-only. Write this down in the tool description so the model knows.
   - Child processes reuse the same node/cli, agent dir and env (keys come via env, as in
     D1) and `--no-session`. Kill children on abort.
4. **Adapter mapping for subagents.** Map tool `subagent` to the canonical subagent items:
   - `tool_execution_start` → a `collabAgentToolCall` (or `subAgentActivity`, whichever the
     app renders best; justify in the report) in progress, listing tasks
   - `tool_execution_update` `partialResult.details.results[]` → per-agent states
     (running / completed / failed, model, last message preview, usage)
   - `tool_execution_end` → completed, with each agent's final output and usage
   - per-agent `usage.cost` also feeds `thread/tokenUsage/updated`
5. **Settings UI:** the Agents policy editor from M1-C now also says, per field, which engine
   enforces it (pi: everything; codex: max concurrent, max depth, default model, roles). Show
   the effective (merged) policy for the current workspace.

## Tests

- Extension logic as pure TypeScript functions (decision table, rule normalization,
  policy resolution, model allow-list, limits), tested with `node --test` (add an
  `engines/pi` `test` script; no new deps).
- **Adapter:**
  - marker → canonical approval requests and decision round trip, including "Allow for
    session"
  - non-marker UI requests still use the generic path
  - replay `rpc_subagents.jsonl` → canonical subagent items with correct per-agent states
    and final outputs
- **Host:** `policy.json` and per-thread access files written on change; protected-path
  rules.
- **vitest:** the policy editor's engine annotations; approvals from pi threads use the same
  components as codex threads.
- Wire `node --test` for `engines/pi` into the verification command (below).

## Constraints

- Offline sandbox: dependencies are pre-fetched. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. **Add no dependencies.** Type
  imports from `@earendil-works/pi-coding-agent` (already installed) are fine.
- No network, real providers or keychain in tests.
- Follow `apps/taucode/AGENTS.md` and the D1 `PiEngine` structure.
- Don't touch: `packages/protocol/**`, fixtures (read-only), `crates/**`, `docs/**`,
  `.mcp.json`, `.claude/**`, `.codex/**`.

## Verification

From `/Users/rolandleyco/taucode-m1w`:

```
export CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home TMPDIR=$PWD/.tmp && mkdir -p $TMPDIR && (cd crates/tau-mcp-proxy && cargo test) && (cd apps/taucode/engines/pi && npm test) && (cd apps/taucode/src-tauri && cargo build --bins && cargo test) && (cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint)
```

(Known sandbox-only failure: `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`.)

## Out of scope

OS-level sandboxing (`@anthropic-ai/sandbox-runtime`; later), cross-engine handoff, the
usage ledger UI, the browser pane, computer use.
