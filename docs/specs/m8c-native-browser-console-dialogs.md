# M8c — Native browser: console messages, JS dialogs, and only advertising working tools

## Goal

In live dogfooding with GPT 6.1 Sol, agents using the native (WKWebView) browser hit three
gaps:

1. `browser_console_messages` fails with "This tool needs trusted input or CDP…". Agents
   can't check console errors, which is the most common verification step.
2. `browser_handle_dialog` fails the same way. A page `confirm()` silently returns `false` in
   WKWebView, so flows behind confirm/alert/prompt can't be tested. One agent spent several
   minutes and about ten tool calls working around a single "Clear all?" confirm.
3. The connector advertises tools the native backend can't serve (for example
   `browser_run_code_unsafe`). Each such call wastes a model turn.

Done means:
- `browser_console_messages` works on the native backend:
  - It returns entries captured since the page loaded: `log`/`info`/`warn`/`error`/`debug`
    console calls, uncaught errors (`error` event, with filename:line), and unhandled promise
    rejections.
  - Each entry has `{type, text, timestamp}`, with text truncated to 2,000 chars.
  - The buffer is capped (keep the latest 500 entries).
  - It honors the Playwright MCP parameters the agent sends: `level`
    (`error|warning|info|debug`, with Playwright semantics: `warning` includes errors), and
    `all` or `onlyErrors` if present.
- `browser_handle_dialog` works on the native backend with pre-armed semantics:
  - `alert`, `confirm` and `prompt` are wrapped in the page world at document start. Each call
    records `{type, message, defaultValue, answered, timestamp}` in a capped list (latest 50).
  - When an answer is armed, the dialog consumes it:
    - `confirm`: `accept` → true, dismiss → false
    - `prompt`: `accept` → `promptText` (or `defaultValue`), dismiss → null
    - `alert`: returns undefined
  - Unarmed dialogs are dismissed (confirm → false, prompt → null), Playwright's default, but
    are recorded.
  - `browser_handle_dialog({accept, promptText})` arms the answer for the next dialog only,
    then returns a short text result. The result says what the most recent dialog was and
    whether it was dismissed, then tells the agent "the next dialog will be
    accepted/dismissed; repeat the action that opens it". This lets an agent click, see that
    nothing happened, arm, and click again.
  - The browser snapshot's text includes a one-line note when the latest dialog was
    auto-dismissed since the last snapshot, so the agent notices.
- With the native backend, `tools/list` omits tools that `backend::tool` (in
  `src-tauri/src/browser/backend.rs`) can't serve; the CDP backend still lists everything.
  Agents then never see `browser_run_code_unsafe`, `browser_drag`, `browser_file_upload`
  and the like. Keep the existing `target` schema rewrite.

## Files

Modify:
- `apps/taucode/src-tauri/src/browser/backend.rs`: map `browser_console_messages` and
  `browser_handle_dialog` to new backend actions (`{"action":"console",…}`,
  `{"action":"dialog",…}`), plus a function returning the set of supported tool names.
- `apps/taucode/src-tauri/src/browser/native/mod.rs`:
  - Add an `.initialization_script(...)` on the `WebviewBuilder` (main frame only) that
    installs the capture/dialog shim.
  - Handle the `console` and `dialog` actions: run page-world JS via the existing
    `macos::evaluate(…, page=true)` path, guarded by the same `automation_allowed` origin
    check as `evaluate`.
- New `apps/taucode/src-tauri/src/browser/native/page_hooks.js`: the document-start shim,
  `include_str!`'d.
  - Use a per-webview random global key, a UUID generated when the webview is built and
    stored with the tab's view state, so the key isn't predictable.
  - Store state on a non-enumerable property under that key.
  - Wrap the console methods so the originals still run.
  - Never throw into page code.
- Wherever `tools/list` from the Playwright MCP child is relayed to the engine (find it:
  `browser/mod.rs`, `pipe.rs` or `launcher.rs`): filter by the supported set when the backend
  is native.
- Tests: `backend.rs` unit tests for the new mappings and the supported-set filter; a
  `src/features/browser/nativeBridge.test.ts`-style JS test that loads `page_hooks.js` in
  jsdom and checks console capture, caps, the error event, unhandled rejections, and armed
  vs. unarmed confirm/prompt/alert; and a Rust test of the tools/list filter if a seam exists.

Off-limits: `engines/pi/**`, `src-tauri/src/shared/**`, every frontend file except the new
test, and `bridge.js`'s existing ops, apart from adding the snapshot dialog note if it lives
there.

## Constraints

- Follow existing patterns in `native/mod.rs` and `macos.rs`: compact Rust style,
  `Result<Value, String>`, `serde_json::json!`.
- Page-world data is untrusted. Bound every string and array, and JSON-encode results through
  the same 200KB result cap `evaluate` uses.
- The private automation bridge world must stay isolated. Don't expose it to the page.
- No new dependencies.

## Verification

Run from `/Users/rolandleyco/taucode-m1w/apps/taucode`:

```
npx tsc --noEmit && npx vitest run src/features/browser && (cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home cargo test --lib browser && PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home cargo test --lib)
```

All of it must pass.

## Out of scope

- CDP backend behavior (unchanged).
- Network request capture.
- UI changes.
- Committing. Don't commit; the reviewer commits.
