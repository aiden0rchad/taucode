# macOS packaging

Run commands from `apps/taucode`. Install frontend dependencies and populate the
Cargo cache before building; the release script does not run `npm ci`. Packaging
uses the installed Tauri CLI and offline Cargo. Build natively on the target Mac
(arm64 or x86_64); cross and universal builds are not supported by these hooks.

```sh
export PATH="$HOME/.cargo/bin:$PATH"
bash scripts/release-macos.sh
```

`tauri build` stages Node/npm and uv before compiling, builds all three helper
binaries before bundling, and produces a release app. `tauri build --debug` uses
debug helpers. Plain `cargo build --bins` and `cargo test` do not require staged
sidecars: `build.rs` leaves copying external binaries and resources to the CLI
bundler, avoiding overwriting Cargo output with stale helper binaries.

Path remapping applies only to release invocations. `release-macos.sh` launches
Tauri through `release-build.mjs`, which appends `--remap-path-prefix` to existing
Rust flags (preserving encoded flags when supplied) and selects the native C/C++
wrappers through child-process `CC`/`CXX`. The release bundling helper uses the same
environment. `.cargo/config.toml` only enables offline mode; `cargo test`, `tauri dev`
and `tauri build --debug` receive no remapping flags or compiler wrappers.

The native wrappers remap `__FILE__` diagnostics and replace vendored OpenSSL's
build-directory module/engine search macros with system paths under `/usr/lib/openssl`;
the vendored build disables shared modules and ships none. The release helper
invalidates only Whisper's release cache when its compiler configuration changes,
because that dependency does not track compiler environment changes. `strip -S`
removes Swift archive debug symbols before the release path check and signing.
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

For local development with an Apple-issued identity, save its exact name or SHA-1
fingerprint from `security find-identity -v -p codesigning` in
`apps/taucode/.signing-identity.local` (one line). This file is ignored by Git and
read as plain text by `sign-dev.sh`, so subsequent rebuilds retain that identity.
An explicit `TAUCODE_SIGNING_IDENTITY` overrides it. An empty local file fails
signing. This preference applies only to the development signer; release signing
still uses the explicit environment configuration above.

After switching signing identities, an existing provider key may need one macOS
approval. Choose **Always Allow** on taucode's Keychain prompt to save access;
**Allow** authorizes only that read ([Apple's explanation](https://support.apple.com/en-ie/guide/keychain-access/kyca1243/mac)).
Verify a real provider request after quitting and relaunching, then again after
rebuilding with the same signing identity. A successful response alone does not
prove prompt-free access if someone approved a prompt during the check.

macOS may separately ask whether **codesign** can use the **Apple Development**
private key while building. That prompt controls signing-tool access, not taucode's
provider key. Approve it in macOS; choosing Always Allow saves access for future
signing operations.

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

Signing and verification share the policy in `scripts/entitlements.mjs`.
`check-bundle.mjs` reads each Mach-O's signed entitlements with
`codesign -d --entitlements :-` and requires exact keys and values, including
rejecting extra keys set to false. It also checks the app bundle signature.

| Binary | Entitlements file / keys | Purpose |
| --- | --- | --- |
| Main app `codex-monitor` and app bundle | `Entitlements.plist`: `com.apple.security.device.audio-input`, `com.apple.security.automation.apple-events` | Dictation and user-approved app automation. WKWebView content runs in Apple's process, so the app needs no JIT or library-validation exception. |
| Bundled `node` and `bun` | `Entitlements.runtime.plist`: `com.apple.security.cs.allow-jit`, `com.apple.security.cs.allow-unsigned-executable-memory`, `com.apple.security.cs.disable-library-validation` | JIT execution and native `.node` modules installed into app data without the app's signing identity. Bun currently arrives through omp installation; this policy also covers it if bundled. |
| `codex_monitor_daemon`, `codex_monitor_daemonctl`, `taucode-diag-mcp` | None; hardened runtime only | No JIT or unsigned native modules. The daemon's computer-use path uses ScreenCaptureKit, Accessibility and CGEvent/TCC, not Apple Events. Its `osascript` notification fallback is debug-only. |
| `uv`, other native resource executables/libraries | None; hardened runtime only | No process-level JIT or automation privileges needed. Native modules inherit the Node process's capabilities. |

`sign-dev.sh` delegates to the same per-binary signer. Node receives runtime
entitlements during staging; uv receives none. The final release signer applies
the correct profile to every executable and seals the app last.

Screen Recording and Accessibility are TCC grants, not entitlements. The setup
checklist only preflights them. Its buttons open the exact macOS privacy panes;
launching the app never automatically requests or changes these grants.

## Verification

```sh
node --check scripts/bundle-node.mjs
node --check scripts/check-bundle.mjs
node --test scripts/release-manifest.test.mjs scripts/packaging-policy.test.mjs
bash -n scripts/release-macos.sh scripts/sign-macos.sh scripts/sign-dev.sh
node scripts/check-bundle.mjs src-tauri/target/release/bundle/macos/taucode.app
```

Run ShellCheck on the shell scripts when installed. `check-bundle.mjs` validates
resources, runs bundled Node/npm/uv with an empty PATH, checks every Mach-O signature
and rejects checkout paths. It starts a temporary daemon with only
`PATH=/usr/bin:/bin`, calls `get_app_settings` and reports `engine_runtime` status.
Only socket-binding permission failures skip the daemon smoke step; missing
resources, bad signatures, crashes and RPC failures remain fatal.

For application-wide checks, follow the [development guide](development.md).
The offline UI preview provides a separate way to check the interface without
packaging or contacting a provider; it does not validate native permissions,
signatures, or installed runtime behavior. Restricted build environments may need
writable temporary and compiler-cache directories; document any skipped native
check instead of treating it as a successful result.

The first-launch checklist is dismissible, persists Skip/Done locally, refreshes
when the window gains focus, and reopens from Settings → About → Setup checklist.
It links to Providers, engine installation and Features without blocking app use.
