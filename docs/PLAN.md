# taucode — build plan

A local-first desktop agent harness that feels like Codex desktop / Claude Code desktop
(sidebar of projects and sessions, rich transcript, right-hand tool panes, side chats,
subagents, integrated browser) and adds Agent-S computer use.

Status: plan, nothing built. Checked against `openai/codex` main and `simular-ai/Agent-S`
main on 2026-10-22.

---

## 1. Principles

1. **Don't write the agent loop.** The loop, tool design, compaction, sandbox and patching are
   what make Codex and Claude Code good, and they took large teams years. Embed
   `codex app-server` (Rust, Apache-2.0) as the engine. Our value is the shell, the browser,
   computer use, and the UX around them.
2. **Rust owns the eyes and hands.** Screen capture, input injection, the browser webview and
   the policy gate all live in the Rust host. Python (Agent-S) only *thinks*: it receives a
   screenshot and returns a structured action. It never touches the screen, keyboard or mouse,
   and its output is never `exec`'d.
3. **Everything the agent can do is an MCP tool or an engine item.** The UI renders one
   stream of typed items; no special cases per feature.
4. **Lazy and small.** Python sidecar, grounding model and browser start only when first used.
5. **Model-agnostic by construction.** Providers are configured once in taucode and routed
   per role (main, side chat, subagents, computer-use planner, grounding). Engines sit behind a
   trait: codex is engine #1, and ACP engines (Claude Code, Gemini CLI, goose) plug in so each
   model family can run in the harness it was tuned for. See §7.
6. **Spend tokens on work, not discovery.** CodeGraph answers "where/how is X" in one call
   instead of 20–40 grep/read calls, and the host keeps its dedup state honest across
   compaction and rewinds. Usage is measured per turn and per provider. See §8.

## 2. Architecture

```
┌──────────────────────────── Tauri v2 app (one process) ────────────────────────────┐
│  Web UI (React/TS)  ── Tauri IPC (commands + event channel) ──  Rust host (tau-host) │
│   sidebar / transcript / composer / panes                       ├─ engine supervisor │
│                                                                 ├─ session store     │
│  Browser pane = native child webview (WKWebView / WebView2)     ├─ browser bridge    │
│   └─ injected agent bridge JS  ◄── eval / IPC ──►               ├─ capture + input   │
│                                                                 ├─ policy gate       │
│                                                                 ├─ shadow-git ckpts  │
│                                                                 └─ MCP server (rmcp) │
└──────────────┬───────────────────────────────────────┬─────────────────────┬───────┘
        │ JSON-RPC / ACP over stdio            │ MCP (stdio)         │ MCP (stdio)
 ┌──────▼─────────────────┐           ┌────────▼────────┐   ┌────────▼─────────┐
 │ Engines (Engine trait)  │──MCP────► │ tau MCP server  │   │ cua sidecar (Py) │
 │  codex app-server       │  client   │ browser.*       │   │ Agent-S3 planner │
 │  ACP: Claude Code,      │           │ computer.*      │   │ + grounding call │
 │       Gemini CLI, goose │           │ codegraph proxy─┼─┐ └────────┬─────────┘
 └──────┬─────────────────┘           └─────────────────┘ │          │ HTTP
        │ Responses API / native APIs                       │ ┌────────▼─────────┐
 ┌──────▼──────────────────────────┐   ┌──────────────────▼┐│ UI-TARS-1.5-7B   │
 │ Providers: OpenAI, Ollama, ...  │   │ codegraph daemon   ││ (MLX/vLLM/HF)    │
 │ LiteLLM gateway (lazy sidecar   │   │ per-project SQLite ││                  │
 │ or homelab URL) for Anthropic,  │   │ graph, file watch  │└──────────────────┘
 │ Gemini, OpenRouter, Bedrock...  │   └────────────────────┘
 └─────────────────────────────────┘
```

### Why these choices

| Decision | Choice | Reason |
|---|---|---|
| Shell | Tauri v2 + React/TS | Small footprint; Rust host in-process. The cost: no CDP on macOS (see §5). |
| Engine | `codex app-server` child process | Has threads, turns, `turn/steer`, `thread/queue/*`, `thread/fork`, approvals, `turn/diff/updated`, `turn/plan/updated`, compaction, `thread/rollback`, background terminals, `fs/watch`, `review/start`, `model/list`, skills, MCP client, Seatbelt/Landlock sandbox. Ships generated TS types in `app-server-protocol/schema/typescript`. |
| Models | taucode provider registry → codex `model_providers` + LiteLLM gateway + ACP engines | codex now speaks only the Responses wire API, so non-OpenAI models reach it through a Responses-compatible gateway. Details in §7. |
| Code intelligence | CodeGraph (MIT) daemon behind our MCP proxy | 62% fewer tokens, 88% fewer tool calls in its own benchmarks; Rust parse kernel, SQLite, sub-second incremental sync. Details in §8. |
| Computer use | Agent-S3 (`gui-agents` 0.3.2) as planner, UI-TARS-1.5-7B grounding | S3 is the stable library; S4 (released 2026-10-20) is a research system for evolving interfaces, not something to embed yet. |
| MCP in Rust | `rmcp` crate | Official Rust SDK. |
| Capture / input | ScreenCaptureKit (`screencapturekit` crate) / CGEvent on macOS; `windows-capture` + SendInput on Windows | Lets us exclude our own windows from screenshots by window ID. |
| Terminal | `portable-pty` + xterm.js | Same as every serious terminal pane. |

### Subscriptions and terms

- Agent-S and UI-TARS are free to run locally; the planner model still costs API money unless
  local.
- Claude subscriptions can't be used in third-party apps: Claude models go through an API key
  (directly or via LiteLLM).
- codex app-server supports ChatGPT sign-in. Check OpenAI's terms before shipping that in a
  distributed app; API keys are always fine.

## 3. Repo layout

```
taucode/
  apps/desktop/            Tauri app: src-tauri/ (thin) + ui/ (React, Vite)
  crates/
    tau-host/              state, IPC commands, event fan-out to UI windows
    tau-engine/            Engine trait; codex adapter (spawn, JSON-RPC, reconnect); ACP adapter
    tau-providers/         provider registry, keychain secrets, role routing, price table,
                           codex config + LiteLLM config generation, usage ledger
    tau-codegraph/         codegraph binary management, per-project index lifecycle, MCP proxy
    tau-browser/           child-webview manager + agent bridge JS + browser.* tools
    tau-computer/          capture, input, HUD/occlusion, kill switch, computer.* loop
    tau-policy/            approval rules for shell/file/browser/computer actions
    tau-checkpoint/        shadow git (GIT_DIR outside the worktree) snapshots + restore
    tau-mcp/               rmcp server exposing browser.* and computer.* to the engine
  sidecars/cua/            Python, uv-managed: Agent-S3 wrapped as an MCP "brain"
  packages/protocol/       codex TS types, vendored from a pinned codex version
  docs/
```

Pin one codex version. Vendor its schema. Regenerate types on upgrade and run contract tests
against recorded JSON-RPC fixtures, because the protocol moves fast and many methods are
marked experimental.

## 4. UI spec (matches the two reference screenshots)

Three columns plus an optional fourth, all resizable and collapsible.

**Left sidebar**
- New chat, search, project groups with sessions nested under them (and under subfolders).
- Per-session state dot: running, waiting on you (approval/question), unread, done, error.
- Pin, archive, rename, drag between groups, "Show more".
- Account + model badge at the bottom.

**Center: transcript**
- Virtualized list (TanStack Virtual) of engine items, each with its own renderer:
  agent message (markdown, Shiki code), reasoning summary (collapsed), command execution
  (streaming output, exit code, approve/deny inline), file change (inline diff, approve),
  MCP tool call (args/result, progress), subagent call (status, opens its thread),
  computer-use run (live thumbnail, step list), browser action, plan/todo list.
- Task list pinned under the latest message from `turn/plan/updated`.
- Next-step box at turn end (recap + one question + buttons).
- Composer: image/file attachments and paste, `@` file mentions (fuzzy search over `fs/*`),
  `/` skills and commands, model + reasoning-effort picker, permission mode (Ask / Auto /
  Read-only → codex approval policy + sandbox mode), Stop, and while a turn runs, Enter
  steers (`turn/steer`) and Shift+Enter queues (`thread/queue/add`).
- Header: session title, branch/worktree, and buttons to open each pane.

**Right panel: tool panes** (the "Tools" launcher from screenshot 1)
- Changes: turn and session diffs (`turn/diff/updated`), per-file revert, checkpoint timeline.
- Terminal: user terminal tabs plus the agent's background terminals.
- Files: tree + viewer (CodeMirror 6, read-mostly).
- Browser: see §5.
- Side chat: `thread/fork` of the current thread in its own pane; answers questions without
  touching the main thread; "send to main" button.
- Subagents: tree of spawned agents (from codex `CollabAgent*` items) with live state; click
  to open the child transcript in the pane or a split.
- Environment/Context: working dir, sandbox mode, token usage meter
  (`thread/tokenUsage/updated`), MCP connectors and their status.

**Global**
- Command palette (⌘K), shortcuts matching Codex desktop where sensible (⌘⇧G Changes,
  ⌃` Terminal, ⌘P Files, ⌥⌘S Side chat).
- Split view of two sessions; drag a session out into its own window.
- Light/dark tokens from day one.

Stack: React 19, Vite, Tailwind, Radix primitives, Zustand (one store fed by the engine event
stream), TanStack Virtual, Shiki, xterm.js, CodeMirror 6. No Next.js, no Electron.

## 5. Integrated browser

**v1: native child webview + injected bridge.** Tauri v2 multiwebview (`unstable` feature)
mounts a second webview in the window. On every page an init script installs a bridge that
exposes:

| Tool | Implementation |
|---|---|
| `browser.navigate / back / forward / reload` | webview API |
| `browser.snapshot` | walk DOM + ARIA, keep visible/interactive nodes, assign `ref_N`, return compact YAML (same idea as Playwright MCP and Claude's `read_page`) |
| `browser.find(query)` | substring match over the snapshot |
| `browser.click / type / select / scroll(ref)` | dispatch events from the bridge; scroll element into view first |
| `browser.screenshot` | native: `WKWebView.takeSnapshot` (objc2) / WebView2 `CapturePreview`; optional set-of-mark overlay drawn before capture |
| `browser.text` | readability-style main-content extraction |
| `browser.console / network` | bridge hooks `console.*` and `fetch`/`XHR` |
| `browser.eval` | gated by policy, off by default |

Tabs are multiple child webviews; the user sees and can take over the same tab the agent uses.
Site allowlist per project and confirmation for submit/purchase-like clicks live in
`tau-policy`.

**v2: "Use my Chrome" mode.** For sites that need Chromium or the user's real profile, attach
to Chrome over CDP and wrap `@playwright/mcp` as an MCP server. No screencast canvas in v1.

Known limits of v1: WebKit on macOS (no Chrome extensions), cross-origin iframes need a
per-frame bridge, synthetic events are `isTrusted=false` (some sites ignore them; fall back to
computer use for those).

## 6. Computer use (Agent-S3)

### Split of responsibilities

```
engine ──calls──► computer.run_task(instruction, app?, max_steps)   (MCP tool, long-running)
                    │
tau-computer loop:  capture screen (exclude our windows) ─► cua.predict(instruction, png, ax_tree?)
                    ◄── structured action {kind:"click", x, y, button, clicks} | {kind:"type", text} | …
                    policy check ─► (approve if needed) ─► execute via CGEvent/SendInput
                    emit progress (item/mcpToolCall/progress) + thumbnail to UI
                    repeat until done/fail/max_steps/kill switch
```

### The sidecar (`sidecars/cua`)

- MCP server (Python `mcp` SDK, stdio) with tools `predict`, `reset`, `health`.
- Subclass `OSWorldACI` so each action method (`click`, `type`, `scroll`, `drag_and_drop`,
  `hotkey`, `hold_and_press`, `open`, `switch_applications`, `wait`, `done`, `fail`) returns a
  JSON action instead of a `pyautogui` code string. Grounding (description → x,y) still runs
  inside the sidecar against UI-TARS.
- `call_code_agent` and `set_cell_values` disabled: code work goes to the main engine.
- Coordinates come back in grounding space (1920×1080 for UI-TARS-1.5-7B); Rust maps them to
  the captured display, accounting for Retina scale and multi-monitor offsets.
- Planner model configurable (any vision model via the same provider config); grounding
  endpoint configurable: local MLX/vLLM UI-TARS, HF Inference Endpoint, or off.
- Python shipped via `uv` with a pinned `python-build-standalone` runtime inside the app
  bundle, signed with the app.

### Safety and UX (all in Rust)

- **Own windows excluded from capture** with ScreenCaptureKit content filters, plus a compact
  always-on-top HUD (current step, Pause, Stop) while a run is active.
- **Global kill switch** (`tauri-plugin-global-shortcut`, default ⌃⌥⌘.): stops the loop and
  releases held keys/buttons immediately.
- **Policy:** per-run app allowlist, denylist (password managers, Keychain, System Settings
  security panes, banking), "confirm before typing into secure fields", "confirm
  submit/send/buy-like clicks", max steps, max wall time.
- **Permissions:** Screen Recording and Accessibility are granted to the app bundle itself,
  and since capture and input happen in-process there is no child-process TCC inheritance
  problem. Onboarding screen checks both and deep-links to System Settings.
- Every step logged (screenshot, action, reasoning) to the session for replay.

## 7. Models and providers

### What codex allows (checked in `app-server-protocol`, 2026-10-22)

- `thread/start` takes `model` + `model_provider`. The **provider is fixed for the life of a
  thread**; `thread/settings/update` and `turn/settings/update` can change only the `model`.
- `thread/fork` takes `model` + `model_provider`, so switching to another provider mid-task is
  a fork of the conversation onto the new provider.
- The only `wire_api` left is `responses`. Any provider must expose an OpenAI
  Responses-compatible endpoint.

### Layers

1. **Provider registry** (`tau-providers`). The user adds providers once in Settings:
   OpenAI, Anthropic, Google, OpenRouter, Ollama/LM Studio, Bedrock/Vertex, any
   OpenAI-compatible URL, or their own LiteLLM. Keys live in the OS keychain, never in config
   files. Each provider lists its models (fetched where the API allows, else typed), with
   context window, vision/tool support and prices.
2. **Gateway.** For providers without a native Responses endpoint (Anthropic, Gemini, most
   local servers), route through LiteLLM's `/v1/responses`. Two modes: a lazy local sidecar
   (shares the bundled Python runtime with `cua`) or an existing LiteLLM URL, such as one on
   the homelab. taucode generates the LiteLLM config from the registry, so users never edit it.
3. **Engines.** codex can drive any registered model. ACP engines run a model family in the
   harness it was tuned for: Claude models in Claude Code (API key), Gemini in Gemini CLI.
   Same UI, same MCP tools (browser, computer, codegraph), different loop.
4. **Role routing.** Defaults per role, overridable per project and per session:

   | Role | Typical choice |
   |---|---|
   | Main coding model | strongest available |
   | Side chat | cheaper/faster model |
   | Subagents | cheaper model unless the task says otherwise |
   | Titles, recaps, next-step box | smallest model |
   | Computer-use planner | vision model |
   | Grounding | UI-TARS |

### Switching in the UI

The model picker groups models by provider and shows what a switch will do:

| Switch | Mechanism | Cost shown to the user |
|---|---|---|
| Same provider, other model | `thread/settings/update` | Prompt cache lost: ~N tokens re-sent uncached |
| Other provider, same engine | `thread/fork` with new `model_provider`; old thread kept and linked | Full history re-sent to the new provider |
| Other engine (codex ↔ Claude Code) | New thread seeded with a generated handoff summary + open files + diff | Summary only; marked as a handoff in the transcript |

Also: per-session fallback ("if provider errors or rate-limits, fork to X"), and a
per-provider spend cap.

### Usage ledger

Record every turn's tokens (input, cached input, output, reasoning) from
`thread/tokenUsage/updated` and ACP usage events, priced from the registry. The Context pane
shows tokens and cost per session, per provider and per role; Settings shows daily and
monthly totals and the codegraph savings estimate (§8).

### Caveat

codex's prompts and tool formats are tuned for OpenAI models. Other models work through it,
but usually less well than in their native harness. That's why ACP engines are part of the
plan, not an extra. Measure each model in each engine on the eval suite (§12) before making it
a default.

## 8. Code intelligence and context efficiency (CodeGraph)

[colbymchenry/codegraph](https://github.com/colbymchenry/codegraph), MIT, v1.6.x. A
pre-built graph of symbols, call edges and dependencies (tree-sitter via a Rust kernel, SQLite
+ FTS5, native file watching, ~0.3 s incremental sync). Its MCP server exposes one tool,
`codegraph_explore`, that returns the relevant verbatim source, call paths and blast radius in
one call.

### What its "memory" actually is

- **Persistent:** the code graph in `<project>/.codegraph/codegraph.db`, kept fresh by the
  watcher, with a connect-time catch-up for edits made while nothing was running.
- **Per session:** an in-memory record of what `codegraph_explore` already sent on this MCP
  connection. Later calls replace source the agent already has with a back-reference
  ("delivered earlier in this conversation"), and only when the file bytes are unchanged.
  This is never persisted and dies with the connection.
- It is **not** long-term agent memory (preferences, decisions). For that use codex's thread
  memory mode, AGENTS.md, and skills.

### Integration

- **Binary.** Ship a pinned codegraph release (it bundles its own runtime, so no Node in the
  app). Verify its release signature in CI. Run with `CODEGRAPH_TELEMETRY=0` unless the user
  opts in. Never run its interactive installer: it rewrites global agent configs
  (`~/.codex`, `~/.claude`), which we manage ourselves.
- **Index lifecycle.** On project open: if there's no index, offer to build one (default on
  for git repos). Show progress in the sidebar. Add `.codegraph/` to the project's local git
  exclude (`.git/info/exclude`), not `.gitignore`.
- **One MCP connection per thread, through our proxy.** codex (and ACP engines) see a
  `codegraph` MCP server that is really `tau-mcp`, which holds one upstream codegraph session
  per thread. The proxy **reconnects the upstream session** on `thread/compacted`,
  `thread/rollback`, `thread/reverted` and checkpoint rewinds. Otherwise codegraph would keep
  pointing at source "delivered earlier" that compaction already removed, and the agent would
  be missing code without knowing it. Forks and side chats get a fresh session, which errs on
  the safe side (it re-sends).
- **Steering.** Only `codegraph_explore` is listed (their measurements show one tool steers
  better). taucode adds a short developer instruction to every thread: use `codegraph_explore`
  first for "how/where/what calls", and Read for files you're about to edit. Subagents get the
  same tool and instruction, since their benchmark found file-reading subagents erase the
  savings.
- **Same graph in the UI**, using its CLI with `--json`:
  - composer `@` mentions for symbols, not just files (`codegraph query`)
  - blast radius on file-change cards (`codegraph impact`)
  - a "Run affected tests" button in the Changes pane (`git diff --name-only | codegraph affected --stdin`)

### Verified in M0 (2026-10-23, codex 0.164, codegraph 1.6.2)

- codex sends `_meta.threadId` on every MCP tool call, so `tau-mcp-proxy` keys upstream
  sessions by thread without any process-to-thread mapping.
- codex reports compaction as an `item/completed` with `item.type == "contextCompaction"`;
  `thread/compacted` is never emitted. Resets hang off that item.
- codegraph's cross-call dedup is **off by default** (`CODEGRAPH_EXPLORE_DEDUP=1` opts in) and is
  read by the per-project daemon. taucode therefore runs its own daemon via
  `CODEGRAPH_DIR=.codegraph-taucode`, so other clients of the same project (Claude Code) keep
  dedup off. This costs a second small index per project.
- Result on a real thread: a repeated explore shrank from 22.4k to 18.4k chars with dedup
  pointers; after a compaction the same call re-sent the full source.

### Known trade-off

CodeGraph cuts tokens *processed* but leaves more *resident* context (its README measures
about 80% more at the end of long sessions: 67k vs 18k on VS Code). Countermeasures: compaction
thresholds tuned per model in the role router, and the dedup reset above, so dense payloads
aren't paid for twice.

## 9. Checkpoints and worktrees

- Before each turn that may write, snapshot the workspace into a shadow repo
  (`GIT_DIR=~/.taucode/shadow/<project-hash>`, `GIT_WORK_TREE=<project>`), so rewinds work even
  in non-git folders and never touch the user's history. Rewind = restore files + `thread/rollback`.
- Optional "new session in a worktree" for git projects, like Codex/Claude desktop.

## 10. Performance and usage budget

| Metric | Target |
|---|---|
| Tokens per eval task, codegraph on vs off | ≥ 40% fewer with codegraph on |
| Tool calls per "how does X work" question | ≤ 4 |
| Prompt-cache hit rate on multi-turn sessions | ≥ 70% where the provider supports caching |
| Cold start to usable sidebar | < 800 ms |
| Shell idle RAM (UI + host, excluding engine/sidecars/browser) | < 150 MB |
| Added latency, engine delta → pixels | < 16 ms p95 |
| Transcript with 10k items | 60 fps scroll |
| Python sidecar / grounding model | not started until first `computer.*` call; stopped after idle timeout |

Measure these in CI on a fixed fixture session from M1 on.

## 11. Milestones

Each milestone ends with a demo and its exit criteria passing.

**M0 — Spike (1 week)**
- Tauri window spawns `codex app-server`, `thread/start` + `turn/start`, streams
  `item/agentMessage/delta` into a plain list, handles one `item/commandExecution/requestApproval`.
- Same thing on a second provider through LiteLLM `/v1/responses` (Claude via API key), and
  a mid-task provider switch via `thread/fork`.
- codegraph indexed on a test repo and reachable from the thread through a pass-through proxy.
- Confirm subagent items (`CollabAgent*`), and whether codex opens one MCP connection per
  thread or shares one (this decides how the codegraph proxy keys sessions).
- Exit: one real coding task done end to end from the app on two providers.

**M1 — Core desktop parity, providers, codegraph (4 weeks)**
- Sidebar, sessions, resume/archive, full item renderers, composer (attachments, `@`, `/`,
  model/effort, permission modes, steer/queue/stop), Changes pane, Terminal pane, Files pane.
- Provider registry + Settings UI, keychain secrets, grouped model picker with switch-cost
  hints, usage ledger in the Context pane.
- codegraph lifecycle (index on open, progress, per-thread proxy with compaction/rewind
  resets, steering instruction).
- Exit: you'd rather use it than the codex TUI for a day of work; perf budget met; codegraph
  on/off A/B shows the token target on the eval suite.

**M2 — Browser (2 weeks)**
- Child webview pane with tabs, bridge, `browser.*` MCP tools, policy prompts.
- Exit: agent logs into a local dev app, finds a bug from the console, fixes it, verifies in
  the pane.

**M3 — Computer use (3 weeks)**
- `tau-computer` (capture, input, HUD, kill switch), cua sidecar, UI-TARS setup helper,
  progress UI, step log.
- Exit: 15 of 20 scripted macOS tasks pass (Finder, Notes, Settings toggles, a non-web app);
  kill switch stops within 100 ms; no screenshot ever contains our own window.
- Status (2026-10-23, M3–M3i): background-only, element-first.
  - **Live suite passing in the background** (cursor and frontmost unchanged):
    - TextEdit typing
    - Notes new note (⌘N → `ax-menu`)
    - Finder browse into a subfolder
    - System Settings General → About (`ax-select`)
    - the Privacy & Security pane denylist
    - Calculator 12×7
    - a pi agent driving a local web app through the taucode browser
  - **Known limit:** Finder disables File › New Folder while it's in the background, and
    popup/context menus aren't reachable there yet. The agent gets a recoverable
    "not executed" and reports failure.
  - **Specs:** `docs/specs/m3d`–`m3i`. Harness: `suite.py` / `live_webapp.py` (session
    scratchpad).

**M4 — Side chats, subagents, checkpoints, ACP engine (3 weeks)**
- Side chat pane, subagent tree, task list, next-step box, shadow-git rewind, worktree
  sessions, split view.
- ACP engine adapter with Claude Code as the first ACP engine; cross-engine handoff;
  role routing for side chats and subagents; codegraph symbol mentions, blast radius,
  affected tests.
- **Wave 1** (in progress 2026-10-23, parallel worktrees `m4a` / `m4b` / `m4c`):
  - `docs/specs/m4a-agents-tree-and-side-chats.md`
  - `m4b-checkpoints-rewind.md`
  - `m4c-acp-claude-engine.md`: `@agentclientprotocol/claude-agent-acp@0.89.1`,
    `--hide-claude-auth`, isolated `CLAUDE_CONFIG_DIR`, API keys from the registry only
- **Wave 2** (user request 2026-10-23):
  - `m4d-simple-chat.md`: Chat mode, a plain chatbot with no project; Code · Chat · Work
    mode switcher
  - `m4e-import-claude-codex.md`: import projects, threads, Cowork tasks, MCP servers
    and chats from Codex desktop/CLI, Claude desktop (Code tab and Cowork), and the
    claude.ai / ChatGPT exports. Read-only, idempotent, never reads credentials.
  - `m4f-work-mode.md`: Cowork-style Work mode, with folder spaces, a task queue,
    plan/outputs panes, document skills (pinned `engines/work` uv lock), and
    approval-before-overwrite.
  - Order: m4d, then m4e ‖ m4f.

**M5 — Ship (2 weeks)**
- Signing + notarization with bundled Python, permission onboarding, auto-update
  (`tauri-plugin-updater`), crash reporting (opt-in), settings UI.

About 15 weeks for two people on macOS first. Windows after M5; Linux after that.

**Later:** more ACP engines (Gemini CLI, goose); provider fallback chains; remote/SSH projects;
self-hosted web console and SessionControl/StarNet GraphRAG ideas from the zcode fork;
Agent-S4-style learned interfaces; Chrome/CDP browser mode.

## 12. Evals

A fixed suite of about 30 tasks across 3 repos: questions, bug fixes, small features, one
browser task, one computer-use task. Each task is run per (engine, model) and with codegraph on
and off. Record pass/fail, tokens, cost, tool calls and wall time. The suite decides
role-routing defaults and catches regressions when codex, codegraph or a model is upgraded.

## 13. Splitting the work

| Person A — host + engine | Person B — UI |
|---|---|
| tau-engine codex + ACP adapters, event fan-out | Layout, sidebar, transcript renderers |
| tau-providers, LiteLLM gateway, usage ledger | Provider settings, model picker, usage views |
| tau-codegraph proxy and lifecycle | Symbol mentions, blast radius, affected tests UI |
| tau-mcp, tau-browser bridge | Composer, panes, side chat, subagent tree |
| tau-computer + cua sidecar | Browser pane chrome, HUD, onboarding |
| tau-policy, tau-checkpoint, packaging | Command palette, shortcuts, theming |

Agree on the event schema (codex items + our `browser`/`computer` extras) in M0 so both can
work against recorded fixtures.

## 14. Reuse map: take code, don't write it

Reference clones live in `~/taucode-refs/` (outside the repo), indexed with codegraph so agents
query them instead of reading files. Checked 2026-10-23.

| Need | Take from | License | How |
|---|---|---|---|
| Desktop shell, sidebar, threads, composer (steer/queue, `@`/`$`/`/`), approvals, diffs, git, worktrees, terminal, model picker | **Dimillian/CodexMonitor**: Tauri + React on `codex app-server` | MIT | Fork as the M1 base instead of building the UI from scratch. Last pushed 2026-03-26, so update it to the 0.164 protocol first. |
| Engine abstraction across Codex, ACP agents, pi, OpenCode | **pingdotgg/t3code** `packages/provider-core`, `provider-acp`, `effect-codex-app-server`, `provider-pi` | MIT | Port the adapter contracts to our `Engine` trait; read their event normalization before writing ours. |
| Model-agnostic engine with native multi-provider support and mid-session hand-off | **earendil-works/pi**: `pi-coding-agent` (engine), `pi-ai` (unified LLM API, cost tracking, context hand-off) | MIT | Run pi as engine #2 for non-OpenAI models instead of forcing them through codex; reuse `pi-ai`'s model catalog and pricing for `tau-providers`. |
| Claude in its native harness | **agentclientprotocol/claude-agent-acp** + `agent-client-protocol` Rust crate | Apache-2.0 | ACP engine adapter in the host. |
| MCP server/client | **modelcontextprotocol/rust-sdk** (`rmcp`) | Apache-2.0 | Used by `tau-mcp-proxy`. |
| MCP client/process patterns, sandboxing, patch format | **openai/codex** `codex-rs/rmcp-client`, sandbox crates | Apache-2.0 | Reference; sandboxing comes for free through app-server. |
| Browser snapshot and ref format | **microsoft/playwright-mcp** | Apache-2.0 | Port its accessibility-snapshot format to the WKWebView bridge; use it as-is for Chrome mode. |
| Computer use | **simular-ai/Agent-S** | Apache-2.0 | As in §6. |
| Code graph | **colbymchenry/codegraph** | MIT | As in §8. |

Keep a `THIRD_PARTY_NOTICES.md` with each copied file's origin and license from the first
copied line.

**Effect on milestones:** if CodexMonitor is forked, M1 shrinks to "port CodexMonitor onto the
0.164 protocol, then add providers, codegraph and our panes" (about 2 weeks instead of 4), and
the M0 engine module is kept only where it beats CodexMonitor's backend.

## 15. Risks

| Risk | Mitigation |
|---|---|
| Non-OpenAI models weaker inside codex | ACP engines for their native harness; eval suite sets defaults per model. |
| Responses-API gateway gaps (reasoning items, images, tool-call shapes) | Pin LiteLLM; fixture tests per provider; fall back to an ACP engine for that model. |
| Provider fixed per codex thread | Switch via `thread/fork`, with cost shown up front. |
| codegraph dedup out of sync after compaction or rewind | Per-thread proxy reconnects upstream on those events (§8). |
| codegraph inflates resident context | Per-model compaction thresholds; track resident tokens in evals. |
| codegraph is a fast-moving single-maintainer project | Pin version, verify signatures, keep it behind the proxy so it can be swapped or disabled per project. |
| codex app-server protocol changes | Pin version, vendor schema, fixture contract tests, upgrade deliberately. |
| WKWebView limits (no CDP, untrusted events) | Bridge covers most sites; Chrome/CDP mode in v2; computer use as last resort. |
| Grounding model needs a GPU / lots of RAM | Hosted endpoint option; UI-TARS via MLX on Apple Silicon (16 GB+). |
| Computer use is slow (seconds per step) | Prefer browser tools and shell over GUI; route to CU only for native apps. |
| Bundled Python signing/notarization | Solve in M3, not M5: build a signed bundle in CI as soon as the sidecar exists. |
| Scope creep toward a full IDE | No editor beyond a viewer; "Open in editor" hands off to the user's IDE. |
