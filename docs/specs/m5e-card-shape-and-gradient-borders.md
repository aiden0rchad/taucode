# M5e: theme-controlled card shape and gradient borders (premium Glass and Neon edges)

## Goal

Themes can control the **shape** and **border paint** of the large card surfaces drawn by
the Squircle primitive:
- project/space cards, the Ungrouped cards and Work space cards
- the composer, the Settings window, the theme editor and the add-engine modal (all
  ModalShell squircle cards), and side-panel tool tiles

Two built-ins use this:
- **Glass Dark / Glass Light:**
  - keep the squircle corners
  - the 1px border becomes a soft **glass/silver gradient**, brighter at the top-left as
    if lit, fading through the middle, with a faint lift at the bottom-right
  - plus a hairline inner top highlight, so the edges feel like cut glass
- **Cyberpunk Neon:** cards get **chamfered (cut) corners**: straight diagonal cuts at the
  top-left and bottom-right, like the user's reference (a Home Assistant
  "cyberpunk" dashboard).
  - The border is a **neon gradient**, magenta/pink `#ff2bd6` → violet `#a855f7` →
    electric cyan `#00e5ff` along the diagonal.
  - It gets a faint outer glow (when `effects.neonGlow` is on).
  - The card fill gets a very subtle top-to-bottom gradient (slightly lighter at the
    top), so cards feel lit.

All other built-ins look exactly as today (a solid border with squircle corners).

## Current state

- `src/features/design-system/components/squircle/{Squircle.tsx,squirclePath.ts,squircle.css}`:
  - With no native support (WebKit, which is the real app), it measures with a
    ResizeObserver and draws a `clip-path: path()` background plus an SVG stroke path
    colored by `--squircle-border`.
  - With native `corner-shape: squircle` (Chromium only) it uses CSS borders and hides
    the SVG.
  - M5c fixed positioning via `:where()`. Keep that.
- Themes: `src/features/themes/**` (manifest, validators in TS and Rust
  (`src-tauri/src/themes.rs`), shared fixtures, builtins, the editor, the gallery,
  `docs/themes.md`, `docs/theme.schema.json` and the generated docs table).
  `effects.neonGlow` exists from M5c.

## Design

### Theme manifest additions (schema v1, backward compatible; all optional)

```json
"shape":  { "corners": "squircle" | "chamfer" | "round", "chamfer": 10 },
"border": { "gradient": ["#ff2bd6", "#a855f7", "#00e5ff"], "angle": 135, "highlight": false }
```

- `corners`: `squircle` (default, today's look), `round` (plain circular radius
  `--radius-window`), `chamfer`.
- `chamfer`: the cut size in px, 4–20, default 10. Cuts are on the **top-left and
  bottom-right** corners. The other two corners keep a small 2px radius, so they aren't
  razor sharp. Clamp to ≤ 1/3 of the smaller side for tiny elements.
- `border.gradient`: 2–4 colors (validated like color tokens; `var(--allowed-token)`
  allowed). When absent, the solid `--squircle-border` is used, exactly as today.
- `border.angle`: 0–360, default 135.
- `border.highlight`: adds a 1px inner top highlight. Used by Glass.
- Selected cards (`.is-selected`, `:focus-within` composer) use the theme's
  `--squircle-border-active`. With a gradient, the selected state raises the opacity and
  brightness of the same gradient (≈ +35%) instead of switching to a flat color; with
  neonGlow it also gets a stronger glow.
- Validators:
  - TS and Rust validate the same rules, with new fixtures in
    `src/features/themes/fixtures`: valid chamfer, valid gradient, too many stops,
    `url(` in a stop, out-of-range chamfer, unknown corners value
  - update `docs/theme.schema.json`, `docs/themes.md` (a new "Shape and border"
    section with examples) and the generated docs table

### Rendering (in the Squircle primitive; one implementation)

- Expose the theme's shape and border to CSS as custom properties on `:root`. The
  applier writes:
  - `--card-corners: squircle|chamfer|round`
  - `--card-chamfer: 10px`
  - `--card-border-1..4`, `--card-border-angle`, `--card-border-stops: 3`
  - `--card-border-highlight: 0|1`
- The Squircle component reads them (`getComputedStyle`) on mount, on resize and on the
  existing `taucode-theme-applied` event, and picks the path generator:
  - `squirclePath` (existing)
  - `roundedRectPath`
  - **`chamferPath(width, height, cut, radius = 2, inset)`** (new, in `squirclePath.ts`
    or a sibling file)
  - The fill clip and the stroke use the same shape (the stroke is inset 0.5px for
    crispness, as today).
- **Gradient stroke:** inside the existing border `<svg>`, add a `<defs><linearGradient>`
  with a unique id per instance (`useId`):
  - `gradientUnits="objectBoundingBox"`, rotated by the angle
  - `<stop>`s whose `stop-color` comes from CSS variables via the `style` attribute
  - the stroke uses `url(#id)`
  - With no gradient, keep the plain `stroke: var(--squircle-border)`.
- **Glow (neon only, when `effects.neonGlow` is on):** a second, identical path behind the
  stroke with `stroke-width: 3`, the same gradient, `opacity: .35`, and an SVG
  `feGaussianBlur` (stdDeviation 3) filter defined once per instance. It's static: no
  animation, no per-frame work. Check that the glow isn't clipped (the svg is
  `overflow: visible` and the card mustn't clip its own border).
- **Highlight (glass):** a 1px line path along the top edge (following the corner
  curves), with a horizontal gradient `rgba(255,255,255,.0) → .35 → .0`.
- **Fill gradient:** neon cards get `--squircle-background: linear-gradient(180deg,
  color-mix(in srgb, var(--surface-card) 92%, white 8%), var(--surface-card))`. Glass
  keeps its translucent fill from M5c.
- **Native path:** when the theme has `corners: chamfer` or a border gradient, **always
  use the SVG path** even where `corner-shape` is supported, so Chromium (the preview)
  and WebKit (the app) look the same. Native `corner-shape` stays only for the plain
  squircle with a solid border.
- **Performance:**
  - Paths are recomputed only on resize or theme change, using the existing cache (key
    it by shape kind too).
  - No layout thrash: read the CSS vars once per theme change, not per resize.
  - Gradient defs are static.

### Built-in values

- **Glass Dark:**
  - corners `squircle`; gradient `["rgba(255,255,255,0.30)", "rgba(255,255,255,0.06)",
    "rgba(255,255,255,0.14)"]`; angle 160; highlight true
  - active: the same stops at about `0.48 / 0.12 / 0.24`
- **Glass Light:**
  - gradient `["rgba(255,255,255,0.95)", "rgba(0,0,0,0.06)", "rgba(255,255,255,0.7)"]`;
    angle 160; highlight true
  - Plus a subtle outer `rgba(0,0,0,0.06)` hairline, so cards still separate on light
    backgrounds. Tune for visibility; contrast ≥ 1.3:1 against the sidebar for the
    average border color.
- **Cyberpunk Neon:** corners `chamfer`, chamfer 10 (the composer and Settings window may
  use 14: make the cut scale gently with card size, e.g. `min(14, max(8, minSide/10))`);
  gradient `["#ff2bd6", "#a855f7", "#00e5ff"]`; angle 135; neonGlow true.
  - Rows inside cards (selected thread rows) keep their own styling, not chamfered.
- All other built-ins: unchanged (no shape/border keys).

### Theme editor and gallery

- Editor: a new "Shape & border" group with:
  - Corners (segmented: Squircle / Round / Chamfer)
  - Cut size slider (shown for Chamfer)
  - Border gradient: a toggle, plus 2–4 color stops with swatches, add/remove, and an
    angle slider
  - Top highlight toggle
  - The live preview panel uses the same Squircle primitive, so it shows the shape and
    gradient.
- Gallery cards: the preview frame uses the theme's shape and border, so Neon's card in
  the gallery is chamfered with the neon edge and Glass shows the silver edge.

## Tests

- `chamferPath`: the right number of vertices, cuts at TL/BR, clamping for tiny boxes, no
  NaN, symmetric inset.
- Validators: the new fixtures (TS and Rust identical).
- Squircle:
  - picks squircle / round / chamfer from the CSS vars
  - renders a gradient with a unique id per instance
  - two instances don't share ids
  - falls back to a solid stroke with no gradient
  - uses SVG even when `corner-shape` is supported if the theme sets a gradient or
    chamfer
  - glow only when neonGlow is on
- Theme switch re-reads the vars (dispatch `taucode-theme-applied`) and updates the path
  kind.
- Built-ins: Glass and Neon have the expected keys; every other built-in has none
  (unchanged look).
- Keep M5c's positioning tests and the WebKit check script working.

## Files

`src/features/design-system/components/squircle/**`, `src/features/themes/**`,
`src-tauri/src/themes.rs` (validator only), `docs/themes.md`,
`docs/theme.schema.json`, `scripts/themes-docs.mjs`, the CSS where `--squircle-*` vars
are set per surface (`styles/visual-refresh.css`).

**Off-limits:** diagnostics, engines and feature toggles (a parallel M5d is working in
another worktree).

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (the permitted tailscale
  exclusion), plus `npm run build -- --mode uipreview` and
  `node --check scripts/webkit-check.mjs`.
- Don't run `tauri build`; no browser or computer use (the reviewer runs the WebKit
  check).
