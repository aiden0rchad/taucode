# M4p: Settings redesign (every section) and side-chat-in-panel polish

Source: user screenshots plus the reviewer's DOM sweep of every Settings section in the
preview. Keep the M4m/M4o visual language and tokens; don't rename CSS variables.

## Part A: Settings

### Bugs

1. **The nav doesn't scroll.**
   - `.settings-nav-list` is 789px tall with `overflow: visible` inside a shorter dialog,
     so the bottom items (Computer Use, Server, Import, About) are clipped and
     unreachable.
   - Make the dialog body a two-column grid with `min-height: 0`; the nav and content
     columns each `overflow-y: auto`, scrolling independently.
   - Dialog size: `min(1040px, 92vw)` × `min(860px, 90vh)`.
2. **An "Engine runtimes" block leaks into Models & Agents, Browser and Computer Use**,
   with run-together text ("piMock runtime ready" + a grey "Set up pi" pill).
   - Render it in exactly one place: Models & Agents → **Engines**.
   - Browser and Computer Use instead show one compact status row (runtime name, status
     pill, a "Manage" link that jumps to Models & Agents → Engines).
   - Labels read like "pi · Ready" with a status pill (Ready / Installing / Missing /
     Error, with an icon); "(mock)" goes only in the preview fixtures, as a muted
     suffix.
3. **Native `<fieldset>`/`<legend>` boxes** (Models & Agents, Browser "Browser
   preferences", Computer Use "Planner and safety") draw a border with the legend cutting
   through it. Replace them with the group component below.
4. **Native selects truncate their values** (Computer Use "Background (recommended):
   y…"). The sweep counted native selects in 13 sections, up to 10 on one page
   (Projects).

### Settings design system

Build it once and use it in **every** section: Providers, Models & Agents, Connectors,
Composer, Display & Sound, Shortcuts, Dictation, Git, Open in, Projects, Environments,
Browser, Computer Use, Server, Import, About.

- **`SettingsPage`:**
  - title (20px semibold) and a one-line description
  - optional sub-tabs as a segmented control under the title (replace today's grey
    pills like "Codex engine / Engine features")
  - **Models & Agents** gets the tabs Defaults · Subagents · Roles · Engines
- **`SettingsGroup`:**
  - a heading (13px semibold, sentence case) and an optional description, above a card
    (raised surface, hairline border, radius 12)
  - rows inside are separated by hairlines
  - no fieldset/legend
- **`SettingsRow`:** label (13px medium) plus a description (12px muted) on the left, and
  the control right-aligned (fixed control column ~260px for selects/inputs; toggles
  right-aligned). Long controls (textareas, lists) span full width below the label.
- **Controls:** one styled `Select` (custom trigger showing the full selected value,
  chevron, popover list, keyboard support; or a styled native select that never
  truncates the value: width fits the content up to the column, ellipsis only beyond
  that), `Toggle` (the existing settings-toggle style), `TextField`, `TextArea` (styled,
  monospace option for lists like allowed origins), `NumberStepper`, segmented control,
  and `Button` variants. **No raw native checkboxes or radios**: Composer and Open in
  still have them; they become toggles or segmented controls.
- **Help and warnings:**
  - helper text sits directly under the control it describes (never floating between
    unrelated rows)
  - warnings render as an inline callout (info/alert icon, subtle tinted border); for
    example the "Some Anthropic-compatible gateways reject…" note belongs in the Claude
    Code engine row under Engines
- **Spacing:** 24px between groups, 12px row padding, consistent 13px base; the minimum
  text size stays 11px.
- **Per-section touch-ups:**
  - Shortcuts: a two-column list with key-cap styled chips
  - Projects: per-project rows collapsed into expandable cards instead of 10 inline
    selects
  - About: version and build info as a definition list

## Part B: side chat in the tabbed panel

From the user's screenshot:

1. **A leftover column renders beside the panel:** a narrow strip with a "Side chat"
   button sits between the conversation and the panel. When a side chat is shown in the
   panel tab, the old split view must not render at all.
2. **Header:**
   - remove the plain "← Back" button (closing or switching tabs replaces it)
   - one compact header row: side-chat icon + title (truncate), a status pill
     (Idle/Running), and a `⋯` menu (Rename, Archive, Open in split view)
3. **Messages:**
   - use the same message components and column styling as the main conversation, with
     16px side padding inside the panel
   - assistant text currently touches the left edge; the user bubble is clipped at the
     right edge (max-width must account for the panel padding and scrollbar)
4. **Composer:** use the shared `ComposerShell`, with the combined model+reasoning chip,
   the access chip defaulting to Read-only (the side-chat rule), "+" and send. Remove the
   old "Model [text input]" and "Read-only" native select.
5. "Send to main" stays as a message hover action.

## Tests

- **Settings:**
  - the nav scrolls (computed overflow-y auto with min-height 0)
  - the runtime block appears only in Engines
  - no `fieldset`, no raw checkbox/radio inputs anywhere in settings
  - selects render the full value (a test on the trigger text)
  - every section renders through `SettingsPage`/`SettingsGroup`/`SettingsRow` (a test
    iterating all sections)
  - Models & Agents sub-tabs
- **Side chat:** no split column when hosted in the panel; header actions; the shared
  composer used; message padding classes.
- Update the preview fixtures so every settings section and a side-chat panel tab render
  with realistic data.

## Files

`apps/taucode/src/features/settings/**`, `src/features/sidechats/**`, the panel/layout
code that hosts side chats, `src/features/composer/**` (only to reuse `ComposerShell`),
styles, `src/dev/uiPreview/**`. No backend changes.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust parallelism;
  the known sandbox-only tailscale test may be excluded), plus
  `npm run build -- --mode uipreview`.
- Don't run `tauri build`; no browser or computer use (the reviewer checks visuals).
