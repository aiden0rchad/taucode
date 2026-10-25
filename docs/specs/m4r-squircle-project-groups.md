# M4r: squircle project group cards (macOS 27 "Golden Gate" window radius)

## Goal

Each project (Code), each space (Work) and the "Ungrouped" bucket render as a **grouped
card** with a **squircle (continuous-curvature) border**. Each project group is easy to
see, and the corner radius matches macOS 27 Golden Gate windows.

## Facts (reviewer research, 2026-10-25)

- Window corner radius: macOS 27 Golden Gate is reported at **20pt** (macOS 26 Tahoe 26pt,
  earlier 10pt). These are community-measured values (MacRumors
  `NSConvolutionOverride1` thread; 9to5Mac Golden Gate coverage); Apple publishes no
  figure. Define the token `--radius-window: 20px` with a comment citing this.
- **CSS `corner-shape: squircle` / `superellipse()` is Chromium 139+ only.** WebKit /
  Safari (Tauri's macOS WKWebView) doesn't support it. A plain `border-radius` gives a
  circular arc, not a squircle.

## Required

1. **A `Squircle` primitive** (`src/components/Squircle.tsx` or the design-system
   folder):
   - Wraps content; measures its box with `ResizeObserver`.
   - Generates an SVG path for a rounded rectangle with **continuous-curvature corners**
     (a Figma-style "corner smoothing" squircle: radius `--radius-window` = 20px,
     smoothing 0.6; any well-known formulation is fine, e.g. the cubic-Bézier
     approximation of iOS continuous corners). Clamp the radius to half the smallest
     side.
   - Applies it as `clip-path: path(...)` to the background layer, and draws the 1px
     border as an absolutely positioned SVG `<path>` stroke (stroke inside, so no
     half-pixel blur: use a 0.5px inset path or `vector-effect: non-scaling-stroke` with
     integer-aligned coordinates).
   - **Progressive enhancement:** with `@supports (corner-shape: squircle)`, use
     `border-radius: var(--radius-window); corner-shape: squircle; border: 1px solid …`
     and skip the SVG.
   - Recompute only on size change (ResizeObserver, rAF-batched). No per-frame work.
     With ≤ ~50 groups the cost is negligible; reuse paths for identical sizes.
   - The hover/selected states of child rows stay normal radius-6 rows inside the card.
2. **Project group card** (Code sidebar; Work spaces; the Ungrouped bucket):
   - The card contains the project header row (folder icon, name, count, hover '+'/⋯)
     and its tree of threads with the guide lines.
   - The card's border uses `--tree-line` (or `--border`, whichever reads better; keep
     contrast ≥ 1.8:1 against the sidebar), a background one step raised from the
     sidebar (subtle), 4px inner padding, and 6px vertical gap between cards.
   - A collapsed project shows just the header inside the card.
   - Section labels ('Studio', 'Ungrouped') stay outside the cards.
   - The selected project's card border uses `--tree-line-active`.
   - The guide lines start inside the card and must still align (adjust offsets for the
     4px padding).
3. **Consistency:** add `--radius-window` to the radius scale. Use the same squircle
   primitive at the window radius for the other large containers that represent
   "windows" in the UI: the Settings dialog, the composer card and the side panel's
   new-tab tool tiles. Smaller elements keep their existing radii (rows 6, chips 6,
   popovers 12).
4. **Light and dark:** check both; borders crisp at 1x and 2x.

## Tests

- The path generator: correct size, symmetric, radius clamped, smoothing applied
  (snapshot of the path for a 260×120 box); no NaN for tiny boxes.
- The `Squircle` component: uses the SVG path when `CSS.supports('corner-shape', 'squircle')`
  is false, and the CSS path when true (mock `CSS.supports`).
- The sidebar renders one card per project/space plus Ungrouped; collapsed state;
  selected-card class.
- The `--radius-window` token = 20px.
- Preview fixtures: two projects, one collapsed, Ungrouped with Notes, and Work spaces.

## Files

`apps/taucode/src/components/**` (or the design-system dir), sidebar/workspaces/work
features, settings dialog, the composer shell, side panel tiles, styles,
`src/dev/uiPreview/**`. No backend changes; no new deps.

## Constraints and verification

- Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (default Rust parallelism,
  the permitted tailscale exclusion), plus `npm run build -- --mode uipreview`.
- Don't run `tauri build`; no browser or computer use.
