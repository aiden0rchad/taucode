# Builds & platform status

taucode is in **early development**. The source is available for inspection and building; published test artifacts are experimental. A package version or a passing build is not a stable-release or feature-parity guarantee.

The source version is **0.7.70**, prepared on **November 1, 2026**. It includes the interface, resizing, project-role editing and in-app installer-download changes in the [changelog](https://github.com/aiden0rchad/taucode/blob/main/CHANGELOG.md). The published download baseline below remains **0.7.69** until the new release assets are published. See [GitHub Releases](https://github.com/aiden0rchad/taucode/releases) for the assets actually published and the commit behind each build.

## Installable artifacts

The current [v0.7.69 release](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.69) provides both desktop downloads. For step-by-step installation and help choosing a file, use [Installation & first run](getting-started.md#_1-install-the-app).

### Windows x64 test build

The [v0.7.69 release](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.69) contains:

- `taucode_0.7.69_x64-setup.exe`
- `SHA256SUMS`

It is an **unsigned early-development release** for 64-bit Intel/AMD Windows. The release records its source commit. The release notes record the compilation, installation, bundled-runtime and startup checks for this version. Full provider, browser, tool, credential, and task verification is still needed.

### macOS

The same [test release](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.69) includes **taucode_0.7.69_aarch64.dmg** for **Apple Silicon Macs**, `macos-arm64-release-manifest.json`, and shared `SHA256SUMS`.

The package passed the release's recorded bundle, signature, entitlement, bundled-runtime, daemon RPC, and mounted-DMG checks. It uses a **local development signing identity** and is **not notarized**. These checks do not make it a stable release or guarantee opening on every Mac. Follow the [installation steps](getting-started.md#macos-apple-silicon).

There is no Intel Mac installer in this release. Developers can use the [source-build](getting-started.md#build-from-source) and [packaging](packaging.md) instructions.

## Platform matrix

| Area | macOS | Windows x64 | Linux | iOS |
| --- | --- | --- | --- | --- |
| Desktop source/build path | Broadest current verification | CI build/install/startup evidence | Source path; further validation needed | Experimental mobile/remote path |
| Code, Chat, Work | Native checks on the development Mac within documented scope | Full workflow validation needed | Full workflow validation needed | Mobile limitations apply |
| Native browser | WKWebView with same-view agent bridge | WebView2 pane; separate Chromium automation | Chromium path | Experimental; no desktop parity claim |
| Computer use | Experimental native implementation and bounded live checks | No equivalent native verification | No equivalent native verification | Not a desktop-control host |
| Terminal/dictation | Desktop implementation | Feature-level validation needed | Feature-level validation needed | Unavailable in the documented mobile path |
| Signing | Development identity and Keychain checks on one Mac | Unsigned test installer | No release claim | Apple development/signing setup required |

## What has current macOS evidence

The latest acceptance record includes:

- pi/OMP interrupted-turn output and delegation recovery, with owned POSIX process cleanup.
- Code conversation-team persistence, policy isolation, shared limits, and reviewer evidence.
- Bounded cloud/local bugfix and mixed-team checks.
- Work artifacts, transcripts, and preview Open/Stop/Restart behavior.
- Native browser interactions, failure-state tests, and repeated tab cycles.
- Stable development signing and saved Keychain trust across relaunch and a changed code hash on the tested Mac.

The detailed [acceptance record](https://github.com/aiden0rchad/taucode/blob/main/docs/dogfood-acceptance.md) states the scope and remaining uncertainty. A prior unreproduced browser process exit, untested failure injection, another machine's Keychain behavior, or release notarization should not be inferred from those checks.

## Important limitations

- Power-loss durability and complete cross-engine/cross-platform crash recovery are unverified.
- Unknown pricing remains unknown; one local/cloud comparison does not justify automatic routing or quality claims.
- Reported-token caps can overshoot while requests are in flight.
- Custom engines and full-access tools are not a universal OS sandbox.
- Native browser input can be rejected by sites; backend capabilities differ.
- Checkpoints cannot undo external side effects or excluded files.
- The current frontend includes inherited Sentry reporting unless disabled at build time.
- No full VoiceOver/accessibility audit or independent security audit is claimed.

## Update carefully

Keep a backup of important project files and app state. Read the release's source commit and notes, verify provided checksums, and check its signing/notarization status.

For source checkouts, inspect local changes before pulling, install from the committed lockfiles, and fetch any newly needed Cargo dependencies explicitly. Do not delete app data as a routine update step.

taucode checks GitHub Releases for a newer version and can download the compatible installer inside the app, with progress and retry. It opens the completed installer through the operating system; installation remains manual and automatic updater artifacts stay disabled in the Tauri configuration. See [Updates & reporting issues](updates-and-issues.md).

## Follow development

- [Commits](https://github.com/aiden0rchad/taucode/commits/main/) show landed changes.
- [Actions](https://github.com/aiden0rchad/taucode/actions) show documentation and Windows checks.
- [Issues](https://github.com/aiden0rchad/taucode/issues) track reproducible problems and requests.
- [Engineering references](engineering.md) link deeper implementation and acceptance notes.
