# M4j: one consistent, opaque, lean UI across Code, Chat and Work

## Goal

From user screenshots of the M4 build:

1. **Transparency hurts readability.** The window is see-through: `tauri.conf.json` has
   `"transparent": true` and `macOSPrivateApi`, and ~10 stylesheets use
   `backdrop-filter`. The desktop wallpaper and other windows show behind the text. The
   user wants **no transparency** and a **resource-lean** UI.
2. **Code, Chat and Work look like three different apps:**
   - the mode switcher's size, padding and position differ
   - sidebar fonts and button sizes differ (Chat/Work use large "New chat", "Trash",
     "Add space…" buttons; Code is compact)
   - headers differ
   - Settings is only reachable in Code
3. **The Browser button floats bottom-right**, which is jarring. It belongs in the
   **top-right header actions**, like Claude desktop / Codex desktop.

When this is done, the three modes share one shell, one header, one type scale and one
set of components. Everything is opaque. The Browser toggle is in the top-right of every
mode.

## Required

### 1. Opaque, lean rendering

- `tauri.conf.json`: `"transparent": false`; drop `macOSPrivateApi` unless something
  else needs it (check and report). Keep the overlay title bar and traffic-light position.
- Remove every `backdrop-filter` / `-webkit-backdrop-filter`, and every translucent
  surface (rgba backgrounds used as glass) from `src/styles/**` and feature CSS. Replace
  them with **solid tokens**.
- Remove the "Reduce transparency" setting and its plumbing (settings type, UI, tests),
  migrating stored values silently.
- **Define surface tokens** in one place (`src/styles/` tokens file, light and dark):
  `--bg-app`, `--bg-sidebar`, `--bg-surface`, `--bg-surface-raised`, `--bg-input`,
  `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`,
  `--accent-text`, `--danger`, `--warning`, `--success`.
  - Dark: near-neutral greys, e.g. app `#141416`, sidebar `#18181b`, surface `#1e1e22`,
    raised `#26262b`, text `#ececf1` / muted `#a1a1aa`.
  - Light equivalents.
  - All text/background pairs must meet WCAG AA contrast (4.5:1 body, 3:1 large and UI).
    Add a small unit test that computes the contrast for the token pairs.
- No large background images, gradients over the content area, or heavy box-shadow
  stacks. Subtle 1px borders and at most one soft shadow on popovers.

### 2. One app shell for all modes

A shared `AppShell` used by Code, Chat and Work.

**Left sidebar:**
- same width (Code's current resizable sidebar width and behaviour) and same padding
- top: traffic-light inset, then the **mode switcher**: one component, the same size and
  position in all modes (compact segmented control: Code · Chat · Work)
- a **primary action row**, the same component in every mode: "New thread" (Code; keep
  existing project actions), "New chat" (Chat), "New task" (Work), all the same compact
  button style
- a search field where the mode has one (Chat, Code projects search), same style
- the mode's list content, using the **same list-row component** (title, subtitle/meta,
  hover/active states, badges): Code threads, Chat chats grouped by date, Work tasks
  grouped by status and spaces
- **footer, identical in all modes:** "Settings" (always), plus at most one compact
  mode-specific secondary control in the same style:
  - Chat: "Trash" as a list section or small link, not a large button
  - Work: "Add space…" and the concurrency control move into a compact "Spaces" section
    header menu; "Stop all tasks" moves to the Work header actions, shown only when tasks
    are running

**Main header bar** (same height and style in all modes):
- left: title (thread / chat / task name, or "New chat" / "New task" / workspace name)
  and the engine/model chips where they apply
- right: mode-specific actions, then **global actions in a fixed order at the far right:
  Browser toggle (globe icon, tooltip "Browser"), then the right-panel toggle where that
  mode has a right panel**
- remove the floating bottom-right Browser button (`features/browser/BrowserDock.tsx`):
  the dock opens as a right-side panel, toggled from the header; keep its existing panel
  behaviour and shortcuts

### 3. Shared components and type scale

- One type scale (`--font-ui` 13px, small 12px, title 15px, h1 28px), applied everywhere.
  The Chat/Work sidebars currently render larger text: normalize.
- Shared `Button` variants (primary / secondary / ghost / danger; sizes sm and md),
  `Select`, `TextArea`, `Card`, `EmptyState` (eyebrow, h1, subtitle, suggestion cards,
  composer slot) and `Composer` frame styles:
  - Chat's composer, Work's new-task form and Code's composer share radius, padding,
    border, focus ring and the send-button style
  - Chat's and Work's empty states use the same `EmptyState` (they're close already;
    align spacing/sizes)
  - Code's workspace home keeps its content but uses the same cards and typography
- The suggestion cards in Chat/Work use the same `Card` and hover style.

### 4. Small fixes seen in the screenshots

- The composer placeholder says "Ask Codex to do something…" even on pi threads. Use
  "Ask {engine display name} to do something…" (pi → "taucode"; Codex; Claude Code), or
  the neutral "Ask taucode to do something…" when unknown.
- For a non-git workspace:
  - **no** red error toast ("could not find repository … code=NotFound"); that's a normal
    state, not an error
  - show the "not a Git repository" state **once**, in the Git panel with "Initialize
    Git"
  - the main-area banner becomes a single compact line, or nothing
  - the branch chip shows nothing instead of "unknown"

### 5. Don't change behaviour

Same features, data and shortcuts. Only layout, styling and component reuse. Phone and
tablet layouts must keep working (the header actions collapse into an overflow menu at
narrow widths).

## Tests

- Unit: the contrast check for token pairs.
- Each mode renders `AppShell` with the same switcher, footer (Settings present) and
  header; the Browser toggle exists in the header of all three modes; no floating
  Browser button.
- The placeholder per engine.
- The non-git workspace: no error toast, a single not-a-repo state.
- `rg backdrop-filter src` is empty (a test or lint rule).
- Update existing tests for the moved controls.

## Files

`apps/taucode/src/**` (styles, layout, app, chat, work, browser, composer, git display,
settings display section), `apps/taucode/src-tauri/tauri.conf.json`, and the settings
type for the removed option (Rust `types.rs` + TS). No backend behaviour changes.

## Constraints and verification

- No new npm deps (no UI kits). Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`
  with `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust
  parallelism; the known sandbox-only tailscale test may be excluded).
- Don't run `tauri build`.
