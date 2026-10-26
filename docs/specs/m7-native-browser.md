# M7: native browser pane (an embedded WKWebView, not a screencast)

## Problem (the user's report, with a screenshot)

The side-panel browser looks **hazy** and feels **laggy** compared with Codex/Claude desktop. It errors on resize ("Screencast is already active · Retry"), and the toolbar buttons are misaligned.

Root cause (verified):
- The pane is a headless Chromium driven over CDP.
- `src-tauri/src/browser/bridge.rs` runs `Page.startScreencast` with `format: "jpeg", quality: 70` and draws base64 JPEG frames onto a `<canvas>` (`BrowserPane.tsx` → `drawBrowserFrame`).
- Every input event is a CDP round-trip.
- On resize, `attach`/`resize` restarts the screencast without stopping the running one.

## Goal

Make the browser pane a **real native web view embedded in the taucode window** (WKWebView on macOS, via Tauri's multi-webview `Window::add_child`), so it is crisp at Retina resolution, scrolls and types natively, and plays media. That's what Codex and Claude desktop do.

Agents drive the **same** native view through a private JS bridge, plus native APIs (navigate, snapshot, click, type, scroll, screenshot).

The existing headless Chromium/CDP backend stays available as an opt-in **"Headless Chromium (advanced)"** backend for agent tasks that need trusted input. Native is the default.

Done means:
- The panel shows a crisp native page that tracks its rect on every layout change.
- No haze or lag.
- No screencast errors.
- Agent browser tools work against it.
- It's safe:
  - no Tauri IPC from web content
  - an isolated data store
  - a navigation policy
- Tests cover all of the above.

## Facts (verified offline)

- `tauri 2.10.3` has `Window::add_child(WebviewBuilder, position, size)` behind the `unstable` feature (`unstable = ["tauri-runtime-wry?/unstable"]`), with **no new crates**. Add `"unstable"` to the tauri features in `src-tauri/Cargo.toml`.
- `block2 0.6.2` and `objc2-web-kit 0.3.2` are in Cargo.lock / the offline cache. Enable only the `objc2-web-kit` features you need:
  - `WKWebView` `evaluateJavaScript:completionHandler:`
  - `takeSnapshotWithConfiguration:completionHandler:`
  - `WKWebsiteDataStore` / the identifier data store (macOS 14+)
- Capabilities: `src-tauri/capabilities/default.json` scopes by `"windows": ["main", "about"]`.
  - A child webview inside the `main` window would match a window-scoped capability.
  - Tauri already blocks IPC for remote origins unless a capability declares `remote.urls`. Still add **defense in depth**: scope app capabilities to the app's own **webview labels** (`"webviews": ["main", "about"]` or the equivalent Tauri 2 field), so `browser-*` webviews match no capability.
  - Add a test asserting that no capability matches a `browser-` label and none declares `remote`.

## Design

### 1. Native pane lifecycle (Rust: `src-tauri/src/browser/native/**`)

- **One child webview per browser tab:**
  - label `browser-<tabId>`
  - created lazily on first show via `window.add_child(WebviewBuilder::new(label, WebviewUrl::External(url)) …)`
  - built with:
    - `.data_store_identifier(<stable uuid>)` (macOS 14+; on older systems use a non-persistent store), so cookies and storage are isolated from the app's own webview and persist across launches
    - `.incognito(true)` when the browser setting "Private browsing" is on
    - `.user_agent(<Safari-like UA>)`
    - devtools disabled in release
    - transparent background off
- **Commands:**
  - `browser_native_attach {tabId, url?, rect: {x,y,w,h}, visible}`
  - `browser_native_bounds {tabId, rect, visible}`
  - `browser_native_navigate/back/forward/reload/stop`
  - `browser_native_close`
  - `browser_native_snapshot_image`
  - Positions are logical px relative to the window's content view.
  - Only the active tab's webview is visible; the others are `hide()`n (not destroyed), with a cap of 8 live webviews (LRU destroys the oldest hidden one, keeping its URL for restore).
- **Events to the frontend:** url, title, favicon, loading progress, canGoBack/Forward, a new-window request (opens as a new tab), downloads (`on_download`: save to Downloads with the existing confirm flow), and crashes (with a reload offer).
- **Navigation policy** (`on_navigation`):
  - allow `http`, `https`, `about:blank`, `data:` (images only), `blob:` (same-origin)
  - block `file:`, `tauri:`, `asset:`, `ipc:` and the app's custom schemes
  - other schemes (mailto:, etc.) ask to open externally
  - Test the policy function.

### 2. Placement and overlays (frontend)

- `BrowserPane` no longer renders a canvas in native mode. It renders a measured placeholder `<div>` (the "native slot") and reports its client rect on:
  - ResizeObserver
  - window resize
  - sidebar or side-panel resizer drags (rAF-throttled)
  - panel expand/collapse
  - mode switches
  - scroll of any ancestor
- The rect sent to Rust is rounded to whole logical px. The bounds update must be cheap: batched once per frame.
- **Native views draw above HTML,** so any HTML overlay that intersects the pane would be hidden beneath it. Add a tiny overlay registry:
  - `ModalShell`, `AnchoredPopover`, context menus, the command palette, toasts over the pane, and active drag ghosts register while open
  - if any registered overlay rect intersects the native slot, or the panel is hidden, another tab is active, or the window is minimized, the native view is **hidden** and the slot shows the **last native snapshot** (`takeSnapshot` PNG, captured just before hiding), so the swap is visually seamless
  - when overlays close, show the view again
  - Test the intersection logic.
- **No flashes:** the slot has the theme's surface color; first paint waits for the webview's first load event before revealing.

### 3. Toolbar and tab chrome (fix the alignment the user flagged)

- One row, 36px tall, everything vertically centered:
  - back, forward, reload/stop: 28px icon buttons, 16px icons
  - a 30px-tall address field (flex 1, 8px radius, a lock icon for https, select-all on focus, Enter navigates)
  - downloads: 28px
  - more (⋯): 28px, with Open in default browser, Copy URL, Clear site data, Zoom ±
- A 2px loading progress bar under the row.
- No error row. Errors show inline in the page area as a calm card with Retry.
- **Tabs:** favicon + title (ellipsis) + close; the active tab is highlighted; a "+" for a new tab.
- Use the shared IconButton sizes from the design system (`--icon-button-28`); create them if missing, consistent with the M5n alignment pass. Keep visuals consistent across themes.

### 4. Agent control through the native view

- Define a `BrowserBackend` trait used by the existing agent browser tools (the MCP browser server and connector plumbing; keep the tool names and schemas stable): `navigate`, `snapshot` (an accessibility-like tree with stable refs), `click(ref)`, `type(ref, text, submit?)`, `press(key)`, `select(ref, value)`, `scroll`, `screenshot`, `tabs` (list/new/activate/close), `wait_for(text|ref)`.
- **NativeBackend:**
  - `navigate` / `tabs` go through the native commands.
  - `snapshot` / `click` / `type` / `press` / `select` / `scroll` / `wait_for` run a **private bridge script** (`src-tauri/src/browser/native/bridge.js`, included with `include_str!`) through WKWebView `evaluateJavaScript:completionHandler:` (objc2 + block2) or `callAsyncJavaScript`, in an isolated content world (`WKContentWorld` non-page world) so page scripts can't see or tamper with it.
    - `snapshot` assigns `data-taucode-ref` ids (or a WeakMap in the isolated world) and returns role/name/value/bounds for interactive and landmark elements, capped (1,000 nodes / 200 KB).
    - `click` focuses, scrolls into view and dispatches pointer, mouse and click events.
    - `type` uses the native value setter + input/change events, or `execCommand('insertText')` for contenteditable.
    - `press` dispatches keyboard events for common keys.
  - `screenshot` uses `takeSnapshotWithConfiguration:` (PNG, Retina).
  - Document the limitation: synthetic events aren't `isTrusted`. When a tool call fails because a site requires trusted input, the error says "Try the Headless Chromium backend (Settings → Browser)".
- **CdpBackend:** the existing implementation. Also fix the screencast bug: `attach`/`resize` must stop an active screencast before starting another (or just update the viewport and keep it running), and use quality 90 at the device pixel ratio. Even when not the default, it shouldn't error.
- **Setting:** Settings → Browser, "Engine": Native (default) / Headless Chromium (advanced; a note on why you'd pick it). Agent tools use the selected backend. The visible pane always shows the native view in Native mode; in Headless mode it shows the CDP screencast (the existing canvas path) with the fixed bug.
- **Agent takeover UX:** while an agent drives the native view, show the existing "agent active" frame and a "Take over" button around the slot (HTML outside the native rect). User input into the native view while the agent is acting pauses the agent (the existing takeover semantics).

### 5. Platforms

- macOS: native as above.
- Windows: `add_child` gives WebView2. Implement the same, minus the objc parts (use `ExecuteScript` via the webview2 interface if readily available offline; otherwise the native pane works and agent tools fall back to CDP on Windows).
- Linux: fall back to the CDP pane.

## Tests

- Rust:
  - the navigation policy
  - the capability isolation test (no capability matches `browser-*`, no `remote`)
  - label and bounds validation
  - LRU of live webviews
  - the backend trait with a fake
  - the CDP resize fix (a mock CDP: `attach` → `resize` doesn't call `startScreencast` twice without a stop)
- Frontend:
  - the native slot reports rounded rects and batches per frame
  - the overlay registry hides the view and shows the snapshot when a modal or popover intersects, and restores after
  - hidden when the panel is collapsed or another tab is active
  - toolbar layout: every control's vertical center within 0.5px of the row's center (jsdom geometry is limited; assert classes and sizes, and add the geometry check to `scripts/webkit-check.mjs` with the preview mock of the native slot)
- Bridge script (jsdom):
  - `snapshot` produces refs and roles
  - `click` triggers handlers
  - `type` updates the input value and fires events
  - caps enforced
- The preview harness renders the native slot as a placeholder (the mock shows a static page image) so UI checks work.

## Files

- `src-tauri/Cargo.toml` (tauri `unstable` feature; objc2-web-kit features), `src-tauri/src/browser/**`, `src-tauri/capabilities/*.json`, `src-tauri/src/lib.rs` (commands)
- `src/features/browser/**`, `src/features/panels/SidePanel.tsx` (slot integration only), the overlay registry in the design system (`ModalShell`, `AnchoredPopover` register/unregister)
- The browser settings, `src/dev/uiPreview/**`, `docs/browser.md`

**Off-limits:** the header/title-bar actions, sidebar footer, composer toolbars and general icon-button sizing outside the browser. A parallel M5n alignment pass owns those; coordinate only through a shared `--icon-button-*` token if you need one (create it in `styles/ds-tokens.css` if absent, and keep the names `--icon-button-24|28|32`).

## Constraints and verification

- Offline. No new crates except enabling features of the already-cached `tauri`, `objc2-web-kit` and `block2`. `CARGO_HOME=/Users/rolandleyco/taucode-m7/.cargo-home`, `$HOME/.cargo/bin` on PATH, `TMPDIR=/Users/rolandleyco/taucode-m7/.tmp`.
- Verification: the command in `docs/specs/m3-computer-use.md` from `/Users/rolandleyco/taucode-m7` (the permitted tailscale exclusion), plus `npm run build -- --mode uipreview` and `node --check apps/taucode/scripts/webkit-check.mjs`.
- No browser or computer use: the reviewer verifies the native pane in the real app. Don't commit.
