# M5b: feature toggles and swappable engines (adds oh-my-pi)

## Goal

taucode is built like a car with swappable components, so users get two things:

1. **Turn whole features on and off** in Settings → Features. Someone who isn't
   interested in git, for example, can switch off all the git UI. A disabled feature
   disappears from the UI and does no background work, and where it's an agent tool, the
   agent no longer gets that tool.
2. **Choose and swap the engine** (the agent runtime): pi (default), **oh-my-pi (omp)**,
   codex, Claude Code (ACP), plus **custom engines** a user registers (any ACP agent, or
   any pi-RPC-compatible fork).

Done means:
- Settings → Features and Settings → Engines exist.
- omp runs end to end through the pi adapter with taucode's registry models.
- Every toggle hides its entry points and stops its background work.
- The tests below pass.

## Current state (read first)

- **Engines:**
  - `EngineKind` = `"pi" | "codex" | "claude"`: TS in `src/types.ts`; Rust in
    `src-tauri/src/types.rs` (an enum with `#[serde(other)] Pi`).
  - Routing is in `src-tauri/src/shared/engines.rs`. The pi adapter is
    `src-tauri/src/shared/pi/{config,process,translate,agents}.rs`; ACP is
    `src-tauri/src/shared/acp/**`.
  - Runtime install: `src-tauri/src/shared/engine_runtime.rs`. The bundled
    `package.json`/lockfile are listed under `resources` in `src-tauri/tauri.conf.json`,
    and `npm ci` runs into app data on demand.
  - `AppSettings.defaultEngine` exists, and projects can have an engine (see
    `ProjectEngine`, `SidebarTree.tsx`, `ComposerMetaBar.tsx`, `engineCapabilities.ts`).
- **pi isolation:**
  - `PI_CODING_AGENT_DIR=<data>/pi/agent/<id>`
  - `engines/pi/no-accounts.mjs` blocks auth files
  - `models.json` is written from the taucode provider registry (`pi/config.rs` ~line 235)
  - taucode extensions (`engines/pi/extensions/*.ts`) are embedded with `include_str!`
- **Settings:**
  - `SettingsFeaturesSection.tsx` today lists **codex** experimental flags only.
  - Nav: `SettingsNav.tsx`, `settingsTypes.ts`.

## oh-my-pi facts (verified by the reviewer 2026-10-25)

- Package `@oh-my-pi/pi-coding-agent@18.12.6`: MIT, a fork of pi, bin `omp` →
  `dist/cli.js`, **requires Bun ≥ 1.3.14**.
- **Already installed for you** in `apps/taucode/engines/omp/`:
  - `package.json` pins `@oh-my-pi/pi-coding-agent` 18.12.6 and `bun` 1.4.4
  - `package-lock.json`
  - `.gitignore`
  - `node_modules/` (~1.1 GB, git-ignored): run it as
    `engines/omp/node_modules/.bin/bun engines/omp/node_modules/.bin/omp`
  - The source and types are in `node_modules/@oh-my-pi/pi-coding-agent/src` and
    `dist/types`: read `modes/rpc/rpc-types`, `modes/rpc/wire/commands`, and the
    config/models loading code.
- `omp --mode rpc` speaks NDJSON with pi's command names: `prompt`, `abort`,
  `set_thinking_level`, `set_model`, `get_state`, `steer`, `follow_up`, `new_session`.
  Compare the event and response shapes with pi 1.4.0's, and translate the differences
  in the adapter.
- Isolation: `PI_CODING_AGENT_DIR` (also `PI_CONFIG_DIR`) is honored. A smoke test with
  `PI_CODING_AGENT_DIR=$tmp/agent` created only `agent.db`, `models.db` and
  `skill-descriptions.db` there.
  - Models come from **`models.yml`** in that dir (not `models.json`). Derive the exact
    schema from the omp source.
  - With no model configured it prints "No default model selected… create
    <dir>/models.yml" and exits.
- Must not leak or phone home:
  - set `OTEL_SDK_DISABLED=true`
  - never pass `OMP_AUTH_BROKER_URL`, `OMP_AUTH_BROKER_TOKEN` or any `OTEL_*` exporter
    vars (use the env allowlist)
  - no `/login` or OAuth providers. Mirror `no-accounts.mjs`: check whether omp's auth
    storage path differs, block it the same way via a Bun preload (`bun --preload`) or an
    equivalent hook, and test it.
  - Find any auto-update or network checks in the omp source and disable them with
    flags/env/config. List what you found in your report.
- omp also ships its own tools (LSP, debugger, browser, Python, subagents). Expose them as
  the engine's capabilities, but **taucode's own approvals gate, subagent policy and
  connectors still apply**:
  - port the taucode pi extensions if omp's extension API is compatible
  - otherwise enforce at the adapter: deny tool calls that the approval policy or
    disabled features forbid
  - Report exactly what is enforced and how.

## Required

### A. Engine registry (pluggable components)

1. Replace the closed `EngineKind` with an **engine id string** plus a registry of engine
   definitions:
   ```ts
   type EngineProtocol = "pi-rpc" | "codex-app-server" | "acp";
   type EngineDefinition = { id: string; name: string; protocol: EngineProtocol; builtIn: boolean;
     command?: string; args?: string[]; env?: Record<string,string>; configDirEnv?: string;
     enabled: boolean; description?: string };
   ```
   - Built-ins: `pi` (default), `omp`, `codex`, `claude`.
   - Persisted old values (`pi`/`codex`/`claude`) deserialize unchanged. An unknown id on
     a thread or project falls back to the default engine with a visible notice; it never
     panics.
   - The Rust routing (`engines.rs`) dispatches by **protocol**, so a new pi-RPC or ACP
     engine needs no new Rust code.
2. **omp built-in:** uses the pi adapter, parameterized by an `EngineProfile`:
   - command/args: `bun <omp cli> --mode rpc` plus the flags pi uses for no-session or
     session-dir as appropriate
   - config dir env: `PI_CODING_AGENT_DIR=<data>/omp/agent/<id>`
   - a models-file writer: `models.yml` for omp, `models.json` for pi, both generated
     from the same registry (API keys only via env/commands as pi does today; never
     written to disk)
   - preload/no-accounts hook
   - extension set
   - Thinking levels map through the same reasoning capability code.
3. **Runtime install:** add `engines/omp/{package.json,package-lock.json,.gitignore}` to
   bundle `resources`, never `node_modules`. `engine_runtime.rs` installs omp **on
   demand** (`npm ci` into app data) only when the user enables or selects omp, with
   progress and a cancel. Detect the installed version and Bun. Show "Install (~1.1 GB)"
   with the size in the UI.
4. **Custom engines:** in Settings → Engines, "Add engine":
   - name, protocol (ACP or pi-RPC), command (validated: exists and executable; resolved
     via PATH), args, non-secret env (keys matching
     `/(KEY|TOKEN|SECRET|PASSWORD)/i` are rejected with "use a provider instead"), and an
     optional config-dir env var name (when set, taucode points it at an isolated dir)
   - "Test" spawns it, does the protocol handshake (ACP `initialize`, or pi `get_state`)
     and reports the result. Saved in `AppSettings.engines`. Examples in the placeholder
     text: Gemini CLI (`gemini --experimental-acp`), a pi fork.
5. **Settings → Engines page:**
   - one row per engine: icon, name, protocol badge, status (Ready vN, Not installed →
     Install, Error → details), an enable toggle, and "Set as default"
   - custom engines also get Edit/Delete
   - The default engine select lives at the top.
   - Disabled engines vanish from the engine pickers (new-thread / project engine /
     composer); existing threads on a disabled engine open read-only with "Enable
     <engine> to continue".
6. The engine pickers everywhere list only enabled engines from the registry: project
   engine, new thread, side chat (same engine as the parent) and Work (keep Work's
   current engine constraints; if Work only supports pi, add omp only if the work
   extension runs under omp, else leave Work on pi and say so).

### B. Feature toggles

1. `AppSettings.features` (TS and Rust, serde default all `true`, so existing settings
   files keep everything on):
   ```
   git, worktrees, browser, computerUse, terminal, sideChats, subagents, checkpoints,
   chatMode, workMode, connectors, importTools, dictation, reasoningEffects
   ```
2. **Settings → Features** (a new taucode page in the nav, first in its group):
   - grouped toggles with one-line descriptions:
     - Workspace: Git, Worktrees & clones, Terminal, Checkpoints & rewind
     - Agents: Subagents tree, Side chats, Connectors (MCP)
     - Tools for the agent: Browser, Computer use
     - Modes: Chat, Work
     - Extras: Import from Claude/Codex, Dictation, Reasoning effects
   - Move the existing codex experimental flags page to **Engines → Codex → Advanced
     flags** (rename it "Codex flags"); nothing about it may be lost.
3. What each toggle does when **off** (UI hidden **and** no background work):
   - **git:**
     - hide the Changes tab, branch chips (header/composer meta/home), git panel, diff
       views, PR/issue views, git init prompts and git-related sidebar badges
     - stop git status/log polling and file watchers for git
     - the commit/push actions disappear
     - The calm non-git state is the default view.
     - **Checkpoints keep working** (they use their own store; verify this and keep it
       independent).
     - Turning git off also disables worktrees.
   - **worktrees:** hide "new worktree/clone agent", worktree sections and menus.
   - **browser:**
     - hide the header globe, browser tabs in the side panel and Browser settings
     - don't spawn the browser engine
     - **don't expose browser tools** to any engine (pi extension, omp, ACP mcp servers,
       codex config)
   - **computerUse:** don't expose computer-use tools or start its helper; hide its
     settings.
   - **terminal:** hide the Terminal tab and shortcut; don't spawn PTYs.
   - **sideChats / subagents:**
     - hide the side-chat entry points and tab; hide the Agents tab and sidebar subagent
       rows (they collapse into the parent)
     - for subagents, also set the max-concurrent policy to 0 for the agent (no spawn
       tool)
   - **checkpoints:** hide the rewind UI and stop creating checkpoints.
   - **chatMode / workMode:**
     - hide that segment in the Code/Chat/Work switcher
     - at least one mode must stay on: Code can't be disabled, and the UI prevents
       turning off the last mode
     - if the active mode gets disabled, switch to Code
   - **connectors:** don't pass MCP connectors to engines; hide Connectors settings.
   - **importTools:** hide Import settings and menus.
   - **dictation:** hide the mic; don't load the model.
   - **reasoningEffects:** the reasoning slider keeps working without heat, particles or
     animation.
4. Changes apply live. Engine-side changes (tools exposed) apply to **new turns/sessions**
   only; show "Applies to new sessions" on those rows. Don't kill running sessions.
5. One source of truth: `useFeatures()` (TS) and a `features()` accessor (Rust) that the
   gated code checks. Don't scatter raw settings reads.

## Files

`src/types.ts`, `src-tauri/src/types.rs`, `src-tauri/src/shared/{engines.rs,
engine_runtime.rs,pi/**,acp/**}`, the codex config writer (tool exposure only),
`engines/omp/**` (no `node_modules` commits), `engines/pi/**` (only if needed to share
the no-accounts hook), `src-tauri/tauri.conf.json` (resources only), the settings
feature (new Features and Engines sections; nav), the UI entry points listed above,
`src/dev/uiPreview/**` (fixtures: omp installed, a custom engine, some features off),
`docs/engines.md` (how to add an engine: protocols, isolation and the env rules).

**Off-limits:** themes and appearance (a parallel M5a touches `styles/themes*`,
`useThemePreference`, Appearance settings and window effects), and the provider registry
semantics.

## Constraints

- **No account logins, ever.** Keys come only from the registry via env/commands. Every
  engine gets an env allowlist and an isolated config dir.
- Offline. No new npm deps in `apps/taucode`. omp is already installed. Use
  `CARGO_HOME=/Users/rolandleyco/taucode-m5b/.cargo-home` with `$HOME/.cargo/bin` on PATH.
- Keep all existing engine behavior and tests green (pi, codex, claude ACP).

## Tests (add)

- **Registry:**
  - old engine values deserialize
  - unknown id → default plus notice
  - dispatch by protocol
  - disabled engine hidden from pickers; a thread on a disabled engine is read-only
- **omp profile:**
  - argv/env (allowlist; `OTEL_SDK_DISABLED`; no `OMP_AUTH_*`/`OTEL_*`)
  - `models.yml` generated from a registry fixture without secrets
  - isolated dir
  - preload blocks auth writes
- **Live omp test,** `#[ignore]` by default: run with `--ignored` when `engines/omp`
  exists. It spawns omp in RPC mode with a stub OpenAI-compatible server on localhost
  (reuse existing pi test helpers if there are any) and checks prompt → streamed events →
  completion through the adapter. Run it yourself and report the result.
- **Custom engine:**
  - validation (missing command, secret-like env rejected)
  - the Test handshake against a tiny fake ACP script and a fake pi-RPC script in test
    fixtures
- **Features:**
  - for each toggle, the entry points are absent from the DOM when off
  - git off stops polling (mock timers / spy on the service)
  - browser/computerUse/connectors off removes the tools from the engine config for pi,
    omp, ACP and codex
  - the last mode can't be disabled
  - an active mode that gets disabled switches to Code
  - settings migration defaults everything to on
- Codex flags are still reachable under Engines.

## Verification

The command in `docs/specs/m3-computer-use.md` (default Rust parallelism, the permitted
tailscale exclusion), plus `npm run build -- --mode uipreview`. Don't run `tauri build`;
no browser or computer use. Report the live omp test result separately.

## Out of scope

Themes (M5a), an engine marketplace, auto-updating engines, and Windows/Linux packaging
for Bun.
