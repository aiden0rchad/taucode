# M5c: centered modals in WebKit, real Glass translucency, Cyberpunk Neon theme

## 1. Bug: the Settings window (and other squircle modals) is stuck to the bottom-right

**Seen in the real app (WKWebView); reproduced in Playwright WebKit** at 1440×900: the
`.settings-window` rect is x=260, y=72, w=1180, h=828. Centered would be x=130, y=36.

**Cause:** the M4r Squircle primitive (`src/features/design-system/components/squircle/squircle.css`)
declares `.squircle.squircle { position: relative; … }`. Its specificity (0,2,0) beats
`.ds-modal-card { position: absolute; top: 50%; left: 50%; transform: translate(-50%,-50%) }`
(0,1,0), so every `ModalShell squircle` card is `position: relative` and ends up offset.
Chromium (our preview harness) happens to lay it out centered, so the preview missed it.

**Fix:**
- The squircle primitive must never change the host element's positioning or layout.
  Declare `position: relative` and `isolation: isolate` with zero specificity
  (`:where(.squircle)`), so any host that positions itself (modal cards, popovers) keeps
  its own `position` / `top` / `left` / `transform`.
- The background/border resets may keep their current specificity.
- Audit every Squircle usage (settings window, theme editor, add-engine modal, composer
  shell, side-panel tiles, project/space cards) for the same class of override, and fix
  any you find.
- **Tests:**
  - a CSS test that no rule in `squircle.css` sets `position`, `top`, `left`, `inset` or
    `transform` with specificity above (0,0,0)
  - a jsdom test that a `ModalShell squircle` card keeps the computed `position: absolute`
    from `.ds-modal-card`

**WebKit check script** (the reviewer runs it; your sandbox can't bind localhost or
launch browsers):
- Add `apps/taucode/scripts/webkit-check.mjs`. Resolve `playwright` from
  `engines/browser/node_modules` via `createRequire`.
- It takes the base URL of a running UI preview (default `http://localhost:1421`) and an
  output directory.
- With WebKit at 1440×900 and 1024×700, it:
  - opens Settings (`?settings=appearance`), the theme editor (Duplicate & edit on Dark)
    and the Add engine modal (`?settings=engines`, then click Add engine)
  - asserts each modal card is centered within ±2px horizontally and vertically, and
    fully inside the viewport
  - for every built-in theme (`?theme=<id>`), loads Code mode and saves a screenshot to
    `<out>/<theme>-<w>x<h>.png`
  - exits non-zero on any assertion failure, printing the offending rects
- Add an npm script, `check:webkit`. Verify it only with `node --check` and the
  typecheck/lint.

## 2. Glass themes show no transparency

**Seen:** with Glass Dark active, the real app looks fully opaque.

**Why:**
- `glass-dark` / `glass-light` only make `--surface-sidebar` and `--surface-topbar` 88%
  opaque, over a dark vibrancy material, so they look the same as the opaque theme.
- The material's appearance isn't pinned. With a light system appearance the vibrancy
  would even be light under a dark theme.

**Fix:**
1. **Real translucency, still readable.** The vibrancy material blurs the desktop, which
   is what keeps text readable, so lean on it. Proposed values (tune them, but keep the
   tests):
   - Sidebar and title bar: alpha ~0.45 (dark: `rgba(24,24,28,0.45)`; light:
     `rgba(246,246,248,0.5)`).
   - Main app / messages surfaces: alpha ~0.72, with the material behind the whole
     window.
   - Cards and project cards inside the sidebar: alpha ~0.35 over the sidebar.
   - The composer, popovers, menus, modals (settings, editor), the side panel, toasts and
     code blocks stay ≥ 0.94, so typing and reading long output stay crisp.
   - Strong text on translucent surfaces. Muted text on the sidebar is lifted
     (dark ≥ `#c4c6ce`).
2. **Pin the material appearance:**
   - when a translucent theme is applied, also set the window theme
     (`window.set_theme(Some(Theme::Dark | Theme::Light))`) to match the theme's
     `appearance`
   - when an opaque theme is applied, clear the effect and restore
     `window.set_theme(None)`
   - Put this in the same `window_set_material` command, which now takes `material` and
     `appearance`.
3. **Confirm the effect applies.** Log at info level what was applied, and return an
   error string if `set_effects` fails. Keep the existing error toast. Make the
   `window_set_material` call idempotent and fast: it may be called on every theme switch.
4. **Contrast tests:** composite each translucent surface over the material's worst-case
   backdrop instead of pure white/black:
   - dark material: backdrops `#000000` and `#4a4a4e`
   - light material: `#ffffff` and `#bdbdc2`
   - body text ≥ 4.5:1, muted ≥ 3:1
   - Keep the existing rule that the composer, modals and popovers are ≥ 0.94 alpha.
5. **Gallery:** Glass card previews show a translucent hint (a faint diagonal/checker
   backdrop behind the preview's sidebar strip) so it reads as "glass".

## 3. New built-in theme: Cyberpunk Neon (`cyberpunk-neon`, dark)

**Palette:**

| Role | Values |
|---|---|
| App / messages | `#0b0714` |
| Sidebar / topbar | `#0e0a1c` |
| Cards / composer / popovers | `#160f2b`; raised `#1e1538` |
| Hover | `#24183f` |
| Active / selection | `#2d1450` |
| Text | primary `#ece8ff`, strong `#ffffff`, muted `#a89fd0` (≥ 4.5:1 on cards) |
| Primary accent | neon magenta `#ff2bd6` (send button, primary buttons, selected-row edge, `--border-accent`, active tree line) |
| Secondary accent | electric cyan `#00e5ff` (links, `--text-accent`, focus rings, tree lines at ~45% mix, inline code) |
| Highlight / warning | yellow `#fcee0a` |
| Status | success `#3dffa2`, error `#ff3b6b` |
| Borders | `#2b2150`, strong `#3d2f6e` |
| Code blocks | `#0f0a1f` background, syntax via the dark code theme with the accents above |

- **Fonts:** UI stays system sans; code mono.
- **Effects:** add a new effect, `effects.neonGlow` (boolean), to the theme schema (TS
  validator, Rust validator, shared fixtures, `docs/theme.schema.json`, `docs/themes.md`
  and the generated docs table). When on, it's static CSS only, with no animation and no
  per-frame work:
  - a soft glow on the accent: `box-shadow: 0 0 0 1px <accent>, 0 0 14px
    color-mix(in srgb, <accent> 45%, transparent)` on the primary buttons and the send
    button
  - focus rings glow cyan
  - the selected sidebar row gets a 2px magenta left edge with a faint glow
  - headings in assistant prose get `text-shadow: 0 0 10px color-mix(magenta 35%)`
  - `prefers-reduced-motion` doesn't matter here (static), but a "Glow effects" toggle
    appears in Appearance when the active theme has `neonGlow`, like the
    animated-background toggle
- **Contrast:** passes the same contrast suite as other built-ins (body ≥ 4.5, muted ≥ 3,
  the accent button text legible: use `#0b0714` text on the magenta, or white if it
  passes better; pick whichever is ≥ 4.5:1).
- **Gallery card:** shows the glow on the Send button in the preview.
- **Preview harness:** `?theme=cyberpunk-neon` works.

## Files

- `src/features/design-system/components/squircle/**`, `src/styles/**`,
  `src/features/themes/**`, `src/features/settings/**` (Appearance toggles only)
- `src-tauri/src/themes.rs` (material + appearance, validator effect),
  `docs/themes.md`, `docs/theme.schema.json`
- `apps/taucode/scripts/webkit-check.mjs`, `package.json` (script only)

**Off-limits:** engines, feature toggles and providers.

## Constraints and verification

- No new deps. Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md` (the permitted tailscale
  exclusion), plus `npm run build -- --mode uipreview` and
  `node --check scripts/webkit-check.mjs`.
- Don't run `tauri build`; no browser or computer use.
