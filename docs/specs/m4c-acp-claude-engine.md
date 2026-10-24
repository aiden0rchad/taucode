# M4c: ACP engine with Claude Code as the first ACP agent

## Goal

A third engine, `claude` (Claude Code over the Agent Client Protocol), selectable per
thread and as the default like `pi` and `codex`. With it, a user can run Claude Code
inside taucode, using **only an API key from taucode's provider registry**. No claude.ai
login and no OAuth, ever.

Its threads look and behave like other threads:
- streaming messages, reasoning, tool calls, diffs, plans
- approvals in taucode's UI, mapped from taucode access modes
- MCP connectors (codegraph, browser, computer use)
- resume after restart
- file checkpoints (M4b turn hooks)

## Pinned adapter (already committed)

- `apps/taucode/engines/claude-acp/{package.json,package-lock.json}` pins
  `@agentclientprotocol/claude-agent-acp@0.89.1` (Apache-2.0). The bin is
  `node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js`.
- It bundles `@anthropic-ai/claude-agent-sdk` (including the native `claude` binary for
  darwin-arm64).
- `node_modules` is installed locally for reading the adapter source. At runtime the
  engine is installed by `shared/engine_runtime.rs` from the lockfile, the same way as
  `pi` / `browser`: add `"claude-acp"` there (manifest `package-lock.json`, executable
  path above).
- **Read the adapter source** to confirm method names and payloads before coding:
  - `dist/acp-agent.js`: initialize, newSession, prompt, setMode, setModel, loadSession,
    fork if present, providers
  - `dist/hide-claude-auth.js`, `dist/paths.js`
  - the ACP SDK types in `node_modules/@agentclientprotocol/sdk`
- Don't rely on the network.

## Auth and isolation (hard requirements)

- **Always** launch with `--hide-claude-auth`. Under it, the adapter refuses
  sessions/turns a claude.ai subscription would pay for.
- **Environment:** build the child env from an allowlist (PATH, HOME, LANG, TMPDIR, the
  ones below). Never inherit `ANTHROPIC_*`, `CLAUDE_CODE_OAUTH_TOKEN*`, `CLAUDE_*` from
  the parent.
  - `CLAUDE_CONFIG_DIR=<app data>/claude/<workspaceId>`, created with 0700. It's
    isolated, so the user's `~/.claude` (and any login there) is never read.
  - **Auth from the registry provider selected for the thread:**
    - `ProviderKind::Anthropic` → `ANTHROPIC_API_KEY=<key from keychain via the existing
      providers::token path>`, plus `ANTHROPIC_BASE_URL` if the provider has one
    - `OpenaiCompatible` / other kinds the user marks as "Anthropic-compatible
      (/v1/messages)" (add a provider flag `anthropicCompatible: bool` to
      `ProviderEntry`, default false, with a Settings checkbox) → `ANTHROPIC_BASE_URL`
      plus `ANTHROPIC_AUTH_TOKEN`
    - any other provider → the engine is unavailable with a clear error ("Claude Code
      needs an Anthropic or Anthropic-compatible provider")
  - secrets go only into the child env (never args or files) and are added to the
    existing redaction list
  - optional `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`
- If the adapter returns `authRequired`, show "Claude Code rejected the API key (no
  claude.ai login is used)". Never surface or trigger the adapter's login auth methods.

## Engine implementation

`shared/acp/**`: a generic ACP client, JSON-RPC 2.0 over stdio with newline-delimited
messages, plus a `ClaudeAcpEngine` implementing `EngineBackend` (`shared/engines.rs`).

### Process and session

- **Process lifecycle:** one adapter process per workspace (like pi), restarted when the
  provider configuration fingerprint changes and no turn is running (mirror
  `PiEngine::spawn` patterns: idle shutdown, secrets, emit).
- **`initialize`:**
  - send protocolVersion, `clientCapabilities: { fs: { readTextFile: false,
    writeTextFile: false }, terminal: false }`, so Claude uses its own tools and our
    permission flow
  - record `agentCapabilities` (loadSession, fork, model/mode support)
- **Threads:**
  - `start_thread` → `session/new {cwd, mcpServers}`
  - `resume_thread` → `session/load` when supported, else a fresh session with a note
  - persist the `threadId → sessionId` mapping plus the title/preview in taucode
    metadata; `list_threads` / `read_thread` serve from that (and from replayed
    `session/load` updates)
- **Turns:**
  - `send_turn` → `session/prompt` (text, plus image attachments if the composer
    provides them and the agent advertises promptCapabilities.image)
  - `interrupt` → `session/cancel`
  - `steer` → the adapter's steering extension if advertised, else unsupported
- **Models:**
  - `list_models` returns registry models of the selected provider only (the registry is
    the only model source)
  - apply via `session/set_model` (or the adapter's documented equivalent), else
    `ANTHROPIC_MODEL` at spawn
- `fork` → ACP fork if advertised, else an "unsupported for this engine" error. Side
  chats / rewind check this capability.
- `compact` → unsupported for now.

### MCP connectors

- Pass the enabled connectors (codegraph, taucode-browser, taucode-computer, and user
  connectors) as `mcpServers` (stdio and http forms), from the same connector registry
  pi uses (`shared/connectors`).
- Apply the same per-connector allow/deny lists. Built-in connectors keep their existing
  approval semantics via the permission mapping below.

### Translation (`session/update` → codex v2 items and events, like `shared/pi/translate.rs`)

| ACP update | Emitted as |
|---|---|
| `agent_message_chunk` | `agentMessage` delta |
| `agent_thought_chunk` | `reasoning` |
| `tool_call` / `tool_call_update` with kind `execute` | `commandExecution` (command, output, exit code) |
| kind `edit` with diff content | `fileChange` with unified diff |
| kind `read` / `search` / `fetch` / other | `dynamicToolCall`, or `mcpToolCall` when the title/raw input names an MCP tool `mcp__server__tool` |
| Claude Task subagents | `collabAgentToolCall` (whatever the adapter marks as subagent/task), so the M4a Agents panel can show them |
| `plan` | `plan` item |
| `usage` / token info, if present | the token usage events the UI already consumes |

Emit `turn/started` and `turn/completed` (with stopReason mapping), and errors.

### Permissions

`session/request_permission` → taucode's approval flow (the same UI events pi and codex
use).

**Access-mode mapping:**
- **read-only:**
  - set the session mode to plan if the agent advertises it
  - reject any edit/execute/MCP-mutating permission automatically, with a reason
  - allow reads
- **ask:** forward every permission request to the UI, with the options mapped
  (allow once / allow always for session / reject)
- **full-access:**
  - auto-allow edit/execute within the workspace
  - **still forward** browser and computer-use connector calls according to their
    connector approval settings (they're `ask` by default; same `connectorDecision` rules
    as pi's gate, ported to Rust or reused)

Never pass bypassPermissions / dangerous modes.

### Settings and UI

- Engine picker: "Claude Code (ACP)", next to pi and Codex; shows availability and the
  reason when unavailable (no compatible provider, Node missing, runtime installing).
- The model picker for claude threads lists the selected provider's models.
- Engine status/errors appear as for other engines.
- Add `EngineKind::Claude` (serde `"claude"`) to `types.rs` and the TS types; migration:
  unknown engine → default.

## Tests

- **Fake ACP agent fixture** (a small Node or Python script under
  `src-tauri/tests/fixtures/`) speaking ACP:
  - initialize/new/prompt with streamed updates
  - a permission request
  - tool calls of each kind
  - cancel
  - load
  - an `authRequired` error
- **Rust tests:**
  - env allowlist: no `ANTHROPIC_*`/`CLAUDE_*` inherited; `CLAUDE_CONFIG_DIR` isolated;
    `--hide-claude-auth` always present; the key only in env
  - provider mapping (Anthropic, Anthropic-compatible, unsupported)
  - translation table
  - permission mapping per access mode, including connector approvals
  - `thread → session` persistence and resume
  - cancel
  - an auth error message that never offers login
- **Frontend:** engine picker availability, the provider "Anthropic-compatible" checkbox,
  claude-thread model picker.
- **Smoke (optional, skip if no network):** spawn the real adapter with an empty key and
  assert `initialize` succeeds and returns capabilities.

## Files

- `apps/taucode/src-tauri/src/shared/acp/**` (new), `shared/engines.rs` (router),
  `shared/engine_runtime.rs`, `types.rs`, provider settings types, Tauri command wiring,
  tests and fixtures
- frontend: `src/features/settings/**` (engine and provider), `src/features/models/**`
  if needed, `src/types.ts`, `src/services/tauri.ts`
- Don't modify `engines/claude-acp/package*.json` (already pinned), `computer/**` or the
  pi extensions.

## Constraints and verification

- No new crates or npm deps in the app.
- Offline. Always set `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (§Verification). The known
  sandbox-only failure `tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`
  may be excluded.
- Don't run `tauri build`.

## Out of scope

Other ACP agents (Gemini CLI, goose), cross-engine handoff, conversation rewind for ACP
(file rewind comes from M4b), and compaction.
