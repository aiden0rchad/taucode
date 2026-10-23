# M1-D1: the pi engine adapter

## Goal

Implement `PiEngine` behind the `EngineBackend` interface introduced in M1-C, making pi
(`@earendil-works/pi-coding-agent` 1.4.0) taucode's **default engine**. A pi session appears
in the app as a thread. The adapter translates pi's RPC protocol into the app's canonical
(codex v2-shaped) event model, so the existing UI renders pi threads without UI rewrites:
items, streaming, approvals, compaction, token usage, steer and interrupt all work.

pi is natively multi-provider: switching to any model of any enabled provider happens
**in the same thread** (pi `set_model`), with no fork.

Done = the verification command passes. A live test against real providers is run outside
your sandbox.

## Ground truth

- **pi docs**, on disk at `/Users/rolandleyco/taucode-refs/pi/packages/coding-agent/docs/`:
  - `rpc.md`: framing. JSONL, split **only on LF**, not Node `readline`, and keep reading
    stdout continuously.
  - `rpc-commands.md`, `json.md` (events), `rpc-extension-ui.md`
  - `sessions.md`, `session-format.md`, `cli.md`, `models.md`, `models-configuration.md`,
    `providers.md`, `mcp.md`, `settings.md`, `environment-variables.md`
- **Recorded real traffic** (pi 1.4.0, scrubbed) in
  `apps/taucode/src-tauri/tests/fixtures/pi-1.4.0/`:

  | File | Covers |
  |---|---|
  | `rpc_session.jsonl` (`{"dir","msg"}` per line) | get_state; prompt with write + bash tools, streaming text/thinking/toolcall deltas; permission-gate `extension_ui_request` (select) answered with `extension_ui_response`; `set_model` to another model mid-session; steer (`queue_update`); abort (`agent_settled` with `aborted:true` arrives **before** the abort response); successful manual compaction with summary; get_session_stats; get_messages |
  | `rpc_compact_too_small.jsonl` | the same flow where compaction fails with "Nothing to compact (session too small)" (`compaction_end.errorMessage`, `success:false`) |
  | `session_file.jsonl` | pi's on-disk session file for `rpc_session.jsonl` (`session-format.md`) |

- **Reference adapter:** t3code's `packages/provider-pi` (MIT, TypeScript, about 5.9k
  lines) at `/Users/rolandleyco/taucode-refs/t3code/packages/provider-pi`. It maps pi events
  into an app event model. Read its mapping decisions before inventing your own.
- **Canonical event model:** the generated protocol types in `packages/protocol/src`
  (`ServerNotification.ts`, `ServerRequest.ts`, `v2/ThreadItem.ts`, `v2/Thread.ts`,
  `v2/Turn.ts`, …) and the codex fixtures in
  `apps/taucode/src-tauri/tests/fixtures/codex-0.164/`. Emit exactly these shapes.

Use codegraph (`codegraph explore "<names>"`) to look things up; never list big folders.

## Required changes

1. **pi runtime.**
   - `apps/taucode/engines/pi/package.json` pins `@earendil-works/pi-coding-agent@1.4.0`
     (already installed offline, with a lockfile).
   - Run it as `node <engines/pi>/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js --mode rpc …`.
     Allow overrides via env `TAUCODE_PI_CLI` (a path to cli.js) and `TAUCODE_NODE` (a node
     binary).
   - Require node ≥ 22.19. Check the version once and surface a clear, actionable error.
2. **Isolated pi home**, managed by taucode. Never read or write the user's `~/.pi`.
   - **No accounts or logins** (taucode-wide rule, see M1-C §0):
     - never write `auth.json`; never use pi's `/login`, OAuth or subscription-based
       providers (e.g. `openai-codex`, `github-copilot`, token-plan providers)
     - credentials come only from the registry, as API key via keychain→env, env var, or
       command
     - an `extension_ui_request` or provider error asking for login surfaces as an error,
       never as a login flow
   - Set `PI_CODING_AGENT_DIR=<app data>/pi/agent`, `PI_TELEMETRY=0` and
     `PI_SKIP_VERSION_CHECK=1` for every pi process. Sessions go in
     `<app data>/pi/sessions/<workspace id>/`, passed with `--session-dir`.
   - Generate `models.json` from the provider registry (M1-C) whenever the registry
     changes:
     - built-in kinds map to pi's built-in provider ids (openai, anthropic, google,
       openrouter, groq, mistral, xai, deepseek, zai); set only `baseUrl` when the user
       changed it, and list the enabled models
     - `ollama`, `lmstudio` and `openai-compatible` become custom providers with
       `api: "openai-completions"` (or `"openai-responses"` when `apiStyle = responses`;
       check the exact api ids in `models-configuration.md`)
     - auth in `models.json` is **never a literal secret**:
       - `apiKey` → `"$TAUCODE_PROVIDER_<ID>_KEY"` (pi env interpolation), with that env var
         set on the pi child from the keychain
       - `env` → `"$<VAR>"`
       - `command` → `"!<command> <args…>"` with proper shell quoting
       - `none` → a dummy value as pi's docs suggest
     - assert in tests that no keychain value ever appears in a file or args
   - Generate `mcp.json` with the codegraph server when codegraph is enabled (reuse
     `codegraph.rs`): the command is the taucode exe with
     `--tau-mcp-proxy -- codegraph serve --mcp`, and the env has `TAUCODE_STATE_DIR`,
     `CODEGRAPH_EXPLORE_DEDUP=1` and `CODEGRAPH_DIR=.codegraph-taucode`.
     - Set `TAUCODE_SESSION_KEY=<thread id>` on each pi process's env, and confirm in pi's
       `mcp.md` or the pi-mcp source that MCP children inherit it. If they don't, put it in
       the per-thread config instead and explain in the report.
     - Bump the codegraph epoch for the thread on compaction (`compaction_end` without an
       error).
   - Generate `settings.json` with sane defaults (keep pi's compaction defaults, quiet
     startup).
   - Leave `extensions/` empty here; M1-D2 adds taucode's gate and subagent extensions.
3. **Process management.**
   - One pi RPC process per active thread: spawn on start/resume, idle shutdown after 10
     minutes without a running turn, transparent respawn with the session file on the next
     action.
   - Correlate commands by string `id`.
   - Shutdown: close stdin, wait, then kill. This avoids pi's EPIPE crash when killed
     mid-write.
   - Read stdout continuously; stderr goes to the debug log.
   - When a process dies mid-turn, emit `turn/completed` (failed) with the error, and mark
     the thread so the next action respawns it.
4. **Thread API mapping.**
   - Thread id = pi `sessionId`. `Thread.modelProvider` = the current provider id, plus an
     `engine: "pi"` field.
   - `start_thread` (cwd, model): spawn with `--provider/--model` from the requested
     `ModelRef` or the policy default.
   - `send_turn(input, model?)`: if the model differs from the current one, `set_model`
     first, then `prompt` (text + images).
   - `steer` → `steer`; `interrupt` → `abort` (expect `agent_settled{aborted:true}` before
     the response); `compact` → `compact`; `set_name` → `set_session_name`.
   - `fork` → pi's whole-session copy (`clone`; check `rpc-commands.md`) → new thread, then
     optional `set_model`.
   - `list_threads`: scan the workspace's session dir. Use the session header, name and
     first user message for title/preview, and the mtime for recency. Never spawn pi
     to list.
   - `read_thread` / `list_history`: when the process is alive, use `get_messages`;
     otherwise parse the session file. Run both through the **same** translator used for
     live events, with paging by turns as the UI expects.
5. **Event translation** (live and from history; one module, table-driven, unit-tested
   against the fixtures). A canonical *turn* is one prompt run (`agent_start` … `agent_settled`).
   A pi `turn_start/turn_end` is one model call inside it.
   - `agent_start` → `turn/started` (generate the turn id) + `thread/status/changed` active.
     `agent_settled` → `turn/completed`: status `interrupted` if `aborted`, `failed` on error,
     else `completed`; then status idle.
   - user `message_start/end` → `userMessage` item.
   - assistant `text_*` → `agentMessage` item, with `item/agentMessage/delta` per
     `text_delta`.
   - `thinking_*` → `reasoning` item with the matching reasoning delta notification.
   - `tool_execution_start/update/end`:
     - `bash` → `commandExecution`: command and cwd. `partialResult` is cumulative, so
       derive `item/commandExecution/outputDelta` deltas from it. Final output and exit
       status come from the result; `isError` → failed.
     - `write` / `edit` → `fileChange`: write = add/update with content; edit = update with a
       unified diff built from the edit args. Emit `turn/diff/updated` with the turn's
       accumulated diff.
     - `mcp__<server>__<tool>` → `mcpToolCall`.
     - Everything else (read, grep, find, ls, …) → `dynamicToolCall`, with tool name,
       arguments and result.
   - assistant message `usage` (and `cost`) → `thread/tokenUsage/updated`, with last and
     total accumulated and `modelContextWindow` from the model.
   - `compaction_start/end` → a `contextCompaction` item; failure → item failed + an `error`
     notification with pi's message.
   - `auto_retry_start/end` and `extension_error` → user-visible status/error notifications.
   - `extension_ui_request` (`select`, `confirm`, `input`; generic in D1) → the canonical
     user-input server request (the shape of `item/tool/requestUserInput` in
     `ServerRequest.ts`). Map the answer back to `extension_ui_response` (`value`,
     `confirmed` or `cancelled`). Honour the request `timeout`. Fire-and-forget methods
     (`notify`, `setStatus`, `setWidget`, …) go to the debug log.
   - `set_model` success → update the thread's provider/model and notify the UI with the
     closest existing notification for a model change (pick from the generated types; say
     which in the report).
6. **Model picker integration:**
   - for pi threads, every enabled model of every enabled provider is selectable, and
     switching happens in place (no fork confirmation)
   - for pi `kind`s pi doesn't support, disable the model with a reason
7. **Remove the M1-C pi stub.** Default engine = pi (from policy), and codex threads keep
   working unchanged.

## Tests

- **Translator:** replay `rpc_session.jsonl` and `rpc_compact_too_small.jsonl` and assert
  the exact canonical sequence:
  - item types and order; delta reconstruction equals final text/output
  - approval → requestUserInput → `extension_ui_response` round trip
  - `set_model` applied; steer queued; interrupted turn status
  - compaction item completed or failed with the message; codegraph epoch bumped only on
    success
- **History:** parse `session_file.jsonl` into turns/items equal to the live translation of
  the same session (modulo deltas).
- **Process manager**, with a scripted fake pi (Python stdlib, `tests/fixtures/fake_pi.py`):
  - id correlation; `agent_settled`-before-response ordering
  - LF-only framing, including a message whose text contains U+2028
  - graceful shutdown (no kill when stdin close suffices)
  - idle timeout and respawn with the session file
  - crash mid-turn → failed turn
- **Config generation:** `models.json` for every kind and auth type; `mcp.json`;
  `TAUCODE_SESSION_KEY`; no secrets in files or args.
- **vitest:** pi threads render from canonical events (reuse existing pipeline tests);
  in-place model switch from the picker sends `model` on the next turn with no fork dialog;
  engine badge shows "pi".
- All existing tests keep passing.

## Constraints

- Offline sandbox: dependencies are pre-fetched, including `apps/taucode/engines/pi/node_modules`.
  Always set `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. **Add no Rust or npm
  dependencies**; the pi package is the only new runtime and it's already there.
- Tests must not need network, real providers or the keychain. Real pi may be started in
  tests only with a provider that doesn't touch the network; prefer the fake.
- Follow `apps/taucode/AGENTS.md` and the M1-C `EngineBackend` design.
- Don't touch: `packages/protocol/**`, fixtures (read-only), `crates/**`, `docs/**`,
  `.mcp.json`, `.claude/**`, `.codex/**`.

## Verification

From `/Users/rolandleyco/taucode-m1w`:

```
export CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home TMPDIR=$PWD/.tmp && mkdir -p $TMPDIR && (cd crates/tau-mcp-proxy && cargo test) && (cd apps/taucode/src-tauri && cargo build --bins && cargo test) && (cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint)
```

(Known sandbox-only failure: `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`.)

## Out of scope

taucode's approval-gate and subagent extensions (M1-D2), the sandbox extension,
cross-engine handoff, bundling Node for release, the usage ledger UI.
