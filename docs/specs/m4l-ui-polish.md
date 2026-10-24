# M4l: opaque window, Browser in the header, Chat/Work match Code. Keep the existing look.

## Context

- An earlier attempt (M4j, reverted) replaced the design tokens. The font fell back to
  Times and surfaces and borders disappeared. **Don't repeat that.**
- **The current look of Code mode is the reference the user likes:** its fonts, card
  rows, pills, borders, radii and spacing. Preserve the existing CSS variables and font
  stacks. Change values, don't rename or remove variables, unless every reference is
  updated in the same change.
- **Preview harness:** `npm run preview:ui` (port 1421) renders the UI in a browser with
  fixtures. Useful screens:
  - `?mode=code&theme=dark`
  - `?mode=chat&theme=dark`
  - `?mode=work&theme=dark`
  - the same with `theme=light`

  You can't open a browser in your sandbox; the reviewer compares screenshots before and
  after, so keep changes targeted.

## Required

### 1. Opaque window, same appearance

- `src-tauri/tauri.conf.json`: `"transparent": false`. Drop `macOSPrivateApi` only if
  nothing else needs it (report).
- Remove `backdrop-filter` / `-webkit-backdrop-filter` everywhere.
- **Every translucent background** (rgba / hsla / `color-mix` with transparency used for
  panels, sidebars, cards, inputs, popovers, toasts, the composer) becomes the **solid
  colour it visually resolves to** over the app background, in both dark and light.
  Example: `rgba(255,255,255,0.04)` over `#121212` → `#1c1c1c`. Use a small helper/script
  to compute these and keep the variable names.
- Keep borders, shadows, radii, font stacks and sizes unchanged.
- Remove the "Reduce transparency" display setting and its plumbing (settings types,
  UI, tests); stored values are ignored.
- Add a test asserting `backdrop-filter` doesn't appear in `src/**/*.css`.

### 2. Browser toggle in the top-right header

- Remove the floating bottom-right "Browser" button (`features/browser/BrowserDock.tsx`
  trigger).
- Add a **globe icon button** (tooltip "Browser", same style as the existing header icon
  buttons in Code: play / VS Code / terminal / panel toggles) in the **top-right header
  actions of every mode**, right-most before the panel toggle.
- It toggles the browser dock/panel exactly as the old button did, keeping existing
  shortcuts.
- At narrow widths it joins the existing overflow behaviour.

### 3. Chat and Work use Code's visual language

- **Mode switcher:** the same component, size and position in all three modes; Code's
  current pill switcher is the reference.
- **Sidebar footer:** Code's "Settings" button appears in all modes, same component and
  place.
  - Chat's "Trash" becomes a compact item: a small icon button beside Settings, or a
    "Trash" row at the end of the chat list. Not a full-width button.
  - Work's "Add space…", "Concurrent tasks" and "Stop all tasks" move out of the footer:
    - "Add space…" goes into a "+" on the "Spaces" section header (like Code's project
      "+")
    - concurrency goes into a small menu on that section header
    - "Stop all tasks" goes into the Work header actions, visible only when tasks are
      running
- **Primary action:** "New chat" / "New task" use the same compact style as Code's
  sidebar actions (icon + label, the same height as Code's rows). Not large full-width
  buttons.
- **List rows:** Chat's chats and Work's tasks/spaces render with **Code's thread-row
  styling**: card row, status dot, title, meta line, hover/active states, nested
  indentation like side chats.
  - Pin / Rename / Delete aren't always-visible chips; they live in a hover "⋯" menu and
    a right-click context menu, like Code's thread actions.
  - Section headers ("TODAY", "PREVIOUS 7 DAYS", "NEEDS INPUT", space names) use Code's
    section-header style.
- **Header bar:** Chat and Work main headers use Code's header component/styles (height,
  title weight, icon buttons). The right-side actions end with the Browser globe.

### 4. Render markdown in Chat and Work

- Chat transcripts and Work task transcripts currently show raw markdown in monospace
  (`## heading`, `**bold**`, fenced code).
- Render assistant (and user) messages with the **same markdown/message components Code
  uses** (code blocks with copy, lists, inline code, links), in Code's message typography.
- Monospace only for code.

### 5. Carry over the small fixes (from the reverted M4j)

- Composer placeholder: "Ask {engine} to do something…" (pi → "taucode", codex →
  "Codex", claude → "Claude Code"). The preview currently shows "Ask Codex…" on a pi
  thread.
- Non-git workspace:
  - no red "could not find repository … NotFound" error toast
  - a single "not a Git repository" state with "Initialize Git" in the Git panel
  - no duplicate banner in the main area
  - the branch chip hidden instead of showing "unknown"

### 6. Don't change

Code mode's appearance, apart from the Browser button moving and the opaque surfaces. No
behaviour, data or shortcut changes.

## Tests

- No `backdrop-filter` (test).
- Each mode renders the shared switcher, the Settings footer button, and the header globe
  button; no floating Browser button.
- Chat/Work rows: actions are in the menu, not visible chips.
- Markdown rendered in Chat/Work (heading → `<h2>`, `**x**` → `<strong>`).
- Placeholder per engine; the non-git state.
- Update existing tests accordingly. Keep the preview fixtures working for every screen.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust parallelism;
  the known sandbox-only tailscale test may be excluded), plus `npm run build -- --mode
  uipreview` must compile.
- Don't run `tauri build`.
