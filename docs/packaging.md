# macOS packaging

Run commands from `apps/taucode`. Install frontend dependencies and populate the
Cargo cache before building; the release script does not run `npm ci`. Packaging
uses the installed Tauri CLI and offline Cargo. Build natively on the target Mac
(arm64 or x86_64); cross and universal builds are not supported by these hooks.

```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME=/Users/rolandleyco/taucode-m5p/.cargo-home
bash scripts/release-macos.sh
```

`tauri build` stages Node/npm and uv before compiling, builds all three helper
binaries before bundling, and produces a release app. `tauri build --debug` uses
debug helpers. Plain `cargo build --bins` and `cargo test` do not require staged
sidecars: `build.rs` leaves copying external binaries and resources to the CLI
bundler, avoiding overwriting Cargo output with stale helper binaries.

The Rust and native C/C++ compiler wrappers remap this checkout to `/taucode` in
compiler paths and `__FILE__` diagnostics. The native wrapper also replaces vendored
OpenSSL's generated build-directory module/engine search macros with system paths
under `/usr/lib/openssl`; the vendored build disables shared modules and ships none.
This prevents unused OpenSSL metadata and Whisper assertions from leaking build paths.
The staging hook invalidates Whisper’s cached CMake build once per wrapper revision
because that dependency does not track compiler environment changes. Before bundling,
`strip -S` removes Swift archive debug symbols, then the hook checks every app/helper
binary for the checkout path. Signing happens after stripping.
The bundle check rejects any remaining literal checkout path in the app and helper
binaries. Development engine/diagnostics fallbacks exist only in debug builds or
tests; production resolves engine sources from bundle resources and installs them
in the application data directory. Settings → Diagnostics resolves
`Contents/MacOS/taucode-diag-mcp` from the running app.

## Bundled runtimes

`scripts/bundle-node.mjs` resolves the real Node executable, verifies its Mach-O
architecture and system-only dynamic library dependencies, and copies it with npm,
npm/npx shims, `VERSION`, and Node's `LICENSE` to the ignored
`src-tauri/target/engine-resources/node/` tree. npm includes its own notices.
Use `TAUCODE_BUNDLE_NODE=/path/to/node/bin/node` to select a standalone distribution
with `LICENSE` and `lib/node_modules/npm` in its prefix. Node must be at least 22.19.

The script also stages standalone uv, its version, and MIT/Apache licenses. Both
executables receive writable modes and ad-hoc hardened-runtime signatures before
Tauri copies them; the release signer replaces these with the selected identity. Set
`TAUCODE_BUNDLE_UV` and, if necessary, `TAUCODE_UV_LICENSE_DIR` to choose its binary
and license directory. No Node, Bun, npm, Python, uv, or checkout is needed on the
destination Mac. First-time engine installation needs internet access: npm installs
the pinned packages, omp's npm dependency supplies Bun, and uv downloads managed
Python 3.12 and the locked computer/work dependencies. When no installed browser is found, Playwright installs managed Chromium under
application data; an explicitly configured browser path is honored. Codegraph and Codex remain optional separately installed tools.

Node resolution is `TAUCODE_NODE`, then bundled `Resources/node/bin/node`, then
PATH. Bundled npm always runs through its matching bundled Node, with that Node's
directory first on PATH for install/lifecycle scripts. Engine child processes also
receive Node on PATH. Overrides are intended for troubleshooting, not required setup.

The generated `src-tauri/binaries/<name>-<target-triple>` files become these sidecars
in `Contents/MacOS`: `codex_monitor_daemon`, `codex_monitor_daemonctl`, and
`taucode-diag-mcp`. The diagnostics binary is built from the app package's
`src/bin/taucode-diag-mcp.rs`, using the shared diagnostics crate.

## Signing and notarization

Use an existing identity; these scripts never create certificates or modify the
keychain. For distribution:

```sh
export TAUCODE_SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)'
export TAUCODE_NOTARY_PROFILE=taucode-notary
bash scripts/release-macos.sh
```

Without a configured identity, signing uses the existing `taucode Dev` self-signed
identity, without timestamps or notarization. `scripts/sign-dev.sh [path/to/app]`
uses the same inside-out signer for debug bundles. Missing identities fail signing.

The release script builds app + DMG, signs every Mach-O under Resources and MacOS,
then the app, with hardened runtime. Developer ID signatures use secure timestamps.
It verifies the sealed app (`codesign --verify --deep --strict`), runs the bundle
check and Gatekeeper assessment, then recreates the DMG from the signed app and
signs the DMG. Local Gatekeeper rejection is reported as expected. Developer ID
builds rejected by Gatekeeper fail without a notarization profile.
With a notary profile, submission, stapling, validation and the final Gatekeeper
assessment must all succeed. `release-manifest.json` beside the DMG records app and
Node versions, exact engine dependency pins, optional bundled Codegraph version,
final DMG SHA-256 and identity type. Codegraph is not bundled by default.

Create the notarization keychain profile yourself, outside the release script:

```sh
xcrun notarytool store-credentials taucode-notary --apple-id 'you@example.com' --team-id 'TEAMID'
```

Apple's tool securely prompts you for an app-specific password. Do not put it in
source files, shell arguments, environment variables, or agent messages. The release
script only uses the profile name:

```sh
xcrun notarytool submit path/to/taucode.dmg --keychain-profile taucode-notary --wait
xcrun stapler staple path/to/taucode.dmg
```

### Entitlements

`src-tauri/Entitlements.plist` is used for the app and nested executable signatures:

| Entitlement | Purpose |
| --- | --- |
| `com.apple.security.cs.allow-jit` | Node/V8 JIT under hardened runtime. WKWebView runs web content in Apple's own process. |
| `com.apple.security.cs.allow-unsigned-executable-memory` | Compatibility with Node/V8 and Bun/native engine executable-memory allocation. |
| `com.apple.security.cs.disable-library-validation` | npm engines load native `.node` modules downloaded into application data; these are not signed with the app's Developer ID. |
| `com.apple.security.automation.apple-events` | User-approved computer-use automation of other apps. |
| `com.apple.security.device.audio-input` | Existing dictation microphone capture under hardened runtime. |

Screen Recording and Accessibility are TCC grants, not entitlements. The setup
checklist only preflights them. Its buttons open the exact macOS privacy panes;
launching the app never automatically requests or changes these grants.

## Verification

```sh
node --check scripts/bundle-node.mjs
node --check scripts/check-bundle.mjs
node --test scripts/release-manifest.test.mjs
bash -n scripts/release-macos.sh scripts/sign-macos.sh scripts/sign-dev.sh
node scripts/check-bundle.mjs src-tauri/target/release/bundle/macos/taucode.app
```

Run ShellCheck on the shell scripts when installed. `check-bundle.mjs` validates
resources, runs bundled Node/npm/uv with an empty PATH, checks every Mach-O signature
and rejects checkout paths. It starts a temporary daemon with only
`PATH=/usr/bin:/bin`, calls `get_app_settings` and reports `engine_runtime` status.
Only socket-binding permission failures skip the daemon smoke step; missing
resources, bad signatures, crashes and RPC failures remain fatal.

For the full repository verification, use the M3 command with this worktree's
`CARGO_HOME`, then `npm run build -- --mode uipreview`. The permitted sandbox
exclusion is `cargo test -- --skip tailscale::tests::listen_addr_preflight_fails_when_port_is_in_use`.
Keep `TMPDIR`, `CLANG_MODULE_CACHE_PATH` and `SWIFT_MODULECACHE_PATH` under this
workspace in restricted environments. SwiftPM may additionally need its own
package sandbox disabled when the outer execution sandbox forbids nested sandboxes.

The first-launch checklist is dismissible, persists Skip/Done locally, refreshes
when the window gains focus, and reopens from Settings → About → Setup checklist.
It links to Providers, engine installation and Features without blocking app use.
