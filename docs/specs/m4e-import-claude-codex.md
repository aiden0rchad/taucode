# M4e: import projects and chats from Claude desktop and Codex desktop

## Goal

The user can move their history into taucode with **Settings → Import**:
- projects / workspaces
- coding threads
- Cowork tasks
- MCP server definitions
- plain chats

Sources: Claude desktop (Code tab + Cowork + claude.ai export), the Codex desktop/CLI, and
ChatGPT export.

Rules:
- **Read-only** on every source; copies only, originals never modified.
- **Idempotent:** re-importing skips what's already imported.
- **Never reads credentials:** `auth.json`, `.credentials.json`, keychain, OAuth tokens,
  cookies, `buddy-tokens.json`, …

## Sources and formats (verified on the user's Mac; write fixtures, don't read real user data in tests)

### A. Codex (desktop and CLI share `~/.codex`, or `$CODEX_HOME`)

- **Projects:** `state_5.sqlite` (pick the highest `state_*.sqlite`). Tables:
  - `projects(id, name, metadata, position, created_at_ms, updated_at_ms)`
  - `project_roots(project_id, position, path)`
  - `threads(id, rollout_path, cwd, title, name, created_at_ms, updated_at_ms, archived,
    model, model_provider, project_id, first_user_message, preview, …)`
  - Open read-only (`?mode=ro&immutable=1`, or copy the file plus `-wal` to a temp dir
    first). If the schema differs, fall back to scanning rollouts.
- **Transcripts:**
  - `sessions/YYYY/MM/DD/rollout-<ts>-<id>.jsonl` and `archived_sessions/`
  - `session_index.jsonl` (`{id, thread_name, updated_at}`)
  - The first line `session_meta.payload` has `id, cwd, cli_version, model_provider, …`
    **plus `creator_user_id` / `creator_account_id`**: strip those (and any `account`,
    `user`, `email` fields) when copying.
- **MCP:** `config.toml` `[mcp_servers.<name>]` → taucode connectors. Import command,
  args and url. **Never import env/header values**: list their keys, and mark the
  connector "needs secrets" for the user to fill in (stored in the keychain via the
  existing connector secret flow). Skip `mcp-oauth*` entirely (no OAuth in taucode).
- **Mapping:**
  - each project → a taucode workspace per root path (skip missing paths, but list them)
  - each thread → a **codex-engine** thread: copy the rollout into taucode's isolated
    codex home `sessions/` tree, so codex app-server lists and resumes it natively; keep
    the title and archived state
  - threads with no project → a workspace by `cwd`; if the cwd is missing → a read-only
    archived history

### B. Claude desktop (`~/Library/Application Support/Claude`)

- **Code tab sessions:**
  - `claude-code-sessions/<acct>/<org>/local_<id>.json` with `{sessionId, cliSessionId,
    cwd, originCwd, title, createdAt, lastActivityAt, model, isArchived, …}`
  - transcript: `~/.claude/projects/<encoded cwd>/<cliSessionId>.jsonl`. The encoding
    replaces `/` (and `.`) with `-`; resolve by scanning the dirs for
    `<cliSessionId>.jsonl` rather than re-encoding.
  - **Mapping:** workspace by `cwd`, plus a thread on the **claude** engine (M4c): copy
    the jsonl into taucode's isolated `CLAUDE_CONFIG_DIR/projects/<encoded>/`, so ACP
    `session/load` can resume it.
  - If the claude engine isn't available yet, import as a read-only transcript thread
    (convert to v2 items for display) with "Continue in pi" (fork into a pi session with
    a summarized context; see D).
- **Also** plain Claude Code CLI sessions in `~/.claude/projects/*/*.jsonl` that no
  desktop session references: offer them as a separate group.
- **Cowork tasks:**
  - `local-agent-mode-sessions/<acct>/<org>/local_<id>.json` with `{sessionId,
    cliSessionId, cwd, userSelectedFolders[], title, createdAt, lastActivityAt, model,
    isArchived, initialMessage, fsDetectedFiles[], …}`
  - sibling dir `local_<id>/` with `audit.jsonl`, `uploads/`, `outputs/`
  - **Mapping** → taucode **Work** tasks (M4f):
    - folder = the first `userSelectedFolders` entry
    - transcript from `audit.jsonl`: parse defensively; unknown event kinds are kept as
      raw collapsible entries
    - copy `outputs/` and `uploads/` into the task's attachments dir
    - if M4f isn't merged yet, store as read-only Work history
- **MCP:** `claude_desktop_config.json` `mcpServers` → connectors (same no-secrets rule).
- **Never read:** `buddy-tokens.json`, `ant-did`, `Cookies*`, `Local Storage`, `Partitions`,
  `config.json` auth fields, `ccd-ids.json`.

### C. Chat exports (user picks a .zip or conversations.json)

- **claude.ai export:** `conversations.json`, an array of `{uuid, name, created_at,
  updated_at, chat_messages:[{sender:"human"|"assistant", text, content[], created_at,
  attachments[], files[]}]}`. Use `content[]` text blocks when present.
- **ChatGPT export:** `conversations.json`, an array of `{id, title, create_time,
  update_time, mapping:{node:{message:{author:{role}, content:{parts[]}, create_time},
  parent, children}}, current_node}`. Follow the `current_node` → `parent` chain for the
  main branch.
- **Mapping** → **Chat mode** chats (M4d): convert to pi session files so they can be
  continued. Assistant turns are recorded as assistant messages from model
  `imported/<source>`; the next user turn uses the chat's selected registry model.

### D. Fallback for unsupported continuation

Any imported transcript that can't be resumed natively shows as read-only, with
"Continue": it creates a new pi thread whose first context message is a compact
transcript summary (≤ 8k tokens, truncating the middle) plus the last 10 messages
verbatim.

## UI

**Settings → Import** shows detected sources with counts:
- "Codex: 12 projects, 340 threads"
- "Claude Code: 8 projects, 95 sessions"
- "Cowork: 14 tasks"
- "MCP servers: 6"
- plus "Import chats from export file…"

Flow:
1. Each source expands to a tree: project → threads, with checkboxes (default: all
   non-archived) and an "include archived" toggle.
2. Import runs in the background with progress and a summary.
3. An import log is stored at `<app data>/imports/<timestamp>.json`, with an "Undo
   import": it removes what that import created. It never touches sources, and never
   deletes threads the user continued after importing without a confirm.

Idempotency key: `(source, source id)`, stored in `<app data>/imports/index.json`.

## Safety

- Path allowlist per source: only the files listed above.
- Max file size 200 MB per transcript; skip and list larger ones.
- Strip account/user identifiers.
- Run every parser on a background thread with a cancel button.
- No network.

## Tests

- **Fixtures** (synthetic, written in the repo under
  `src-tauri/tests/fixtures/import/**`): a mini `state_5.sqlite` built in the test with
  the schema above, rollouts with `session_meta` account ids (asserted stripped), Claude
  desktop session JSONs + `~/.claude` jsonl, a Cowork dir with audit.jsonl / outputs,
  `claude_desktop_config.json` with env secrets (asserted not imported),
  claude.ai / ChatGPT `conversations.json`.
- Assertions:
  - credential files are never opened (a test helper wraps file opens; asserting the
    denylist)
  - idempotent re-import
  - undo import
  - missing cwd handling
  - ChatGPT branch following
- **Frontend:** detection list, tree selection, progress, summary, undo.

## Files

`src-tauri/src/import/**` (new), Tauri commands, connector registry integration,
`src/features/settings/**` (Import section), `src/services/tauri.ts`, `src/types.ts`.
Read-only use of the codex/claude/pi session writers (call existing helpers; don't change
the engines).

## Constraints and verification

- Use rusqlite only if it's already a dependency; otherwise invoke the system `sqlite3`
  binary read-only, or parse rollouts only. **No new crates.**
- Offline; worktree-relative `CARGO_HOME`; verification as in M4a. Don't run
  `tauri build`.

## Out of scope

Syncing (one-shot import only), importing settings, themes or keybindings, and any
cloud API.
