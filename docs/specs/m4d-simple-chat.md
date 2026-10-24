# M4d: Chat mode (a simple chatbot, no project)

## Goal

A **Chat** mode for plain conversations, like the basic ChatGPT or Claude chat. There's
no workspace, no file access and no coding tools: just a model from the provider
registry, streaming answers, attachments, and a chat history separate from coding
projects.

It's one of three top-level modes, switched from the sidebar header:
- **Code:** today's projects/threads
- **Chat**
- **Work:** M4f; render a placeholder tab if M4f hasn't landed

## Required

### 1. Mode switcher

- A segmented control at the top of the sidebar: Code · Chat · Work.
- Persisted per window (app settings).
- ⌘1 / ⌘2 / ⌘3 select modes, only if those aren't already bound (check the shortcuts
  settings and avoid conflicts).

### 2. Chats list and storage

- Chats aren't tied to a workspace. Store them under `<app data>/chats/` via a dedicated
  "chats" pseudo-workspace that the Code mode never lists.
- Sidebar in Chat mode: "New chat", search, pinned, then chats grouped Today / Previous
  7 days / Older. Rename, pin, delete (moves to an in-app Trash with 30-day restore; no
  permanent delete without a second confirm).

### 3. Engine

- Run chats on the **pi** engine with **no tools**:
  - pass pi's flag to disable built-in tools (check pi 1.4.0's CLI for `--tools` /
    `--no-tools`, and use the form that results in zero tools)
  - no MCP connectors
  - still load `taucode-gate` (defence in depth: block every tool call)
  - cwd = an empty per-chat scratch dir under `<app data>/chats/scratch/<chatId>` (0700)
- System prompt: a short neutral chat persona; don't reuse the coding system prompt if pi
  allows replacing it (use `--system-prompt` / `--append-system-prompt` per pi's CLI).
- Model picker uses registry models (all enabled providers). Default to the agent
  policy's default model, with a separate optional `chatDefaultModel` setting. Switching
  models mid-chat works like threads today (fork mechanism).
- Optional per-chat "Web" toggle, **off by default**: when on, allow only the
  taucode-browser read-only tools (`browser_navigate`, `browser_snapshot`,
  `browser_take_screenshot`). Hide it if the browser connector is disabled.
- Attachments: images (paste/drag) when the model supports images (registry capability
  flag if present; else allow and let the provider error surface), and text-like files
  (≤ 1 MB, inlined as fenced text with filename).

### 4. UI

- A clean centered conversation column (max-width ~760px): markdown, code blocks with
  copy, regenerate last answer, edit-and-resend the last user message (forks from that
  point), stop.
- No diff/git/terminal panels in Chat mode.
- Empty state: model picker plus 3 example prompts.
- Title auto-generated from the first exchange (reuse the existing thread-naming
  utility).
- Phone/tablet layouts work.

### 5. Export

Per chat, "Export as Markdown" (save dialog). Uses only local data.

## Tests

- **Rust:** chat spawn args (zero tools, no MCP, gate loaded, scratch cwd), storage
  separate from workspaces, trash/restore, model switching.
- **Frontend:** mode switcher and persistence, chat list grouping/search/pin, composer
  with attachments, regenerate/edit-resend, the web toggle hidden when the browser is
  disabled, export.

## Files

Backend pi spawn options (`shared/pi/**`) for the no-tools chat profile, new
`src-tauri/src/chats/**` if needed, Tauri commands, `src/features/chat/**` (new),
sidebar/layout/app shell for the mode switcher, `src/services/tauri.ts`, `src/types.ts`,
settings.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME` and the verification command as in the M4a spec
  (worktree-relative).
- Don't run `tauri build`.

## Out of scope

Projects, tools beyond the read-only web toggle, voice.
