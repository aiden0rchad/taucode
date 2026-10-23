# M1-C: engine-neutral providers, models, subagent policy and engine router

## Goal

taucode must be provider- and model-agnostic. Today `apps/taucode` (CodexMonitor-based) assumes
codex everywhere: one `codex app-server` per workspace, codex's `model/list` as the only model
source, and codex-only config. Make taucode **own** providers, models and subagent policy,
and route threads to **engines** behind one interface:

- **pi** (earendil-works/pi) will be the default engine. Its adapter arrives in the next
  spec (M1-D1); here, only reserve it.
- **codex** remains an optional engine, refactored to sit behind that interface.

The UI keeps rendering the codex v2 event model (items, turns, approvals), which is the
app's **canonical event model**. Each engine adapter produces it.

Done = the verification command passes. Live tests are run outside your sandbox.

## 0. No accounts, no logins (hard requirement)

taucode is a straight interface to the user's own providers and models. It has **no sign-in
of any kind**: no ChatGPT/OpenAI account, codex login, account switching or plan/rate-limit
displays, and no "connect your account" flows. All model access goes through providers in
the registry (section 1).

- **Remove from frontend and backend** (Tauri commands, daemon RPC methods, hooks,
  components, menu items, onboarding steps, and the tests that encode them):
  - codex login/logout (`codex_login`, `codex_login_cancel`, `account/login/*`)
  - account read/switching (`account_read`, `useAccountSwitching`, `useHomeAccount` and
    the home account card)
  - rate limits and plan (`account_rate_limits`, `useThreadRateLimits`,
    `account/rateLimits/updated`, `account/updated`)
  - ChatGPT apps/connectors (`apps_list`, `codex_apps`)
  - anything else that only exists to serve a signed-in OpenAI account. Search for it; don't
    rely on this list being complete.

  Ignore those notifications explicitly if an engine still sends them. Per-thread token usage
  (`thread/tokenUsage/updated`) stays: it's the user's own usage data.
- **Keep** local, account-free features: git/worktrees, local gh-CLI-based GitHub panels,
  terminal, prompts, skills, the self-hosted remote daemon, dictation.
- **Codex engine runs fully isolated.** It uses a taucode-managed `CODEX_HOME`
  (`<app data>/engines/codex-home`), never `~/.codex`, so no `auth.json` and no user config,
  MCP servers or plugins leak in.
  - taucode writes that home's `config.toml` (or passes `-c` overrides) with:
    - `forced_login_method` unset and no credentials store
    - `model_provider` = the chosen registry provider
    - the registry providers
    - `requires_openai_auth = false` on every provider
    - `apps`, `plugins`, `analytics` and `feedback` disabled, update checks off, no
      `chatgpt_base_url`
  - Look up the exact keys and values in codex's `codex-rs/config/src/config_toml.rs` at
    `/Users/rolandleyco/taucode-refs/codex/codex-rs` (codegraph-indexed).
  - The OpenAI provider kind is reached with an API key like any other provider.
  - If codex still asks for login with this setup, surface its error; never fall back to a
    login flow.

## 1. Provider registry (Rust `types.rs` + `src/types.ts`, persisted in app settings)

`AppSettings.providers: Vec<ProviderEntry>`:

- `id`: slug `[a-z0-9_-]{1,32}`, unique
- `name`
- `kind`: one of `openai`, `anthropic`, `google`, `openrouter`, `groq`, `mistral`, `xai`,
  `deepseek`, `zai`, `ollama`, `lmstudio`, `openai-compatible`
- `baseUrl`: optional for hosted kinds (each kind has a default), required for
  `openai-compatible` (covers LiteLLM, vLLM, any proxy)
- `apiStyle` (only for `openai-compatible`): `chat-completions` (default) or `responses`
- `auth`:
  - `{ "type": "none" }`
  - `{ "type": "apiKey" }`: the key lives in the macOS keychain
  - `{ "type": "env", "var": "…" }`
  - `{ "type": "command", "command": "…", "args": [...] }`: stdout is the token
- `models: Vec<ProviderModel>`: fetched or manually added, each
  `{ id, displayName?, enabled, contextWindow?, supportsTools?, supportsImages? }`
- `modelsFetchedAt?`, `enabled`, `extraHeaders?` (non-secret)

Kind table: default `baseUrl`, models-listing request and auth header per kind. Encode it
once in Rust and unit-test it.

| Kind | Default baseUrl | List models | Auth header |
|---|---|---|---|
| openai | `https://api.openai.com/v1` | `GET /models` | `Authorization: Bearer` |
| anthropic | `https://api.anthropic.com/v1` | `GET /models` | `x-api-key` + `anthropic-version: 2023-06-01` |
| google | `https://generativelanguage.googleapis.com/v1beta` | `GET /models` (ids come as `models/<id>`; strip the prefix; follow `nextPageToken`) | `x-goog-api-key` |
| openrouter | `https://openrouter.ai/api/v1` | `GET /models` | Bearer |
| groq | `https://api.groq.com/openai/v1` | `GET /models` | Bearer |
| mistral | `https://api.mistral.ai/v1` | `GET /models` | Bearer |
| xai | `https://api.x.ai/v1` | `GET /models` | Bearer |
| deepseek | `https://api.deepseek.com` | `GET /models` | Bearer |
| zai | `https://api.z.ai/api/paas/v4` | `GET /models` | Bearer |
| ollama | `http://localhost:11434/v1` | `GET /models` | none |
| lmstudio | `http://localhost:1234/v1` | `GET /models` | none |
| openai-compatible | (required) | `GET /models` | Bearer |

**Secrets:**
- Store them in the macOS keychain via the `keyring` crate (already a dependency): service
  `dev.taucode.desktop`, account `provider:<id>`.
- Commands: `provider_set_api_key`, `provider_has_api_key`, `provider_delete_api_key`.
- A key is never returned to the frontend, logged, persisted in settings, or placed in
  command-line args. It goes only to an engine child process's environment.
- Put keychain access behind a trait, with an in-memory fake for tests.
- Deleting a provider deletes its key.

**Test & fetch models** (command `provider_test_and_fetch(id)`):
- performs the kind's list request with the right auth, 15 s timeout
- returns `{ ok, latencyMs, models: [...], error? }`
- error strings must say what to fix (401 → "API key rejected", connection refused → "Is the
  server running at <url>?", 404 on /models → "This endpoint doesn't list models; add model
  IDs manually")
- merges results into `models`, preserving the user's `enabled` flags and manual entries
- `reqwest` is already a dependency

## 2. Model catalog

A model is addressed everywhere as `provider/model` (a `ModelRef { providerId, modelId }`).
`list_models(workspaceId)` returns enabled models across all enabled providers, each tagged
with the engines that can run it (see the capability rule in 4), plus codex's own
`model/list` models under a synthetic provider `codex-default` when the codex engine is
available.

## 3. Subagent policy

`AppSettings.agentPolicy` with a per-workspace override `WorkspaceSettings.agentPolicy:
Option<AgentPolicyOverride>`, where every field is optional:

- `defaultModel: ModelRef` (main thread default) and `defaultEngine: "pi" | "codex"`
  (default `"pi"`)
- `subagents.enabled` (default true)
- `subagents.maxConcurrent` (1–16, default 4): how many may run at the same time
- `subagents.maxPerTurn` (1–32, default 8): how many one turn may spawn in total
- `subagents.maxDepth` (1–3, default 1)
- `subagents.allowedModels: Vec<ModelRef>` (empty = any enabled model) and
  `subagents.defaultModel: Option<ModelRef>`
- `subagents.roles: Vec<{ name, description, model: Option<ModelRef>, toolAccess: "read-only" | "full", instructions }>`.
  Seed the roles `scout` (read-only, fast model), `planner` (read-only) and `worker` (full).

Validation: role models must be in `allowedModels` when that list is non-empty; a role or
default model whose provider is disabled is an error shown in Settings.

## 4. Engine router

Introduce an `Engine` abstraction inside the backend, so `WorkspaceSession` no longer *is* a
codex process.

- `trait EngineBackend` covers what the app needs:
  - `start_thread(params) -> Thread`, `resume_thread`, `read_thread`, `list_threads`,
    `list_history` (turns/items pages)
  - `send_turn(input, model?)`, `steer`, `interrupt`, `compact`, `fork(model?)`, `set_name`
  - `respond_to_server_request`, `list_models`, `shutdown`
  - an event stream that emits **canonical (codex v2-shaped) app-server messages**, exactly
    what the frontend consumes today
- `CodexEngine`: wraps today's code with no behaviour change. It is fed by the registry:
  - **Providers:** for each enabled provider that codex can use, append
    `-c model_providers.<id>={ name, base_url, wire_api = "responses", supports_websockets = false, … }`.
    Codex can use: kind `openai`, kind `openrouter`, kind `ollama`, and `openai-compatible`
    with `apiStyle = "responses"`.
    Auth mapping:
    - `apiKey` → `env_key = "TAUCODE_PROVIDER_<ID>_KEY"`, set on the child env from the
      keychain
    - `env` → that var
    - `command` → `auth = { command, args, timeout_ms = 5000, refresh_interval_ms = 300000 }`
  - **Subagent policy:** `-c agents.max_threads=<maxConcurrent>`,
    `-c agents.max_depth=<maxDepth>`, and `-c agents.default_subagent_model="<model>"` when the
    default subagent model is codex-usable. Roles: a codex role config file per role,
    generated in the app data dir and referenced via
    `-c agents.<role>={ description = …, config_file = "<path>" }`; the role file sets
    `model` and `model_provider`.
  - **TOML quoting:** as rigorous as `codegraph.rs`. Never put secrets in args.
- `PiEngine`: a stub that returns "pi engine not yet available" (M1-D1 implements it). Do
  register the `"pi"` kind and route to it.
- **Routing:**
  - `thread/start` takes an `engine` (default from policy)
  - the session keeps `threadId → engine`
  - methods with a `threadId` go to the owning engine
  - `list_threads` merges all engines, sorted by recency; each thread carries `engine`
- **Switching models:**
  - engine `pi` (later): any model, same thread (pi's `set_model`)
  - engine `codex`: same provider → per-turn model; different provider → fork with
    `modelProvider` + `model`, which needs a confirmation in the UI
  - model not usable by the thread's engine → offer "Continue in a new <engine> thread"
    (handoff, later); for now disable it with a tooltip

## 5. UI

- **Settings → Providers** (new section, existing patterns):
  - list with enabled toggles; add / edit / delete
  - kind picker with presets
  - auth selector; masked API-key field that only writes ("Saved in Keychain" state +
    replace/remove)
  - **"Test & fetch models"** button with a spinner, latency, the error text from (1), and the
    fetched list with per-model enable toggles plus "Add model ID manually"
  - an "engines" line per provider (pi / codex)
- **Settings → Agents** (extend the existing section):
  - default engine and model
  - subagent limits: max concurrent, max per turn, max depth
  - allowed-models multi-select (from the catalog)
  - default subagent model
  - roles editor
  - a per-project override in the Projects section
- **Composer model picker:**
  - grouped by provider, shows the thread's engine
  - disables models the engine can't run (with a reason)
  - the codex cross-provider fork confirmation shows "~N tokens re-sent", from the latest
    `thread/tokenUsage/updated`
- **Engine badge** on threads in the sidebar and the thread header; a new-thread
  engine/model choice in the composer when a thread is empty.

## Tests

- Rust:
  - kind table: URLs, headers, Google prefix stripping and paging
  - test-and-fetch error mapping: put the HTTP call behind a small trait (request in →
    status/body/error out) and test with a fake; no sockets, no network
  - merge preserving the user's flags
  - keychain fake; delete cascades to the key
  - codex arg generation for providers, policy and role files, with no secrets in args
    (assert)
  - policy validation and override merging
  - router dispatch by `threadId`; `list_threads` merge
  - pi stub behaviour
  - all existing 413+ tests still pass
- vitest: Providers section (presets, masked key, test & fetch success and failure
  rendering, manual model add), Agents policy editor (bounds, allowed-models validation,
  roles), picker grouping and disabled reasons, codex fork confirmation, engine badges.

## Constraints

- Offline sandbox: deps are pre-fetched. Always set
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. **Add no dependencies.**
- Follow `apps/taucode/AGENTS.md` and the codegraph wiring that landed in the previous commit
  (`src-tauri/src/codegraph.rs`): its `-c` args must keep working with the new codex
  engine. Codegraph applies to every engine; keep it engine-neutral.
- Don't touch: `packages/protocol/**`, fixtures, `crates/**`, `docs/**`, `.mcp.json`,
  `.claude/**`, `.codex/**`.
- Never log, persist, or transmit secrets except to the keychain and the child process env.

## Verification

From `/Users/rolandleyco/taucode-m1w`:

```
export CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home TMPDIR=$PWD/.tmp && mkdir -p $TMPDIR && (cd crates/tau-mcp-proxy && cargo test) && (cd apps/taucode/src-tauri && cargo build --bins && cargo test) && (cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint)
```

`tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use` can't bind localhost in
your sandbox; if it's the only failure, report it. Lint: 0 errors, no new warnings.

## Out of scope

The pi adapter (M1-D1), the pi approval and subagent extensions (M1-D2), cross-engine handoff,
the usage/cost ledger, the browser pane, computer use.
