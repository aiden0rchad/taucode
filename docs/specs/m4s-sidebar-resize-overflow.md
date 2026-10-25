# M4s: sidebar rows must shrink with the sidebar (no clipped text/selection)

## Bug (user screenshot)

Drag the sidebar narrower than ~290px, and inside each project's squircle card:
- the right side of rows is clipped: times show '9' / '10' instead of '9m' / '10m', and
  long titles run under the card edge instead of truncating
- the **selected row's highlight** is cut off at the card's rounded clip

## Likely cause (verify)

- An earlier measurement showed the row highlight `::before` with a fixed width of 275px
  (and rows / tree nodes sized from the old sidebar width), so rows don't shrink with
  the card.
- The squircle card clips its content (`clip-path` / `overflow: hidden`), hiding the
  overflow.
- Look for fixed px widths, `calc(var(--sidebar-width) …)`, `min-width` on
  `.thread-row`, `.workspace-row`, tree-node / children wrappers and their
  `::before` / `::after`, and for flex/grid children without `min-width: 0`
  (`styles/sidebar.css`, `styles/visual-refresh.css`, squircle css).

## Required

1. Rows (Code threads, Work tasks, Chat chats, project/space headers) are fluid:
   - `width: 100%` of their container, `min-width: 0` on every flex/grid child
   - title `flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis;
     white-space: nowrap`
   - time / count / actions `flex: 0 0 auto` (never shrink, never clipped)
2. The row highlight (hover/selected background or `::before`) is `inset: 0` relative to
   the row (no fixed width), with the row radius, and sits fully inside the card's inner
   padding at every width.
3. Tree guide lines and elbows are positioned relative to each row's start (depth ×
   indent), not to a fixed width; they stay aligned at any width.
4. Nested depth: when the sidebar is narrow, the indent per level may shrink (e.g. 14px →
   10px below 260px sidebar width), so deep rows still show at least ~60px of title.
5. The squircle card recomputes its path on resize (already ResizeObserver plus rAF).
   No frame may show content clipped by a stale path: the clip/background must track the
   current size (e.g. background layer uses `inset: 0` and the stale-path frame only
   affects the decorative background, never clips children).
6. Same rules for the Chat and Work sidebars.

## Tests

- Render the sidebar with fixture data at sidebar widths 220, 260, 300 and 420 (mock
  ResizeObserver; or assert computed styles): rows have no fixed widths; the title has
  ellipsis styles; the time element has `flex-shrink: 0`.
- A CSS scan: no fixed px `width` / `min-width` above 40px on row, tree-node or highlight
  selectors.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (the permitted tailscale
  exclusion), plus `npm run build -- --mode uipreview`.
- Don't run `tauri build`; no browser or computer use.
