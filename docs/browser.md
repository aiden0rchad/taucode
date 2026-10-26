# Browser pane

Settings → Browser → Engine defaults to **Native**. On macOS the pane is a child
WKWebView, rendered at the display's resolution with native scrolling, typing and
media. **Headless Chromium (advanced)** retains the Chromium/CDP pane for sites
that need trusted automation input. Its JPEG stream uses quality 90 and device
pixel ratio; resizing stops an existing screencast before restarting it.

The browser toolbar has 28px buttons in a 36px row, a 30px address field, and a
loading indicator. Its menu opens the current URL externally, copies it, clears
site data after confirmation, and adjusts zoom. Downloads require confirmation
and are saved to Downloads without overwriting an existing file.

## Native lifecycle

Tabs have `browser-<id>` webview labels. Only the active pane is shown. Hidden tabs
retain their page state, up to eight live views; the oldest hidden view is evicted
and its URL retained for restoration. The slot reports rounded logical-pixel
bounds at most once per animation frame, including during panel resizing,
transitions, scrolling and mode changes. Unchanged bounds do not send IPC.

ModalShell, PopoverSurface (including AnchoredPopover and context menus), toasts,
and drag images participate in the native overlay registry. An intersecting
overlay hides the child view, displaying its last PNG snapshot underneath the
HTML. The child is also hidden on pane unmount, tab changes, window hiding and
minimization. The first load stays hidden until WebKit reports completion.
Native navigation failures and page process termination offer Retry in the page
area. A forwarding delegate preserves Wry’s navigation, download and authentication
policy while observing these failures.

## Isolation

App capabilities name only the `main` and `about` **webviews**, with no remote
origins or window-wide grants. A second guard rejects app commands from every
`browser-*` webview. Browser content cannot invoke the app's commands or plugins.

On macOS 14 and later, browser views use a stable, separate WKWebsiteDataStore
identifier. Older macOS versions and Private browsing use non-persistent stores.
Changing Private browsing closes the live views and restores their URLs in the
new store. Settings → Browser → Clear browsing data clears the isolated store;
the toolbar's Clear site data removes the current site's WebKit data records
(WebKit groups these by registrable domain).

Navigation permits HTTP(S), `about:blank`, raster image data URLs and same-origin
blob URLs. File, executable, app, IPC and asset URLs are blocked, including
Tauri's localhost scheme aliases. Other protocols prompt before opening externally.
Allowed/blocked origin preferences also apply to native navigation. Native origin
patterns accept exact origins, `*`, and `*.example.com` host suffixes.

## Agent control

The app's private local browser socket routes the pane and MCP browser tools to
the selected backend. On Unix the desktop host has a separate `native-host`
endpoint, so a daemon or MCP process cannot accidentally claim its native pane.
The launcher preserves Playwright's existing tool names and schemas.

Native navigation, tabs, screenshots, snapshots, clicks, typing, key presses,
selection and waits use the same view the user sees. DOM actions run in the
retained `taucode.browser.private` WKContentWorld. Page JavaScript cannot access
or replace the bridge or its WeakMap refs. Snapshots contain roles, names, values
(except passwords), refs and bounds, capped at 1,000 nodes and 200 KB. Stale refs
require a fresh snapshot. Screenshots use WebKit's PNG snapshot API.

Native DOM input is synthetic and **is not `isTrusted`**. Sites can ignore it;
results and errors advise **“Try the Headless Chromium backend (Settings →
Browser)”**. CDP-only tools, including arbitrary evaluation, console and network
inspection, remain available through that backend. Native snapshots currently
inspect the main document, not cross-origin iframe contents.

The agent activity frame and Take over button sit outside the native rectangle.
Trusted pointer, key, touch or wheel input in the native page requests the existing
agent takeover flow. Agent-generated synthetic events do not trigger takeover.

## Platforms and verification

- macOS: native view and isolated agent bridge (bridge requires macOS 11+).
- Windows: native WebView2 pane and separate Chromium agent tools. Native PNG
  overlay snapshots and per-site data clearing are currently macOS-only; Windows
  can clear browsing data from Settings.
- Linux and headless daemon sessions: Chromium. On macOS, Native agent control
  requires the desktop app; select Headless Chromium for daemon-only use.
- Remote daemon connections retain the remote browser backend.

The UI preview uses a static page image in the native slot. Unit tests exercise
navigation, capability isolation, labels, bounds, LRU eviction, backend dispatch,
CDP screencast sequencing, slot batching/visibility, overlay intersection and the
isolated bridge's DOM behavior and caps. `scripts/webkit-check.mjs` includes a
0.5px toolbar centering check against the preview. Live rendering, media,
WebView2 and Retina snapshot behavior require review in the real app.
