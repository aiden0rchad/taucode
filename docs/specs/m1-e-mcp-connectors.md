# M1-E: MCP connectors registry (engine-neutral, no OAuth)

## Goal

Users add MCP servers ("connectors") once in taucode, and every engine (pi and codex) gets
them. Like providers, connectors authenticate **only** with what the user enters: env
values, headers or bearer tokens stored in the macOS keychain. There are **no OAuth or
account-connection flows** (taucode-wide rule; see M1-C §0).

taucode's own built-in servers (codegraph now; browser and computer use later) are delivered
the same way, so every capability reaches every engine as MCP tools.

Done = the verification command passes. Live tests are run outside your sandbox.

## Model

`AppSettings.mcpServers: Vec<McpServerEntry>`, with `WorkspaceSettings.mcpServers`
overrides (enable/disable per workspace, plus workspace-only servers):

- `id`: slug, unique; reserved ids `codegraph`, `taucode-browser`, `taucode-computer`
- `name`, `enabled`
- `transport`:
  - `{ "type": "stdio", "command", "args": [...], "cwd"?, "env": { NAME: { "value": "…" } | { "secret": true } } }`
  - `{ "type": "http", "url", "headers": { NAME: { "value": "…" } | { "secret": true } }, "bearer": { "secret": true }? }`
- `toolAllowlist?: [names]`, `toolDenylist?: [names]`
- `approval: "auto" | "ask"`: whether every tool call from this server needs approval

Secrets: keychain account `mcp:<id>:<NAME>` (or `mcp:<id>:bearer`), using the same trait
and fake as M1-C. A secret is never written to settings, config files or args. It reaches
the server only through the engine child's env, or headers the engine builds from env
references. An entry whose server requires OAuth (an HTTP 401 with `WWW-Authenticate: Bearer`
and no bearer configured) shows "This server requires OAuth sign-in, which taucode doesn't
support. Use a token/API-key configuration if the server offers one."

## Engine delivery

- **pi:** generate `mcp.json` `mcpServers` from the effective set. Secrets go in via env
  interpolation if pi's MCP config supports it; otherwise via the pi process env, inherited
  by stdio servers (confirm in pi `docs/mcp.md` / the pi-mcp source). HTTP headers come from
  env references.
- **codex:** `-c mcp_servers.<id>={ command, args, env_vars = [...] }` for stdio and
  `{ url, bearer_token_env_var, http_headers / env_http_headers }` for http. Check the exact
  keys in codex's `config_toml.rs` / mcp config types in the codegraph-indexed refs. Secrets
  are set on the child env and referenced by name only.
- Allow/deny lists and per-server approval map to each engine's mechanism. In pi that's
  taucode-gate (M1-D2), which gets an `mcp__<id>__*` rule.
- `codegraph` moves into this registry as a built-in, non-deletable entry (it can still be
  toggled), with today's proxy wiring.

## UI

- **Settings → Connectors:**
  - list with toggles and per-workspace overrides
  - add/edit with transport type, command/args/env or URL/headers; masked secret fields
    ("Saved in Keychain"); allow/deny lists; approval mode
  - **"Test connection"**: spawn or connect, run the MCP handshake and `tools/list`, show
    the tools (name + description) with latency or a readable error. This reuses rmcp; the
    app already links `tau-mcp-proxy`, which depends on rmcp client features.
- **Thread view:** tool calls from connectors render as `mcpToolCall` items. They already do
  for codex; make sure pi's `mcp__<server>__<tool>` mapping (D1) shows the connector's name.
- Remove or replace any remaining UI that reads MCP servers from codex's own config.

## Tests

- Config generation for both engines and both transports; secrets never appear in files
  or args (assert); reserved ids; override merging.
- Test-connection logic against an in-process fake MCP server (rmcp, stdio pair; no
  sockets); OAuth-required detection message.
- vitest: Connectors section, masked secrets, test-connection results/errors, per-workspace
  toggles.

## Constraints

Same as M1-C/D1/D2: offline, no new dependencies, `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`,
don't touch protocol/fixtures/crates/docs/agent configs, no secrets anywhere but the keychain
and child env.

## Verification

Same command as M1-D2.

## Out of scope

The browser and computer-use servers themselves (M2/M3), OAuth of any kind, a connector
marketplace.
