# M4q: compact tree sidebar with branch lines; message hover/bubble fixes

From user screenshots (Code mode, the preview) and reviewer measurements. Keep the
existing tokens; don't rename CSS variables (the undefined-var test must pass).

## 1. Compact sidebar rows

**Measured today:**
- `.thread-row` is 64px tall (title line plus a second line with the engine `pi` badge and
  a model chip like `openai/gpt-5-mini`; the time sits right) with 8px gaps, so 72px per
  thread
- `.workspace-row` is 55px (name, plus '13 conversations · Updated 7m')

The user finds the spacing too loose.

**Required (Code, and Chat/Work for consistency):**
- **Thread / chat / task rows:** a **single line, 28px tall**, 2px vertical gap, radius
  6px:
  - leading status/kind icon (14px), the title (13px, truncate)
  - a right-aligned relative time (11.5px muted), replaced by the row's `⋯` on hover
  - the engine and model move out of the row: show them in the row tooltip and in the
    thread header; keep a tiny 6px engine-colour dot before the time only when the
    thread's engine differs from the project default
  - running = a pulsing dot; needs-input = an amber dot (as today)
- **Project / space rows:** 28px, a folder icon (lucide `Folder` / `FolderOpen` when
  expanded) + name + a muted count, with a chevron and '+' on hover. Drop the second
  'N conversations · Updated' line (put it in the tooltip).
- **Section labels** ('Studio', 'Ungrouped', 'Today'): 11.5px muted, 6px top margin;
  collapse chevrons on hover only.
- 'More…' becomes a 24px muted row ('Show 8 more').
- Add **Display & Sound → Sidebar density: Compact (default) / Comfortable**, where
  Comfortable restores two-line rows (title + model) at 44px. Both densities share the
  same components.

## 2. Tree with branch lines

Render nesting as a tree with subtle **guide lines**, like a file tree / Codex desktop:

```
📁 Atlas                     13
│ ● Polish the welcome mes…  7m
│ ├ ⑂ Why keep the greeting…  8m      ← side chat / fork (child of the thread above)
│ └ 🤖 agent-review           7m      ← subagent thread, if any
│ ○ New thread               7m
└ ○ GPT-5                    7m
```

- Levels: project/space → threads/tasks → side chats / forks / subagent threads (any
  depth; indent 14px per level).
- Lines are 1px, colour `--border` (about 1.4:1 against the sidebar), drawn with
  pseudo-elements: a vertical line from the parent's icon centre down to the last child,
  and a rounded elbow (radius 6px) into each child row's icon. The last child gets a
  corner, not a tee.
- The selected row's elbow and its path to the root brighten slightly (`--border-strong`).
- Collapsing a parent hides its children and their lines. Lines must align at any
  sidebar width and both densities.
- Chat has no nesting except side chats (if any); Work: space → tasks.
- Pure CSS (no SVG per row). Performance: no layout thrash for 500 rows.

## 3. Message hover box contains its content

**Today:**
- hovering an assistant message paints a grey hover background, but the content sits
  flush against the box: the text has no inset and the code block reaches the box edges
  (the user: "the text goes outside of the boxes")
- the hover action icons (side chat, quote, copy) sit in the box's bottom-left with no
  spacing

**Required:**
- The hover highlight is a padded surface: 10px 12px inner padding, applied by a negative
  margin of the same size, so the text doesn't shift on hover; radius 10px, and it
  contains all children including code blocks (which keep their own border).
- The action icons appear in a small floating toolbar at the **bottom-right** of the
  message (an icon group with a raised surface and hairline border), offset so it
  overlaps the box edge slightly, like Claude/ChatGPT. Hidden until hover or
  focus-within.
- Alternatively drop the hover background entirely and only show the floating toolbar.
  **Choose the hover background, kept very subtle** (`--surface-hover` at low contrast).

## 4. Bubble height

User bubbles (main thread and side chat, e.g. 'Why use a constant here?') are taller than
their text: there's empty space under the line. Bubbles must hug their content (12px
14px padding); no reserved space for actions inside the bubble, since actions use the
floating toolbar.

## Tests

- Row heights: compact 28px and comfortable 44px (computed style tokens).
- Tree classes and depth attributes on nested rows; last-child class.
- The density setting persists.
- Hover box padding uses a negative margin (no layout shift: assert that the computed
  content offset is equal with and without the hover class).
- The bubble has no fixed min-height.
- Update the preview fixtures: nested side chat and subagent threads under a thread, a
  collapsed project, Work spaces with tasks.

## Files

`apps/taucode/src/features/{app,threads,workspaces,sidebar,chat,work,messages,sidechats,settings}/**`,
styles, `src/dev/uiPreview/**`. No backend changes.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust parallelism;
  the permitted tailscale exclusion), plus `npm run build -- --mode uipreview`.
- Don't run `tauri build`; no browser or computer use.
