# M5l: step-by-step setup wizard (replaces the setup checklist)

## Goal

Replace the first-run **Setup checklist** with an elegant, calm, **step-by-step wizard**.
Each step asks one thing, applies it immediately (a real configuration change), then
moves to the next. It's easy to follow, skippable, resumable, and re-openable from
Settings → About ("Run setup").

Visual and motion language is inspired by **Animata** (https://animata.design/components):
- **Shared Axis Z** step transitions
- **Soft Blur In** headings
- an **Animated Timeline** progress rail
- selection **cards**
- a **Toggle Switch**
- a **Status Button** (idle → loading → success/error)
- a subtle **Blurry blob / Grid** ambient background

Animata's effects use the Web Animations API, not framer-motion. Implement them with
CSS/WAAPI only: **no new npm dependencies**, no Tailwind.

## Current state

- `src/features/onboarding/SetupChecklist.tsx` (+ `onboarding.css`, tests, and preview
  `?setup=1`) is a status checklist modal. Its rows: Providers, Engines, Screen
  Recording, Accessibility, Codegraph, Diagnostics.
- The status sources (`getSetupStatus`, provider/engine status) and deep links
  (`openSetupPrivacy`) already exist in `src/services/tauri.ts`.
- **Configuration services to reuse:**
  - Providers (Settings → Providers): `providerSetApiKey`, `providerHasApiKey`,
    `providerTestAndFetch`, and the `appSettings.providers` shape `ProviderEntry`
  - Engines: `engine_runtime` (status/prepare/cancel) and the engine registry
    (`appSettings.engines`, `agentPolicy.defaultEngine`)
  - Features: `appSettings.features`
  - Themes: the theme gallery/applier (`applyTheme`, builtins, `appSettings.theme`)
  - Workspaces: the add-workspace (folder dialog) flow, and `codegraphEnabled`
- `ModeShell.tsx` mounts the checklist and handles first-run display and the Settings
  navigation. Keep the first-run trigger semantics (shown once; skipping persists).

## Steps (one question each; each Continue applies the change)

0. **Welcome**
   - The large title "Welcome to taucode" animates with Soft Blur In, per word.
   - Subtitle: "A calm place to work with any model, any engine." (per-word crossfade
     in)
   - Primary **Get started** (a slide-arrow button: the arrow nudges right on hover).
     Quiet link "Set up later".
1. **Look**: "Pick a look."
   - Theme cards in a 3×N grid, reusing `ThemePreview compact`: System, Dark, Light,
     Glass Dark, Claude Dark, Cyberpunk Neon, Matrix, …
   - Selecting a card applies the theme **live** (the whole app, the wizard included,
     changes behind it).
   - The selected card gets an accent ring plus a check with a spring-scale (1 → 1.04
     → 1).
2. **Provider**: "Connect a model provider."
   - Provider cards (icon + name + one line): OpenAI, Anthropic, OpenRouter, Google,
     Local (Ollama / LM Studio, no key), OpenAI-compatible (LiteLLM, vLLM, etc.).
   - Picking one reveals (with a height + blur-in transition) the fields that kind
     needs:
     - Name (prefilled)
     - Base URL (only for compatible/local; prefilled defaults like
       `http://localhost:11434/v1`)
     - API key (password field; never logged; stored via the existing keychain
       service)
   - **Test & continue** is a Status Button: idle → spinner "Testing…" → ✓
     "Connected · 23 models" (morphs to a check) or ✗ with the error text inline. It
     calls the existing `providerTestAndFetch`.
   - On success, show a **model picker** (a searchable list of fetched models) for the
     default model, which sets `agentPolicy.defaultModel`.
   - If providers already exist, show them as already-configured cards, with "Use
     this" or "Add another".
   - Skipping is allowed, with a gentle note: "You'll need a provider before your
     first conversation."
3. **Engine**: "Choose your engine."
   - Cards: **pi** (Recommended · lightweight), **oh-my-pi** (more built-in tools ·
     ~1.1 GB download), **Claude Code**, **Codex**, each with a status chip (Ready /
     Will install / Not available).
   - Continue sets `agentPolicy.defaultEngine` (and enables it if disabled). If the
     runtime is pending, it shows an inline install progress bar (`engine_runtime
     prepare`, with progress events and Cancel) before moving on.
4. **Project**: "Open your first project."
   - A big dashed drop-zone card: "Choose a folder…" (the dialog, via the existing
     add-workspace flow) or drag and drop a folder.
   - After choosing: the folder name and path, plus a **Toggle Switch** "Code
     intelligence (codegraph)", shown only if codegraph is installed (from
     `getSetupStatus`), which sets `codegraphEnabled` for that workspace.
   - Secondary: "Skip — I'll just chat for now."
5. **Features**: "Turn on what you want."
   - Animata-style Toggle Switches (a pill track; the knob springs; the track color
     transitions) in two short groups:
     - Workspace: Git, Terminal, Checkpoints
     - Tools for the agent: Browser, Computer use; plus Chat and Work modes, and
       Developer: Diagnostics
   - Each has a one-line description. The defaults are today's defaults.
   - Writes `appSettings.features`.
6. **Permissions** (only when Computer use is on): "Let taucode see and operate apps."
   - Two rows, Screen Recording and Accessibility, each with a live status chip
     (Granted / Not granted, re-checked on window focus) and an **Open System
     Settings** button (the existing exact deep links).
   - Copy explains why. "Skip for now" is always available.
   - **Never request or change permissions automatically.**
7. **Done**: "You're all set."
   - An **Animated Timeline** recap: each completed step draws its connector line and
     its check in sequence; skipped steps are muted with "Set up later".
   - Primary **Start building**, which closes and lands in Code mode, or Chat if no
     project was added.
   - A small, tasteful success flourish: the check morphs from the Status Button; no
     confetti.

## Layout and motion

- **First run is a full-window takeover**, not a modal:
  - a centered content column, max-width 560px
  - a top bar with the wordmark and "Set up later"
  - a **progress rail** at the top: dots + labels (Look · Provider · Engine · Project ·
    Features · Permissions), Animated-Timeline style; the current dot pulses softly,
    completed dots fill with a check, connectors fill as you advance
  - **Back** (ghost) and **Continue** (primary) pinned at the bottom of the column
  - When reopened from Settings → About, the same wizard opens as a large Squircle
    modal at the window radius, over the Settings scrim.
- **Step transition:** Shared Axis Z (direction-aware):
  - **forward:** the old step exits with scale 1 → 1.04, opacity → 0, blur → 1px
    (~260ms, ease `cubic-bezier(0.4,0,1,1)`); the new step enters with scale 0.94 → 1,
    opacity 0 → 1, blur 2px → 0 (~380ms, ease `cubic-bezier(0.2,0,0,1)`)
  - **back:** the scales are reversed
- **Headings:** Soft Blur In per word (opacity 0 → 1, blur 12px → 0, translateY 9px → 0,
  ~650ms, ease `cubic-bezier(0.22,1,0.36,1)`, 40ms per-word stagger). Body text fades in
  60ms after.
- **Background:** two large, soft, blurred blobs in the theme accent and a secondary
  color (radial-gradients, `filter: blur(80px)`, opacity 0.18–0.25), drifting very
  slowly (a 40s CSS animation), over a faint 24px grid (`background-image` lines at 4%
  opacity) masked to fade at the edges.
  - Uses theme tokens, so it looks right in every theme.
  - Paused when the window is hidden. Static under `prefers-reduced-motion` (all the
    transitions become 150ms crossfades).
- **Cards:** hover lifts 1px with a soft border highlight. Selected: an accent ring plus
  a check badge, spring scale.
- **Keyboard:**
  - Enter = Continue (when valid)
  - Esc = "Set up later" (with a confirm on the first-run takeover)
  - arrow keys move between cards
  - focus moves to each step's heading on transition (a11y)
- **Lean:** CSS/WAAPI only. No per-frame JS. Animations are cancelled on unmount.

## State and behavior

- Wizard progress (the current step, completed and skipped steps) persists in
  `localStorage` (`taucode.setup.v1`), so quitting mid-way resumes at the same step.
  Completion persists in app settings (reuse the checklist's existing first-run flag).
- **Each step applies its change when you continue,** through the existing services,
  and shows an error inline without advancing if that fails.
  - **Back** never undoes an applied change. It re-opens the step with the current
    values.
- Steps adapt:
  - Permissions only appears if Computer use is on.
  - Codegraph's toggle only shows if it's installed.
  - Existing providers, engines and projects are detected and pre-selected, so
    re-running setup is quick.
- Settings → About: replace "Setup checklist" with **"Run setup"**, which opens the
  wizard at the first incomplete step, or at Done if everything's complete.
- Remove `SetupChecklist` (or keep only its status-fetching hook as the wizard's data
  source).

## Preview harness

- `?setup=1` opens the wizard at Welcome; `?setup=<step>` (look|provider|engine|project|features|permissions|done) jumps to a step.
- Mock services in `src/dev/uiPreview/**`:
  - provider test succeeds after 900ms with 23 models; a key containing "bad" fails
    with an error
  - engine prepare streams progress over about 3s
  - the folder dialog returns `~/Projects/atlas`
  - the permissions mix: Screen Recording granted, Accessibility not

## Tests

- Navigation: Next/Back/skip/Esc, direction-aware transition classes, focus moves to the
  heading.
- Each step calls the right service with the right payload:
  - theme apply plus the settings save
  - provider save (the key only through the keychain service; it never appears in
    settings JSON or logs)
  - test & fetch; the default model
  - default engine plus prepare with progress and cancel
  - add workspace plus codegraph
  - features
- Inline errors keep you on the step.
- Resume from `localStorage`.
- The permissions step appears only with Computer use on; deep links use the exact URLs;
  nothing auto-requests.
- `prefers-reduced-motion` gives crossfade-only, no blob animation.
- The Done timeline marks completed and skipped steps.
- The preview `?setup=<step>` routes.
- **The API key never appears in the DOM after save** (the field clears) or in any
  console/diagnostics output.

## Files

`src/features/onboarding/**` (the new wizard components, CSS, tests),
`src/features/chat/ModeShell.tsx` (mounting), the Settings → About entry,
`src/services/tauri.ts` (reuse; add only what's missing), `src/dev/uiPreview/**`.
Reuse the theme, provider, engine and workspace hooks/components; don't duplicate their
logic.

## Constraints and verification

- No new deps. Offline. Follow the existing settings design tokens, the z-index scale,
  the Squircle primitive and the theme tokens (it must look right in Dark, Light, Glass
  Dark, Claude and Cyberpunk Neon).
- Verification: the command in `docs/specs/m3-computer-use.md`, with
  `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` and `$HOME/.cargo/bin` on
  PATH (the permitted tailscale exclusion), plus `npm run build -- --mode uipreview`.
- No browser or computer use (the reviewer checks it visually in WebKit). Don't commit.
