# M5: packaging (a self-contained macOS app, release signing, notarization prep, permission onboarding)

## Goal

A release build of taucode (`tauri build`, not `--debug`) is **self-contained**. On a
Mac with no Node, no Bun, no repo checkout and only the minimal Finder PATH, it can:
- start, run the daemon, and resolve the diagnostics MCP binary
- install and run the pi / omp / Claude ACP / browser / computer-use / work engines

It is signed with a Developer ID identity when one is configured (or with the existing
stable self-signed dev identity), with a documented notarization step. First launch shows
a calm permission onboarding.

Done means:
- a release `.app` passes the self-containment checks below
- `docs/packaging.md` exists
- a `scripts/release-macos.sh` builds, signs, verifies, and (when credentials are
  configured) notarizes and staples

## Current gaps (verified by the reviewer)

1. **Node isn't bundled.**
   - pi/ACP/browser spawn `node` via `TAUCODE_NODE` or PATH
     (`shared/pi/mod.rs:403`, `shared/acp/mod.rs:322`, `engine_runtime.rs:108,187`,
     `codex_aux_core.rs:322`).
   - An app launched from Finder has a minimal PATH, so Node usually isn't found.
   - The developer's Node is at `$(realpath $(which node))` (v26.10.0).
2. **The helper binaries aren't bundled.** The `.app/Contents/MacOS` dir contains only
   `codex-monitor` and `codex_monitor_daemonctl`. `codex_monitor_daemon` and
   `taucode-diag-mcp` are missing.
   - `daemon_binary.rs` resolves them next to the exe, plus dev fallbacks
     (`CARGO_MANIFEST_DIR`, target dirs) under `debug_assertions`.
3. **Dev-path fallbacks:** `engine_runtime.rs:51` and `:236` use `CARGO_MANIFEST_DIR`
   for engines (gated by `debug_assertions` at :51; check :236 too). Release builds must
   never depend on them.
4. **Bun for omp** comes from the npm `bun` package installed by `npm ci` into the omp
   runtime dir. That's fine as long as the runtime install works with the bundled Node
   and npm.
5. **Signing** is only `scripts/sign-dev.sh` (a self-signed "taucode Dev" identity,
   `--deep`). There's no hardened runtime, no entitlements review, no notarization.

## Required

### A. Bundle the runtimes and helpers

- **Node:**
  - At build time, a script (`scripts/bundle-node.mjs`, invoked from `beforeBuildCommand`
    or a `tauri.conf.json` hook) copies the real Node binary (resolve symlinks: the
    machine's `node` is `~/.local/bin/node` → `~/.hermes/node/bin/node`) plus npm (the
    `lib/node_modules/npm` dir and the npm/npx shims) into
    `src-tauri/target/engine-resources/node/` (git-ignored), bundled as the
    `node/` resource.
  - Record the version in `node/VERSION`.
  - Verify that its architecture matches the build target (`file`/`lipo -info`) and fail
    otherwise.
  - Include Node's LICENSE.
- **Resolution order everywhere Node is spawned:** `TAUCODE_NODE` env → **the bundled
  `Resources/node/bin/node`** → PATH. Put this in one helper (e.g.
  `shared/engine_runtime::node_path()`) used by every call site listed above; no
  duplicates. The npm used for `npm ci` engine installs must be the bundled npm, run with
  the bundled node, with `PATH` set so lifecycle scripts find that node.
- **Helper binaries:** bundle `codex_monitor_daemon`, `codex_monitor_daemonctl` and
  `taucode-diag-mcp` into `Contents/MacOS`, using Tauri's `bundle.externalBin` (which
  requires target-triple-suffixed copies under `src-tauri/binaries/`; generate them
  in the build script from the built targets, git-ignored) or an equivalent post-build
  step.
  - `taucode-diag-mcp` lives in `crates/taucode-diag-mcp` (its own crate) **and**
    `src-tauri/src/bin/taucode-diag-mcp.rs`. Bundle the one `daemon_binary.rs` resolves,
    and make sure it's built in release.
- **Release builds have no dev fallbacks:**
  - everything gated on `CARGO_MANIFEST_DIR` must be `#[cfg(debug_assertions)]` or
    test-only
  - add a test or CI check that greps the release binary for the repo path string
    (`strings target/release/codex-monitor | grep -c "$REPO_ROOT"` = 0) and fails
    otherwise
- The Settings → Diagnostics snippets resolve the **bundled** `taucode-diag-mcp` path
  inside the `.app` when running from a bundle.

### B. Signing and notarization

- `src-tauri/Entitlements.plist`: review it for the hardened runtime. Keep only what's
  needed:
  - `com.apple.security.cs.allow-jit` and `allow-unsigned-executable-memory` (for
    WKWebView/Node JIT, if needed)
  - `com.apple.security.cs.disable-library-validation` only if bundled Node or native
    modules require it (document why)
  - `com.apple.security.automation.apple-events` for computer use
  - Screen Recording / Accessibility don't use entitlements (TCC prompts).
  - Document each entitlement.
- `scripts/release-macos.sh`:
  1. `npm ci`-free build: assume deps are installed. Run `tauri build` (release,
     app + dmg).
  2. Sign **inside-out**: every Mach-O in Resources (node, native `.node` modules,
     bundled python if any), then the helper binaries, then the app. Use the hardened
     runtime (`--options runtime`), `--timestamp` and entitlements. The identity comes
     from `TAUCODE_SIGNING_IDENTITY` (a Developer ID Application identity); fall back to
     the self-signed "taucode Dev" identity for local builds, without timestamp/notarize.
  3. Run `codesign --verify --deep --strict` and `spctl -a -vv` (expected to fail without
     a Developer ID, which is reported, not fatal, for local builds).
  4. Notarize only when `TAUCODE_NOTARY_PROFILE` is set, using
     `xcrun notarytool submit <dmg> --keychain-profile "$TAUCODE_NOTARY_PROFILE" --wait`,
     then `xcrun stapler staple`.
     - **Never** read, prompt for or store Apple ID passwords. The user creates the
       keychain profile themselves with `xcrun notarytool store-credentials` (document
       the exact command in `docs/packaging.md`).
  5. Write a `release-manifest.json`: the version, the bundled Node version, the engine
     pins (pi, omp, claude-acp, browser), the codegraph version if bundled, the sha256
     of the dmg, and the signing identity type.
- Keep `scripts/sign-dev.sh` working for debug builds, updated to sign the newly bundled
  helpers and node inside-out instead of `--deep` only.

### C. Permission onboarding (first launch, and from Settings)

- A first-run sheet (ModalShell, the settings design language), shown once and
  re-openable from Settings → About ("Setup checklist"). Its checklist rows show live
  status where detectable:
  - **Providers:** at least one provider with a key or `auth: none`. "Open Providers"
    button.
  - **Engines:** pi runtime ready (Install button if not); others optional.
  - **Screen Recording and Accessibility:** needed only for computer use. Show the
    current TCC status if detectable (CGPreflightScreenCaptureAccess /
    AXIsProcessTrusted) and an "Open System Settings" deep link
    (`x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture` /
    `?Privacy_Accessibility`). **Never request or change TCC state automatically**
    beyond the system's own prompt when the user clicks the button.
  - **Optional:** codegraph installed? (detect the binary) Diagnostics on/off (link to
    Features).
- "Skip for now" and "Done". Nothing blocks app use.
- Status re-checks when the window regains focus.

### D. Self-containment check script

`scripts/check-bundle.mjs <path to .app>`:
- **Contents:** asserts the presence of the bundled node (it runs `node --version` from
  the bundle with an empty PATH), npm, the three helpers, the engine resource dirs
  (package.json and lockfiles), and the omp `no-accounts.mjs`.
- **No repo paths:** asserts the release binary contains no repo path strings.
- **Signing:** asserts every Mach-O is signed (`codesign -v` on each).
- **Daemon smoke:** launches the bundled `codex_monitor_daemon` with `--data-dir <tmp>
  --listen 127.0.0.1:<free port> --insecure-no-auth` and `env -i PATH=/usr/bin:/bin`,
  calls `get_app_settings` over the socket, and reports engine runtime status. The
  reviewer runs this; your sandbox may not allow binding sockets, so make the script
  skip the daemon step with a clear message when binding fails.

## Files

`apps/taucode/scripts/**` (new scripts), `src-tauri/tauri.conf.json` (bundle section:
resources, externalBin, macOS), `src-tauri/Entitlements.plist`, `src-tauri/Info.plist`
(only if needed), `src-tauri/build.rs` (if needed), `src-tauri/src/daemon_binary.rs`,
`src-tauri/src/shared/engine_runtime.rs` and the node call sites listed above, the
onboarding UI under `src/features/onboarding/**` plus a Settings → About entry,
`docs/packaging.md`, `.gitignore`.

**Off-limits:** evals (a parallel M6 worktree adds `src-tauri/src/shared/evals/**`,
`apps/taucode/evals/**` and an Evals settings page) and themes.

## Tests

- **`node_path()` resolution order:** env → bundled → PATH. Use temp dirs; no reliance
  on the real machine.
- **Engine install** uses the bundled npm/node when present (mock the bundle dir).
- **`daemon_binary`** resolves helpers from `Contents/MacOS` in bundle layout, and
  release builds don't consult dev paths (unit-test the dir-list builder with
  `debug_assertions` both ways where feasible).
- **Onboarding:** first-run shows once; statuses render; deep-link buttons call the
  opener with the exact URLs; Skip/Done persist.
- **Scripts:** `node --check` for `bundle-node.mjs` and `check-bundle.mjs`; `bash -n`
  and `shellcheck`-clean style (if shellcheck is available) for `release-macos.sh`; a
  unit test of the manifest builder.

## Constraints and verification

- Offline. No new crates or npm deps. `CARGO_HOME=/Users/rolandleyco/taucode-m5p/.cargo-home`
  with `$HOME/.cargo/bin` on PATH.
- Verification: the command in `docs/specs/m3-computer-use.md`, run from
  `/Users/rolandleyco/taucode-m5p` with this worktree's `CARGO_HOME` (the permitted
  tailscale exclusion), plus `npm run build -- --mode uipreview`.
- You may run `npx tauri build --bundles app` (release) once at the end to check the
  bundle layout if your sandbox allows it. If it fails because of the sandbox, report it;
  the reviewer will run it.
- **Don't** run notarization, don't touch the keychain, and don't create certificates.
  No browser or computer use. Don't commit.
