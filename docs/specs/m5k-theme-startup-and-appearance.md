# M5k: theme startup order and a single window-appearance path

Two small cleanups in /Users/rolandleyco/taucode-m1w (taucode).

## 1. Glass flickers at startup (the default theme is applied before the saved one)

Real app log at every launch, with the saved theme `glass-dark`:
```
{"event":"window_material_applied","material":null,...}        ← default/opaque theme applied first
{"event":"window_material_applied","material":"sidebar","liquidGlass":true,"appearance":"dark"}
{"event":"window_material_applied","material":null,...}        ← applied again (settings reload?)
{"event":"window_material_applied","material":"sidebar","liquidGlass":true,"appearance":"dark"}
```

So the window goes opaque → glass → opaque → glass during startup.

Fix:
- **Apply the saved theme first.** Persist the last-applied resolved theme manifest in
  `localStorage` (`taucode.theme.last`, with its id and a hash). Before React mounts
  (`main.tsx`), apply it synchronously, CSS vars only, and issue a single
  `window_set_material` for it.
- When settings load, re-apply only if the id or the content changed.
- **Make material application idempotent:** if the requested `(material, appearance)`
  equals the last applied, don't call the backend. The applier already dedupes
  `lastMaterial`; make sure the startup path and the settings path share that state, and
  that a full re-apply of the same theme never sends `null` first.
- Expected log at launch with glass-dark saved: exactly **one**
  `window_material_applied` with `material: "sidebar"`.
- **Tests:**
  - with a saved theme in `localStorage`, the first `windowSetMaterial` call is
    `('sidebar','dark')`, and no `null` call precedes it
  - settings loading the same theme causes no further calls
  - switching to an opaque theme sends exactly one `null` call

## 2. One owner for the window appearance

- `src-tauri/src/window.rs::apply_window_appearance` is a legacy CodexMonitor path,
  called from `settings/mod.rs:31` (get) and `:69` (update). It now no-ops for theme ids
  other than `system|light|dark|dim`. For those four it still calls `window.set_theme`
  and sets the NSWindow appearance, which duplicates and can race the frontend applier's
  `window_set_material(material, appearance)`.
- Make the frontend theme applier the **single owner**:
  - for **every** theme (including opaque ones), `window_set_material` receives the
    resolved `appearance`: `light` / `dark`, or `null` for System (so the window follows
    the OS)
  - it sets the window theme accordingly, even when `material` is null
- Remove the calls from `settings/mod.rs` and delete `apply_window_appearance` (and its
  test override plumbing) if nothing else uses it. Keep the macOS NSAppearance code only
  if `window.set_theme` alone doesn't update the native title bar/traffic lights
  correctly; if needed, move it into `window_set_material`.
- **Daemon/headless mode:** there's no window; make sure nothing there depends on this.
- **Tests:**
  - settings get/update no longer touch the window appearance
  - `window_set_material` sets the theme for opaque themes too (mock or unit-test the
    appearance mapping)
  - System passes `null`

## Verification

The command in `docs/specs/m3-computer-use.md`, with
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home` and `$HOME/.cargo/bin` on PATH
(the permitted tailscale exclusion), plus `npm run build -- --mode uipreview`. Don't run
`tauri build`; no browser or computer use. Don't commit.

**Off-limits:** evals, packaging, bundling.
