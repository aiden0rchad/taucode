# M5a: themes framework (built-in Glass, Matrix and Claude themes; user themes you can create, import, export and share)

## Goal

Appearance becomes a **theme system**, not a fixed light/dark/dim switch:
- taucode ships built-in themes: System, Light, Dark, Dim, **Glass** (the old translucent
  look, back as an opt-in theme), **Matrix**, **Claude Light** and **Claude Dark**.
- Users can **add and remove** themes: create one from any theme (duplicate and edit),
  import a theme file or pasted JSON, export or copy it to share, and delete custom themes.
- Developers get a documented, validated theme format with hot reload, so the community
  can build and share themes.

Done means:
- Settings → Appearance shows a theme gallery with live mini previews.
- Switching themes is instant and needs no restart, including turning window
  translucency on (Glass) and off.
- All built-ins pass contrast tests.
- `docs/themes.md` and `docs/theme.schema.json` exist.

## Current state (read first)

- `src/styles/themes.css` imports `themes.{dark,dim,light,system}.css`. These define CSS
  custom properties under `:root[data-theme=…]`.
- `src/features/layout/hooks/useThemePreference.ts` sets `document.documentElement.dataset.theme`.
- `ThemePreference = "system" | "light" | "dark" | "dim"` (`src/types.ts`) is stored in
  `AppSettings.theme`, with the Rust mirror in `src-tauri/src/types.rs`. The picker lives in
  `SettingsDisplaySection.tsx`.
- Tests that guard tokens already exist (`src/styles/*.test.mjs`: contrast, opaque
  surfaces, undefined CSS vars, layers). Keep them passing; extend them per theme.
- **Translucency history:**
  - Before commit `8980f31` the window was transparent and used
    `tauri-plugin-liquid-glass` with `useLiquidGlassEffect.ts` (`macOSPrivateApi: true`,
    `"transparent": true`).
  - M4l removed it because text was hard to read over the desktop. Inspect it with
    `git show 608a4d6:apps/taucode/src/features/app/hooks/useLiquidGlassEffect.ts`,
    `git show 608a4d6 -- apps/taucode/src-tauri/tauri.conf.json` and the CSS at `608a4d6`.
  - `scripts/opaque-surfaces.mjs` and `src/styles/opaqueSurfaces.test.mjs` enforce opaque
    surfaces today. They must keep enforcing that for every non-translucent theme.

## Design

### Theme manifest (v1)

A theme is one JSON file (`*.taucode-theme.json`):

```json
{
  "$schema": "https://taucode.dev/theme.schema.json",
  "schemaVersion": 1,
  "id": "matrix",
  "name": "Matrix",
  "author": "taucode",
  "version": "1.0.0",
  "description": "Phosphor green on black.",
  "appearance": "dark",
  "extends": "dark",
  "tokens": { "--surface-app": "#030803", "--text-strong": "#b8ffc9" },
  "fonts": { "ui": "…stack…", "mono": "…stack…", "prose": "…stack…" },
  "window": { "translucent": false, "material": "none" },
  "effects": { "digitalRain": false }
}
```

- `extends` is a built-in theme id. The theme inherits every token and overrides only the
  ones it lists, so a minimal theme is a few lines.
- `appearance` drives `color-scheme`, the code-highlight theme (`diffViewerTheme.ts`) and
  terminal colors.
- **Token allowlist.** Only themeable tokens are accepted. Generate the list from the
  tokens the dark theme defines and export it from one module (e.g.
  `src/features/themes/tokens.ts`) with a group and label for each token: Surfaces, Text,
  Accent, Borders, Status, Tree, Code, Terminal, Radii. Unknown tokens are rejected with a
  readable error.
- **Value validation (security: themes are shared files):**
  - Accept only colors (`#rgb[a]`, `#rrggbb[aa]`, `rgb()/rgba()/hsl()/hsla()/oklch()`),
    `color-mix(...)` of colors or `var(--allowed-token)`, and px/rem lengths for radius
    tokens.
  - Reject `url(`, `image-set(`, `expression`, `@`, `;`, `{`, `}`, `\`, `<` and values over
    120 chars.
  - Font stacks may name only local font families: no `url`, no `@font-face`, and only
    quoted names, generic families and commas.
  - No custom CSS and no scripts. This rules out network beacons and CSS-attribute
    exfiltration from a shared theme.
- `window.translucent` (bool) with `material`: `"none" | "sidebar" | "underWindow" |
  "hudWindow"`.
- `effects.digitalRain` is the only effect in v1. It's Matrix-only by default, but any
  theme may enable it.
- `schemaVersion` gates future changes. A higher major version is rejected with "made for
  a newer taucode".

### Applying a theme

- **One runtime applier** (`src/features/themes/applyTheme.ts`) resolves `extends`, then:
  - writes the tokens into a single `<style id="taucode-theme">`, as `:root{…}` scoped by
    `data-theme-id`
  - sets `data-theme` (the base appearance, `light` or `dark`, so the existing appearance
    selectors keep working) and `data-theme-id`
  - sets `color-scheme`
  - Built-in themes may stay as CSS files, but they must also exist as manifests (generate
    the manifests or derive the CSS from them; one source of truth), so built-ins and user
    themes go through the same path.
- **System** stays special: it follows the OS between a chosen light theme and a chosen
  dark theme. Add two settings, `themeLight` (default `light`) and `themeDark` (default
  `dark`), so a user can pair Claude Light with Claude Dark.
- **Settings migration:** `AppSettings.theme` becomes a theme id string. Old values
  (`system`, `light`, `dark`, `dim`) still work unchanged. An unknown or deleted id falls
  back to `system` with a toast. Update the Rust type to a `String` with a default of
  `"system"`.

### Window translucency (Glass)

- Re-enable a transparent-capable window: `"transparent": true`, `macOSPrivateApi: true`.
- Prefer Tauri's built-in `Window::set_effects` / `EffectsBuilder` (window-vibrancy is
  already in Cargo.lock) **over** re-adding `tauri-plugin-liquid-glass`. If the built-in
  API can't give an acceptable macOS material, `tauri-plugin-liquid-glass 0.1.6` is in the
  offline cargo cache and may be re-added; say which one you used.
- A Tauri command `window_set_material(material: Option<String>)` applies or clears the
  effect at runtime. With no material, the window shows nothing of the desktop because
  every opaque theme paints opaque `html, body` and app shell backgrounds.
  - Add a test that, for every opaque theme, `--surface-app` / `--surface-sidebar` have
    alpha 1 and the shell paints them.
  - Keep `opaqueSurfaces.test.mjs` passing for every non-translucent theme.
- **Glass readability (the reason it was removed):**
  - Only the sidebar and title bar are translucent (vibrancy, sidebar material).
  - The conversation, composer, popovers, menus, settings and side panel stay at least 92%
    opaque.
  - Text on translucent surfaces gets the strong text token.
  - A contrast test composites each Glass surface over both pure black and pure white and
    requires ≥ 4.5:1 for body text and ≥ 3:1 for muted text in both cases.
  - Provide Glass in dark and light appearance, as two built-ins: `glass-dark` and
    `glass-light`.
- Resource-lean: no effect is applied unless a translucent theme is active. Switching away
  clears it.

### Built-in themes

- **Dark, Light, Dim, System:** unchanged look. Pixel parity: the token values stay
  identical.
- **Glass Dark / Glass Light:** as above.
- **Matrix** (dark):
  - near-black green-tinted surfaces; phosphor green accent (`#00ff41` family); readable
    body text (soft green-white, ≥ 7:1)
  - mono font for UI and prose (SF Mono / Menlo stack)
  - subtle text-shadow glow on the active row and headings only
  - green tree lines and selection
  - `effects.digitalRain: true`: a lightweight canvas "digital rain" drawn **only** in the
    empty states (new-thread welcome, empty chat, empty work). It runs at ≤ 20 fps, low
    opacity (~0.12) and behind content. It pauses on `document.hidden` and when the
    element is offscreen (IntersectionObserver). It is fully disabled under
    `prefers-reduced-motion` and by a per-theme "Animated background" toggle in
    Appearance.
- **Claude Light / Claude Dark:** warm neutral palette in the spirit of claude.ai. These
  are community-approximated values, not official brand assets; don't ship any logos.
  - Light:
    - surfaces: background `#faf9f5`, sidebar `#f5f4ed`, cards `#ffffff`, raised `#f0eee6`
    - text: `#141413`, muted `#5e5d59`
    - accent: clay `#c96442` (hover `#d97757`)
    - borders: `#e8e6dc`
  - Dark:
    - surfaces: background `#262624`, sidebar `#1f1e1d`, cards `#30302e`
    - text: `#f5f4ef`, muted `#b7b5a9`
    - accent: `#d97757`
    - borders: `#3e3e38`
  - Prose (assistant messages) uses a serif stack: `ui-serif, "New York", "Iowan Old
    Style", Georgia, serif`. UI stays sans. The send button and the selected states use
    the accent.
  - Must meet the same contrast tests.

### User themes and sharing

- **Storage:** `<app data dir>/themes/*.taucode-theme.json`. New Tauri commands (with
  tests):
  - `themes_list` → manifests plus validation errors per file
  - `themes_save(manifest)` → validates on the Rust side too; writes atomically to
    `<id>.taucode-theme.json`; ids are `[a-z0-9-]{1,40}`; built-in ids are reserved
  - `themes_delete(id)` (custom only)
  - `themes_import(path)` (copies after validation; on an id clash, ask whether to replace
    or keep both, which suffixes `-2`)
  - `themes_export(id, path)`
  - `themes_open_folder`
  - Validation lives in one shared place on each side, and both sides use the same rules:
    a Rust validator plus a TS validator, tested against the same fixture files under
    `src/features/themes/fixtures/`.
- **Hot reload for theme developers:** watch the themes folder (debounced 200ms) and
  emit `themes-changed`. If the active theme file changes, re-apply it immediately. A
  broken file shows its validation error in the gallery and is skipped; it never breaks
  the app.
- **Settings → Appearance** (use the existing SettingsPage/Group/Row primitives):
  - **Theme gallery:** a grid of cards, each with a mini preview (sidebar strip, a message
    bubble, an accent button and a tree line, rendered from the theme's tokens), the
    name, the author and a check on the active theme. Click to apply.
  - System row: "Light theme" and "Dark theme" selects (shown when System is chosen).
  - Card ⋯ menu:
    - Duplicate & edit
    - Export…, which uses the save dialog (`tauri-plugin-dialog` is already present)
    - Copy JSON
    - Delete (custom only, with a confirm)
  - Header buttons: **New theme** (duplicates the active theme), **Import…** (file
    dialog), **Paste theme** (a modal with a textarea that validates and lists errors
    inline), **Open themes folder**.
  - **Theme editor** (modal or settings sub-page):
    - name, author, description, appearance, extends
    - a color picker plus hex input for each allowlisted token, grouped and collapsible,
      with a search box
    - font stack selects (System, Mono, Serif, or custom text validated as above)
    - a translucency/material select
    - an animated-background toggle
    - **live preview:** apply as you edit; Cancel restores the previous theme
    - contrast warnings inline when a text/surface pair drops below 4.5:1
    - Save writes the file
  - The animated-background toggle lives on the Appearance page and shows only when the
    active theme has an effect.
- **docs/themes.md** (developer guide):
  - the format, every field, the token reference table (generated from the allowlist by a
    script `npm run themes:docs`, with a test that the doc is up to date)
  - an example theme
  - the hot-reload workflow, validation rules and why there's no custom CSS
  - how to share (file or JSON)
- **docs/theme.schema.json:** a JSON Schema for the manifest, so editors autocomplete.
  Built-in manifests reference it.

### Preview harness

- Add `?theme=<id>` support for every built-in to the UI preview (`src/dev/uiPreview/**`).
- Fixtures for one custom theme, plus one invalid theme file to show the error state in
  the gallery.

## Files

`apps/taucode/src/styles/**`, new `src/features/themes/**`,
`src/features/layout/hooks/useThemePreference.ts`, `SettingsDisplaySection.tsx` (or a new
Appearance section in the settings nav), `src/types.ts`, `src/services/tauri.ts`,
`src/dev/uiPreview/**`, `diffViewerTheme.ts`, terminal theme hookup, `src-tauri/src/**`
(a new `themes` module, window material command, types), `src-tauri/tauri.conf.json`,
capabilities (only what the new commands need), `docs/themes.md`,
`docs/theme.schema.json`, `package.json` (the `themes:docs` script only).

**Off-limits:** engines, providers, git, sidebar structure and the composer layout. Don't
change existing Dark/Light/Dim/System token values.

## Constraints

- No new npm dependencies. A color picker is `<input type="color">` plus a hex field.
- No new Rust crates unless the built-in Tauri effects API is insufficient; then only the
  cached `tauri-plugin-liquid-glass 0.1.6`.
- Offline. `CARGO_HOME=/Users/rolandleyco/taucode-m5a/.cargo-home` with
  `$HOME/.cargo/bin` on PATH.
- Respect `prefers-reduced-motion`. No per-frame work when no effect is active.
- Follow the existing code style (compact TS, `features/<area>` layout, settings
  primitives, z-index variables).

## Tests (add)

- The validator, TS and Rust on shared fixtures:
  - accepts valid themes
  - rejects `url(`, unknown tokens, bad ids, oversized values, a newer schemaVersion and
    unsafe font stacks
- `extends` resolution and token merging; System pairing; migration of old `theme`
  values; a deleted active theme falls back to System.
- Contrast for every built-in (including the Glass composite over black and white, and
  Matrix body text ≥ 7:1).
- Opaque themes paint opaque shell surfaces; translucent themes keep the conversation and
  composer ≥ 0.92 alpha.
- Rust: save/list/delete/import/export round trip in a temp dir; reserved ids; atomic
  write; the folder watcher emits on change (use an injectable watcher or poll for the
  test).
- Gallery: renders cards, applies on click, Duplicate & edit opens the editor, Delete is
  hidden for built-ins, and Paste theme shows validation errors.
- Digital rain:
  - mounts only when the effect is on, the theme is active and the view is an empty state
  - not mounted under reduced motion
  - stops when `document.hidden`
  - fps capped (mock rAF/timers)
- The docs token table is up to date.

## Verification

The command in `docs/specs/m3-computer-use.md` (default Rust parallelism, the permitted
tailscale exclusion), plus `npm run build -- --mode uipreview`. Use this worktree's
`CARGO_HOME`. Don't run `tauri build`; no browser or computer use.

## Out of scope

Feature toggles and engines (a parallel M5b), a theme marketplace or online gallery,
custom CSS, background images and custom icon sets.
