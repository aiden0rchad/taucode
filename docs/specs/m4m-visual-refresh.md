# M4m: visual refresh. Composer, sidebar, messages and empty states in the style of modern agent harnesses

## Goal

The user finds the UI "pretty basic": the chat box should be nicer, with well-placed
icons like other harnesses, and Chat/Work look less polished than Code. Bring all three
modes to the polish level of Claude desktop, the Codex app, ChatGPT and Zed's agent
panel, using the shared patterns below. One visual language, light and dark.

## Hard rules (a previous attempt broke the app)

- **Don't rename or remove existing CSS variables, font stacks or class names** that
  other files use. Change values, or add new variables. If you must replace a variable,
  update every reference in the same change and add a test that fails on undefined
  `var(--…)` references (scan `src/**/*.css` for `var(--x)` where `--x` is never
  defined).
- The UI stays opaque: no `backdrop-filter`, no translucency.
- Lean rendering: no new deps, no web fonts downloaded at runtime, no heavy shadows or
  animations (transitions ≤ 150 ms, `prefers-reduced-motion` respected).
- **Review loop:** the reviewer checks screenshots in the preview harness
  (`npm run preview:ui`, port 1421). Keep the fixtures rendering every screen named
  below, and add fixture states if a screen needs them (e.g. composer with attachments,
  streaming state).

## Design language

### Tokens

Additive; keep the existing names working.

- **Type:**
  - UI font: the system stack (`-apple-system, BlinkMacSystemFont, "SF Pro Text",
    "Inter", system-ui, sans-serif`); mono: `ui-monospace, "SF Mono", Menlo, monospace`
  - UI text 13px/1.45, secondary 12px, message body 14.5px/1.65
  - titles 15px semibold; empty-state greeting 28px medium, `letter-spacing -0.01em`
  - numbers: `font-variant-numeric: tabular-nums` in stats and usage
- **Radii:** 6 (chips, small buttons), 8 (rows, inputs), 12 (cards, menus),
  18 (composer).
- **Spacing:** a 4px grid.
- **Icon buttons:** 28×28, 16px icons.
- **Icons:** `lucide-react` everywhere (already a dep), 16px, `strokeWidth 1.75`; no
  emoji or text glyphs (▶ ☰) used as icons.
- **Surfaces (dark):**
  - app background
  - a sidebar ~3% lighter
  - content background
  - raised surface (composer, cards, menus) ~5% lighter, with a 1px hairline border
    (`rgba`-free solid colour, e.g. `#2a2a2e` on `#1b1b1e`)
  - a single soft shadow only on the composer and popovers
  - light theme mirrors this
  - one accent colour, used sparingly: primary button, send button, focus ring, links,
    active states
- **States:** hover = subtle fill; active/selected = slightly stronger fill plus text at
  full contrast; focus = a 2px accent ring offset 1px; disabled = 40% opacity with a
  tooltip explaining why.

### Shared composer (`ComposerShell`), used by Code, Chat and Work

A single rounded card (radius 18, raised surface, hairline border, soft shadow).

- **Attachments row** (when present): dismissible chips above the text. Image
  thumbnails 40px with an × on hover; files show a file-type icon (`vscode-material-icons`,
  already a dep) plus the name.
- **Text area:** auto-grows (1 to ~10 lines, then scrolls), 14.5px, generous padding
  (14px 16px 8px). Placeholder per mode: "Ask taucode anything…" (Code shows the engine
  name, as now), "Message…" → "Ask anything", "Describe the result you want…" (Work).
- **Toolbar** (inside the card, under the text, 36px tall). **Left:**
  - a **"+" icon button** opening the add menu: attach files/images, plus mode-specific
    items (Code: mention a file, add a folder, plan mode, if those exist today; Chat:
    toggle Web; Work: attach to the space)
  - then **compact chips** for active modes, each with an icon and a short label:
    - access mode (Code/Work: shield icon + "Ask" / "Read-only" / "Full access")
    - Web (Chat: globe chip when on, removable)
    - side chat (when inside one)
  - **Right:** model picker as a ghost button (provider initial/logo dot, model name,
    chevron), reasoning/effort chip where it applies (Code), the **mic icon** (existing
    dictation), and a **circular send button** (32px, accent fill, arrow-up icon)
  - **Send states:**
    - disabled (empty input): muted fill, with a tooltip saying why
    - running: the send button becomes a **stop button** (square icon), and keeps the
      queued-message behaviour Code has today
    - Enter sends, Shift+Enter adds a newline (keep the existing shortcuts); the hint
      moves to the send tooltip and isn't a visible text line
- **Under the card,** for Code only: the existing context row (Local/worktree,
  branch, engine) as small muted chips with icons, like the Codex app.
- **Work:** the Space/Model/Access selects move into the composer toolbar as chips
  (space chip with a folder icon on the left; model and access chips on the right), plus
  "Queue" as a secondary icon/action next to send. The brief text area is the composer.
  The "Drop attachments here…" sentence becomes a drag-over overlay plus the + menu.

### Sidebar (all modes)

- Background a step tinted from the content; a hairline right border.
- **Top:**
  - the mode switcher (segmented, 28px, as now but restyled to the tokens)
  - then a row: "New chat / New thread / New task" as a full-width ghost row with a
    pencil/plus icon and the shortcut hint on the right (muted), plus a search icon
    button
- **Rows:**
  - flat, not boxed cards; 32px single-line, or 44px with meta
  - radius 8 hover/active fill
  - leading status dot or icon (running: animated subtle pulse dot; needs-input: amber
    dot; done: none)
  - title (13px, truncate), meta (12px muted: model · relative time); the time
    right-aligned on single-line rows
  - row actions on hover (⋯)
- **Sections:** small muted labels (12px, medium weight, sentence case, not all caps),
  with a collapsible chevron for projects/spaces and a "+" on hover for project/space
  headers.
- **Footer:** Settings as a row with a gear icon; mode-specific small icon buttons
  (Chat: Trash) to its right.

### Header (all modes)

44px; title 14px semibold, truncating, with small chips (engine, branch) next to it.
Right: ghost icon buttons (28px) with tooltips; the globe (Browser) and the panel toggle
come last. A hairline bottom border.

### Messages (Code, Chat, Work transcripts)

- Centered column, max-width 760px.
- **User messages:** right-aligned soft bubble (raised surface, radius 14, max 80%
  width).
- **Assistant messages:** no bubble, full column width, 14.5px/1.65, with good spacing
  for headings, lists and paragraphs.
- **Hover actions** below each message, as icon buttons with tooltips: copy, plus
  regenerate (last assistant), edit & resend (last user), side chat, rewind (user
  messages, Code). They replace today's always-visible text buttons ("Side chat",
  "Regenerate last answer", "Edit and resend", "Rewind…").
- **Tool calls:** compact collapsible rows (icon per kind: terminal, file-pen, search,
  globe, bot for subagents, list-checks for plans), a muted label, and a chevron to
  expand. Show a running spinner while in progress; errors in red text with an alert
  icon.
- **Code blocks:** a header bar with the language label and copy icon, the mono font,
  and a raised surface.
- **Plans:** a checklist with check-circle icons and a thin progress bar.

### Empty states

- **Chat / new Code thread:** the greeting ("What's on your mind?" / "What should we
  build?") centered in the upper third, **the composer centered under it**, and **starter
  pills** below it: small rounded pills, each with an icon and a short label. These
  replace the large cards.
- **Work:** the same layout, with the 5 templates as pills (icon + label), and the space
  chip in the composer prompting "Choose a space".
- **Code home dashboard:** keep its content, restyled with the card tokens (stat cards
  in a grid, tabular numbers, hairline borders).

### Work specifics

- Task rows show a small progress ring (plan completion) when running.
- The right pane:
  - Plan as a checklist with a progress bar
  - Outputs as file tiles: file-type icon, name and size, with Open / Reveal as icon
    buttons on hover
- Header: a status pill, a labelled "Stop" button and the Rewind icon button.

## Tests

- **Undefined CSS variable scan:** fails on any `var(--x)` with no definition anywhere
  in `src/**/*.css`.
- `ComposerShell` is used by Code, Chat and Work; the send/stop toggle; the + menu items
  per mode; the tooltip on disabled send.
- Message hover actions replace the old text buttons (tests updated).
- No emoji/glyph icons (`▶`, `☰`) in TSX.
- The existing behaviour tests still pass.
- Preview fixtures: add a streaming state, a composer with 2 attachments, and a running
  Work task with a progress ring.

## Files

`apps/taucode/src/**` (styles, composer, messages, layout, app, chat, work, sidebar,
dev/uiPreview fixtures). No backend changes.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: `cd apps/taucode && npm run typecheck && npm test && npm run build && npm run lint && npm run build -- --mode uipreview`.
- Don't run `tauri build`.

## References (research, 2026-10-24)

- [Anthropic: Redesigning Claude Code on desktop](https://claude.com/resources/articles/claude-code-desktop-redesign):
  one sidebar for all sessions (filter/group by project), side chats, verbose / normal /
  summary tool detail, a usage button for context/session
- [ChatGPT composer teardown](https://aiuxplayground.com/teardowns/chatgpt/composer):
  a "+" menu for attach/tools/modes, removable mode chips, mic and send on the right,
  starter pills in the empty state, mode-specific placeholders
- [How to design an AI chat composer](https://aiuxplayground.com/guides/how-to-design-ai-chat-composer):
  calm default bar; attach, send and one tools entry always discoverable; model/effort
  controls before send; context as dismissible chips; no unexplained disabled send
- [Zed agent panel docs](https://git.secluded.site/zed/blob/v0.197.5/docs/src/ai/agent-panel.md?source=1):
  model selector in the message editor with a provider logo, profile selector, token
  count near the selectors
- [Codex app docs](https://developers.openai.com/codex/app): project sidebar, thread
  plus review pane, a Local/Worktree/Cloud selector before the first send, and a sidebar
  with search, New chat, Pinned, Projects and Recents
