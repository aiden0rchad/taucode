# Handoff: debug "can't resize the window" in the running taucode dev build

## What this is
**taucode** is a local Tauri v2 desktop agent harness, a fork of CodexMonitor. It has:
- Code, Chat and Work modes
- engines: pi (the default), oh-my-pi (omp), codex, Claude ACP and custom
- themes, diagnostics, evals and a native browser

**Repos and branches:**
- Worktree: `/Users/rolandleyco/taucode-m1w`, branch `m1-wiring`. All current work lives here.
- `main` is at `/Users/rolandleyco/taucode`. Merges happen only when the user approves.
- The latest commit on m1-wiring is `59ef536` (the panel-resize fix). Nothing after `bd2831a` is merged to main yet.

**The running app:** `/Users/rolandleyco/taucode-m1w/apps/taucode/src-tauri/target/debug/bundle/macos/taucode.app` (a debug build, signed with the self-signed identity "taucode Dev"; the process name is `codex-monitor`).
- It was launched from a shell with stdout/stderr going to `/private/tmp/claude-501/-Users-rolandleyco-taucode/e53185da-2175-4cf4-9846-79306911b175/scratchpad/app-run.log`.
- The log has only info lines like `window_material_applied`; no errors were seen.

**User data:** `~/Library/Application Support/dev.taucode.desktop` is a fresh-install test.
- The original data is backed up in `~/taucode-backup-20261025-203033/`, which contains `restore.sh` (quit the app first).
- WebKit localStorage lives under `~/Library/WebKit/dev.taucode.desktop/WebsiteData/Default/**/LocalStorage/localstorage.sqlite3` (UTF-16 values). It currently has `codexmonitor.rightPanelWidth=900`, `sidebarWidth=280`, `sidebarCollapsed=false` and `rightPanelCollapsed=false`.

## The bug to debug now
The user says **"I can't resize the window"** in the running app. It's unclear whether that means the macOS window edges/corner, or the side-panel divider (both were reported recently). Ask, or test both.

Context from earlier attempts:
- **A side-panel divider bug was just fixed** in commit `59ef536`: a dynamic max, a drag starting from the rendered width, pointer capture, and the native browser view hidden while dragging. It's verified in Playwright WebKit at 995pt (overlay) and 1600pt (docked), but not yet verified in the real app.
- **Without Screen Recording or Accessibility access**, macOS didn't return the main window to a CoreGraphics `CGWindowListCopyWindowInfo` query. Only two 30pt-tall strips were listed. Check whether the window is in full screen, hidden or offscreen.

### Prime suspects (all from the M7 native browser, merged in `b849952` with follow-up fix `42448ae`)
1. **Multiwebview window.** The browser pane adds child WKWebViews (`browser-<tabId>`) to the `main` window via `Window::add_child` (the tauri `unstable` feature).
   - `src-tauri/src/browser/native/mod.rs` (around lines 185–245): `add_child` at offscreen (-32000), then `main.set_auto_resize(true)` and `main.set_size(window.inner_size())`; `place()` calls `set_bounds`/`show`/`hide`.
   - `src-tauri/src/window.rs`:
     - `resize_main()`, called from `lib.rs` `on_window_event` on every `WindowEvent::Resized`/`ScaleFactorChanged`, sets the main webview's position to (0,0) and its size to `inner_size()`
     - a macOS `NSEvent` local monitor (`install_main_focus`) that calls `makeFirstResponder` on left mouse-down
   - Check for a resize feedback loop, the main webview not tracking the window, or anything that constrains the NSWindow.
2. **Liquid Glass port.** `window_material` (ported from tauri-plugin-liquid-glass into our code in `42448ae`) attaches an `NSGlassEffectView` to the window content view. Check that it doesn't interfere with resizing or hit-testing at the edges.
3. **window-state plugin plus our `save_main_state`** (`window.rs`): a restored bad size or full-screen state.
4. **Frontend:** the M5n/resize work added a full-window transparent drag overlay while resizing panels (`cursor: col-resize`). If it gets stuck (pointerup missed), it covers the UI (not the native window edges). The overlay registry hides native views while overlays are open.
5. `tauri.conf.json`: `transparent: true`, `macOSPrivateApi: true`, `titleBarStyle: Overlay`, `hiddenTitle`, min 360×600.

## How to debug
- Use **computer use** on the running taucode app: screenshot it, hover the window edges and corner, try a drag, open the browser panel, and try the divider.
- Relaunch with logging:
  ```
  osascript -e 'quit app "taucode"'; pkill -x codex-monitor
  nohup <app>/Contents/MacOS/codex-monitor > <scratchpad>/app-run.log 2>&1 &
  ```
- Rebuild:
  ```
  cd apps/taucode && PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home npx tauri build --debug --bundles app && ./scripts/sign-dev.sh
  ```
- UI preview (mock IPC, no native webviews): `npm run preview:ui` on port 1421 (`.claude/launch.json` config `taucode-ui`), plus `scripts/webkit-check.mjs http://localhost:1421 <outdir>` (Playwright WebKit; it should pass 72+ checks).
- Full verification: the command in `docs/specs/m3-computer-use.md`, with `TMPDIR=/Users/rolandleyco/taucode-m1w/.tmp`. Some eval/import tests fail under the default `/var/folders` TMPDIR because of a symlink path check; that's a known open follow-up.

## Working agreement (the user's standing rules)
- Implementation goes to **Codex with gpt-6-astra, high reasoning**, using a spec file:
  ```
  codex exec -m gpt-6-astra -c model_reasoning_effort='"high"' -c 'mcp_servers.codegraph.command="codegraph"' -c 'mcp_servers.codegraph.args=["serve","--mcp"]' -o <result.md> "Read <spec> and implement it. Do not commit. Run the verification command until it passes. Report what you changed."
  ```
  Use `codex exec resume <session-id>` for corrections. Small fixes under about 20 lines can be done directly.
- Never delegate against a dirty tree: commit a checkpoint first. Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **No account logins anywhere.** Keys live in the Keychain or the child env only. Never `sandbox: danger-full-access`. Never `--skip-git-repo-check`.
- Verify UI in real WebKit (Playwright WebKit or the real app), not just the Chromium preview.
- Keep going without asking for confirmation unless blocked. Ask before merging to main.

## Other open items
- The wizard's compact theme previews crop off the Send button.
- `/var/folders` symlink path checks in the eval/import sandbox code.
- Notarization needs the user's Apple Developer ID.
