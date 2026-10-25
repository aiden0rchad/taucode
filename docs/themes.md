# taucode themes

Choose a theme in Settings → Appearance. System follows the OS using your Light theme and Dark theme selections. Glass uses Tauri's built-in window material API; only its sidebar and title bar reveal the desktop. Claude palettes are community approximations, not official brand assets.

## Format (v1)

Save a UTF-8 `*.taucode-theme.json` file:

```json
{
  "$schema": "https://taucode.dev/theme.schema.json",
  "schemaVersion": 1,
  "id": "my-green",
  "name": "My Green",
  "author": "Your name",
  "version": "1.0.0",
  "description": "A green accent on Dark.",
  "appearance": "dark",
  "extends": "dark",
  "tokens": { "--text-accent": "#80efac" },
  "fonts": { "ui": "system-ui", "mono": "monospace", "prose": "serif" },
  "window": { "translucent": false, "material": "none" },
  "effects": { "digitalRain": false }
}
```

`schemaVersion`, `id`, `name`, `appearance` and `tokens` are required. IDs contain 1–40 lowercase ASCII letters, digits or hyphens. Built-in IDs are reserved. `author`, `version` and `description` are optional metadata. `$schema` enables editor completion. `extends` selects a built-in except System; without it, the appearance selects Light or Dark. Tokens, fonts, window and effects inherit from that base. Appearance is `light` or `dark` and controls native color scheme, syntax highlighting and terminal colors. Font keys are `ui`, `mono`, `prose`. Window material is `none`, `sidebar`, `underWindow` or `hudWindow`; translucency must be enabled to apply a material. `effects.digitalRain` enables an empty-state animation, capped at 20 fps and disabled by reduced motion or the per-theme Animated background preference.

## Validation and safety

Both TypeScript and Rust validate shared fixtures. Colors accept hex (3, 4, 6 or 8 digits), numeric rgb/rgba/hsl/hsla/oklch functions, color-mix in srgb of colors, and references to allowlisted tokens. Radius tokens accept nonnegative px/rem lengths. Unknown tokens, reference cycles, values longer than 120 characters, `url(`, `image-set(`, `expression`, `@`, `;`, braces, backslash and `<` are rejected. Font stacks accept quoted local family names and CSS generic families, separated by commas. No remote fonts, custom CSS, scripts, images or network resources are permitted. Newer schema versions require a newer taucode.

## Create, develop and share

Duplicate any gallery card or choose New theme. Edit tokens, fonts and effects with live preview; Cancel restores the selected theme. Save writes atomically to the app data directory's `themes/<id>.taucode-theme.json`. Open themes folder locates it. Edit that file externally: file size and modification time are polled once per second; only changed files are read, and notifications are debounced for 200 ms, then the active theme reloads. Invalid files appear as errors in the gallery; the last valid active theme remains until fixed. Deleting the selected theme falls back to System with a notification.

Export a card as a file or Copy JSON. Recipients use Import or Paste theme. ID conflicts offer Replace or Keep both; Keep both adds a numeric suffix. Built-ins cannot be overwritten or deleted.

## Token reference

Generated from the Dark theme's themeable color and radius properties. Layout dimensions and typography metrics are deliberately excluded. Run `npm run themes:docs` from `apps/taucode` after changing the allowlist source. The same command generates the Dark, Light and Dim palettes from their CSS and updates the schema's token properties. Tests reject stale generated palettes, allowlists and documentation.

<!-- tokens:start -->
| Token | Group | Label |
| --- | --- | --- |
| `--surface-app` | Surfaces | surface app |
| `--text-primary` | Text | text primary |
| `--text-strong` | Text | text strong |
| `--text-emphasis` | Text | text emphasis |
| `--text-stronger` | Text | text stronger |
| `--text-quiet` | Text | text quiet |
| `--text-muted` | Text | text muted |
| `--text-subtle` | Text | text subtle |
| `--text-faint` | Text | text faint |
| `--text-fainter` | Text | text fainter |
| `--text-dim` | Text | text dim |
| `--surface-sidebar` | Surfaces | surface sidebar |
| `--tree-line` | Tree | tree line |
| `--tree-line-active` | Tree | tree line active |
| `--surface-sidebar-opaque` | Surfaces | surface sidebar opaque |
| `--surface-topbar` | Surfaces | surface topbar |
| `--surface-right-panel` | Surfaces | surface right panel |
| `--surface-composer` | Surfaces | surface composer |
| `--surface-messages` | Surfaces | surface messages |
| `--surface-card` | Surfaces | surface card |
| `--surface-card-strong` | Surfaces | surface card strong |
| `--surface-card-muted` | Surfaces | surface card muted |
| `--surface-item` | Surfaces | surface item |
| `--surface-control` | Surfaces | surface control |
| `--surface-control-hover` | Surfaces | surface control hover |
| `--surface-control-disabled` | Surfaces | surface control disabled |
| `--surface-hover` | Surfaces | surface hover |
| `--surface-active` | Surfaces | surface active |
| `--surface-approval` | Surfaces | surface approval |
| `--surface-debug` | Surfaces | surface debug |
| `--surface-command` | Surfaces | surface command |
| `--surface-diff-card` | Surfaces | surface diff card |
| `--surface-bubble` | Surfaces | surface bubble |
| `--surface-bubble-user` | Surfaces | surface bubble user |
| `--surface-context-core` | Surfaces | surface context core |
| `--surface-popover` | Surfaces | surface popover |
| `--surface-review` | Surfaces | surface review |
| `--border-review` | Borders | border review |
| `--surface-review-active` | Surfaces | surface review active |
| `--text-review-active` | Text | text review active |
| `--surface-review-done` | Surfaces | surface review done |
| `--text-review-done` | Text | text review done |
| `--border-subtle` | Borders | border subtle |
| `--border-muted` | Borders | border muted |
| `--border-strong` | Borders | border strong |
| `--border-stronger` | Borders | border stronger |
| `--border-quiet` | Borders | border quiet |
| `--border-accent` | Accent | border accent |
| `--border-accent-soft` | Accent | border accent soft |
| `--text-accent` | Accent | text accent |
| `--message-link-color` | Accent | message link color |
| `--shadow-accent` | Accent | shadow accent |
| `--status-success` | Status | status success |
| `--status-warning` | Status | status warning |
| `--status-error` | Status | status error |
| `--status-unknown` | Status | status unknown |
| `--select-caret` | Surfaces | select caret |
| `--surface-text-accent` | Accent | surface text accent |
| `--surface-text-muted` | Surfaces | surface text muted |
| `--surface-border-subtle` | Surfaces | surface border subtle |
| `--surface-border-strong` | Surfaces | surface border strong |
| `--surface-border-accent` | Accent | surface border accent |
| `--surface-border-accent-soft` | Accent | surface border accent soft |
| `--surface-text-dim` | Surfaces | surface text dim |
| `--surface-status-error` | Surfaces | surface status error |
| `--surface-status-success` | Surfaces | surface status success |
| `--surface-status-warning` | Surfaces | surface status warning |
| `--surface-status-unknown` | Surfaces | surface status unknown |
| `--surface-border-muted` | Surfaces | surface border muted |
<!-- tokens:end -->
