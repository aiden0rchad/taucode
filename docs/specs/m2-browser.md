# M2: built-in browser (in-app pane + agent browser tools)

## Goal

taucode gets a browser that the user and the agent **share**:

- the user sees and drives it in an in-app **Browser pane** (URL bar, tabs, back/forward/
  reload, live page, mouse and keyboard)
- every engine (pi and codex) drives the **same** browser through browser tools, delivered as
  the built-in MCP connector `taucode-browser` (the reserved id from M1-E)

No accounts, no extension, no cloud browser. Sites the user signs into inside this browser
stay signed in (a persistent taucode-owned profile), which is the user's own web session,
not a taucode login.

Done = the verification command passes. A live test is run outside your sandbox.

## Architecture (decided; don't redesign)

1. **Browser process**, owned by the Rust backend (`src-tauri/src/browser/`), one per app
   instance:
   - binary discovery order: setting → Google Chrome → Microsoft Edge → Chromium (macOS app
     paths, plus Linux/Windows equivalents)
   - a clear error when none is found
   - launch with `--remote-debugging-port=0` (read the actual port from `DevToolsActivePort`
     in the profile dir), `--user-data-dir=<app data>/browser/profile`, `--headless=new`,
     `--no-first-run`, `--no-default-browser-check`, `--disable-features=Translate`, and a
     window size matching the pane
   - lazy start (first pane open or first agent browser tool), restart on crash, stop on app
     quit
2. **CDP bridge** (Rust; `tokio-tungstenite` is already a dependency): connects to the
   browser's CDP websocket, tracks page targets (tabs), and for the **active** tab:
   - `Page.startScreencast` (jpeg, quality ~70, everyNthFrame 1, max size = pane size × device
     pixel ratio) → frames to the frontend over a Tauri channel; ack every frame
     (`Page.screencastFrameAck`)
   - input from the pane → `Input.dispatchMouseEvent` (move/down/up/wheel, buttons,
     clickCount, modifiers), `Input.dispatchKeyEvent` (keyDown/keyUp with key, code, text,
     modifiers; Cmd shortcuts mapped on macOS), `Input.insertText` for IME/paste
   - pane resize → `Emulation.setDeviceMetricsOverride`
   - navigation, history, reload, new/close/activate tab via CDP (`Page.navigate`,
     `Page.getNavigationHistory`, `Target.createTarget/closeTarget/activateTarget`)
   - page title, URL, favicon and loading state events to the frontend
   - put the transport behind a trait, so tests can use an in-memory fake CDP peer (no
     sockets)
3. **Agent tools: Playwright MCP, unchanged.** Pin `@playwright/mcp@0.0.83` in
   `apps/taucode/engines/browser/package.json` (pre-installed offline).
   - The `taucode-browser` connector runs
     `node <engines/browser>/node_modules/@playwright/mcp/cli.js --cdp-endpoint http://127.0.0.1:<port>`.
     Check the bin path in its `package.json`. The host fills in the port when generating
     engine configs; when the browser isn't running yet, the connector's command is a tiny
     taucode launcher that starts it first. Implement that as a `--tau-browser-mcp`
     subcommand of the taucode executable, which ensures the browser is up, then execs or
     proxies playwright-mcp, the same pattern as `--tau-mcp-proxy`.
   - Since the app process owns the browser, the launcher talks to the running app (or
     daemon) over a local unix socket in the app data dir. If no app is running (daemon
     mode), it launches its own browser with the same profile.
   - Agent and user share tabs. When the agent acts on a tab, the pane switches to that tab.
   - Configurable in Settings → Browser:
     - browser binary
     - allowed/blocked origins, passed to playwright-mcp `--allowed-origins` /
       `--blocked-origins`
     - an approval mode for mutating browser tools: click, type, fill_form, select_option,
       drag, drop, file_upload, evaluate, run_code_unsafe, press_key, handle_dialog and the
       cookie/storage setters. "ask" routes them through the existing approval UI (pi:
       taucode-gate `mcp__taucode-browser__*` rules; codex: its MCP tool approval);
       read-only tools (snapshot, screenshot, console, network, tabs, navigate) never ask.
4. **Browser pane** (React, follows the existing panel/layout patterns, like the terminal
   panel):
   - toolbar: back, forward, reload/stop, URL field (Enter navigates; scheme-less input →
     https; plain words → search via a configurable search URL), tabs strip
   - a canvas rendering frames, with correct device-pixel-ratio scaling, focus handling
     (keyboard goes to the page while the canvas is focused), cursor feedback, and wheel
     scrolling
   - an "Agent is using the browser" indicator while an agent browser tool runs, plus a
     **Take over** button that interrupts the turn
   - keyboard shortcut to toggle the pane, matching the app's shortcut conventions;
     persisted layout
   - thread items for `taucode-browser` tool calls render as the existing `mcpToolCall`
     items, with the screenshot result shown as an image when present
5. **Settings → Browser:** binary, origins allow/block lists, approval mode, search URL,
   "Clear browsing data" (stop the browser, delete the profile dir after confirming).

## Tests

- **CDP bridge** against an in-memory fake CDP peer: target tracking, screencast
  start/ack/stop on tab switch, input event mapping (mouse buttons, modifiers, keys including
  Cmd/Ctrl mapping, insertText), resize → metrics override, navigation commands,
  `DevToolsActivePort` parsing, crash → restart.
- **Launcher:** argument building, binary discovery order, profile path, the
  `--tau-browser-mcp` dispatch (no Tauri init), connector config generation for both engines.
- **Approval rules** for mutating vs read-only browser tools, in both engines' generated
  configs.
- **vitest:** Browser pane toolbar/URL normalization, frame rendering with DPR, input
  forwarding calls, agent indicator, Take over, Settings → Browser.

## Constraints

Offline sandbox; no new Rust dependencies and no new npm dependencies besides the
pre-installed pinned `@playwright/mcp` under `engines/browser`. Always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`. No real browser launches or sockets
in tests. Follow `apps/taucode/AGENTS.md`, and the connector registry (M1-E) for delivery.
Don't touch protocol/fixtures/crates/docs/agent configs.

## Verification

Same command as M1-E.

## Out of scope

Chrome extensions, a browser bundled with the app, multiple profiles, downloads UI
(downloads go to ~/Downloads via CDP `Browser.setDownloadBehavior`; note it in the report),
computer use (M3).
