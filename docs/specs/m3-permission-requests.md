# M3 fix: actually request macOS permissions

## Problem

`src-tauri/src/computer/native.rs` only calls `CGPreflightScreenCaptureAccess()` and
`AXIsProcessTrusted()`. Those checks are silent: they never show the system prompt and never
add the app to System Settings → Privacy & Security → Screen Recording / Accessibility. A
user found taucode missing from both lists and couldn't grant anything.

## Required

1. A Tauri command (and daemon RPC parity, where computer use exists in the daemon)
   `computer_request_permissions`:
   - calls `CGRequestScreenCaptureAccess()`, which shows the prompt and registers the app
   - calls `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt = true`, built
     with core-foundation via the existing objc2 / core-graphics crates
   - returns the same `{screen, accessibility}` status shape as the check
2. Settings → Computer Use:
   - add a primary **"Request access"** button next to the status. It calls the command,
     then opens the matching System Settings pane for whatever is still missing.
   - add a note that Screen Recording takes effect only after quitting and reopening
     taucode, with a **"Quit & Reopen"** button (`tauri::process::restart` or the app's
     existing relaunch helper)
3. When the user turns Computer Use **on** and permissions are missing, call the request
   once automatically.
4. Packaging note in the computer README: development builds are ad-hoc signed, so macOS may
   ask again after rebuilds.
5. Tests: the command's status mapping (the FFI goes behind a trait with a fake); UI tests for
   the button and the restart note.

## Constraints

Offline; no new dependencies (use the already-present objc2 / core-graphics / core-foundation;
if core-foundation isn't a direct dependency, use raw `extern "C"` declarations against
ApplicationServices / CoreFoundation, as `native.rs` already does). Always set
`CARGO_HOME=/Users/rolandleyco/taucode-m1w/.cargo-home`.

## Verification

Same command as `m3-computer-use.md`.
