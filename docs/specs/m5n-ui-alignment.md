# M5n: UI alignment and icon consistency pass (plus removing the redundant header ⋯)

The user reports "a lot of UI issues with button alignments, icons". Reviewer measurements (preview, Chat mode, Glass Dark):
- Icon-only buttons come in **six sizes**: 22×22 (×6), 26×29, 28×28 (×13), 32×32, 34×25, and a 155×34 icon-only footer button (the trash button in the sidebar footer).
- The title bar row (`.workspace-titlebar`) has children 17–39px tall, with vertical centers spread by 2.5px.
- Chat sidebar rows (`.mode-row`): the row's ⋯ menu button is 8px off the row's vertical center (44px row vs 22px button).
- In the screenshot:
  - the composer's "Read-only ⇕" select uses a stray up/down glyph, and the icon sizes there don't match
  - the sidebar footer's "Settings" button and the icon-only trash button have different heights and padding
  - the header shows a **⋯ "More actions" button that is redundant**: it's always visible and only contains a Browser toggle that duplicates the side panel's tool

## Required

### 1. One icon-button system
- Add `IconButton` to the design system (`size="sm|md|lg"` → 24/28/32px squares, `radius 8px`, icon 14/16/18px, centered with `display: inline-grid; place-items: center`).
- Variants: ghost (default), subtle (a surface fill), active/pressed. A focus ring from the existing tokens. The `aria-label` is required (TS-enforced).
- Tokens: `--icon-button-24|28|32`. If they already exist (a parallel M7 may add them in `styles/ds-tokens.css`), reuse them.
- Migrate **every icon-only button** in these areas to it, choosing sizes consistently:
  - the title bar (back, forward, sidebar toggle, side-panel toggle, ⋯ when present): md 28
  - sidebar header icons (search, filter, refresh): md 28
  - sidebar row actions (row ⋯, + on project cards): sm 24, vertically centered on the row
  - composer toolbar (+, mic, send): md 28 (send keeps its circular accent style at 28)
  - the side-panel tab bar (+, expand): md 28
  - settings page icon buttons: md 28
  - the diagnostics, evals and onboarding icon buttons
  - **Not** the browser pane toolbar: M7 rewrites it.
- **Icons:** one icon set (lucide) at stroke-width 1.75 everywhere (set it once via a wrapper or CSS `svg { stroke-width: 1.75 }` scoped to the icon buttons). Replace stray glyph characters used as icons (`⇕`, `›`, `▾`, `×` text) with lucide icons (ChevronsUpDown, ChevronRight, ChevronDown, X).

### 2. Row alignment rules
- Every horizontal toolbar row (`.workspace-titlebar`, `.main-header`, `.sidebar-header`, the composer toolbars, `.side-panel-tabbar`, settings group headers, wizard footers) is `display: flex; align-items: center` with a fixed row height. No child may offset its own vertical position (remove the stray `margin-top`/`padding-top` hacks that cause the 2.5px spread).
- Text buttons in toolbars are 28px tall (sm) or 32px (md). Labels are 13px with consistent horizontal padding (10px), and icon + label gap is 6px.
- **Sidebar footer:** "Settings" and the trash (or other) button are the same height (32px). The icon-only button is a 32px square, not a stretched 155px bar.
- **Chat/Work rows:** trailing actions are vertically centered on the row; the time and the ⋯ share one slot (the ⋯ appears on hover in place of the time, like the Code sidebar).

### 3. Remove the redundant header ⋯ and Browser button
- Delete the header Browser (globe) toggle in every mode. The side panel's tools and "New page" are the single entry point. Keep ⌘⇧B, which opens a new page in the side panel.
- `HeaderActions` renders the ⋯ overflow trigger **only when there are overflowed actions** (`overflow === true` and at least one child hidden). With no actions (Chat/Work), render nothing.
- Tests:
  - no globe button in the header
  - no ⋯ when not overflowing
  - the ⋯ appears and contains the overflowed actions at a narrow width

### 4. An automated alignment audit (prevents regressions)
- Add a DOM audit test util, `assertRowAlignment(container)`. For each toolbar row selector above, the children's vertical centers must be within 1px, and icon-only buttons must be exactly 24/28/32px squares.
- Run it in tests for: the title bar, sidebar header, sidebar footer, composer toolbar (thread composer, project home composer and chat composer) and side-panel tab bar.
- Extend `scripts/webkit-check.mjs` (the reviewer runs it) to run the same audit in WebKit on Code, Chat and Work at 1440×900 and 1024×700. It prints the offending elements with their rects.

## Files
- `src/features/design-system/**` (IconButton, tokens)
- the components listed above (app header, sidebar, composer, side panel tab bar, settings, chat/work rows, diagnostics, evals, onboarding)
- `src/styles/**`
- `scripts/webkit-check.mjs`
- `src/dev/uiPreview/**` if fixtures need it

**Off-limits:** `src/features/browser/**` and the native browser work (a parallel M7 worktree owns those), and Rust.

## Verification
The command in `docs/specs/m3-computer-use.md` (`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`, `$HOME/.cargo/bin` on PATH, `TMPDIR=/Users/rolandleyco/taucode-m1w/.tmp`, the permitted tailscale exclusion), plus `npm run build -- --mode uipreview` and `node --check apps/taucode/scripts/webkit-check.mjs`. No browser or computer use. Don't commit.
