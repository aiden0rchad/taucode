# Builds & platform status

taucode is in **early development**. The source is available for inspection and building; published test artifacts are experimental. A package version or a passing build is not a stable-release or feature-parity guarantee.

This page describes the documented **0.7.68** baseline as of **October 30, 2026**. See [GitHub Releases](https://github.com/aiden0rchad/taucode/releases) for the assets actually published and the commit behind each build.

## Installable artifacts

### Windows x64 test build

The [Windows x64 test release](https://github.com/aiden0rchad/taucode/releases/tag/windows-x64-v0.7.68) contains:

- `taucode_0.7.68_x64-setup.exe`
- `SHA256SUMS`

It is an **unsigned prerelease** for 64-bit Intel/AMD Windows. The release records its source commit. CI checked compilation, silent installation, bundled Node/npm/uv, and a ten-second app startup. Full provider, browser, tool, credential, and task verification is still needed.

### macOS

Build from source using [installation](getting-started.md) and [packaging](packaging.md). Local signed development checks are documented, but they do not establish that a public release asset is notarized. Check an individual release's notes and signature evidence before distributing it.

Draft releases are not public downloads. This documentation does not link to a draft or promise a stable macOS binary.

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

Automatic updater artifacts are disabled in the current Tauri configuration. Do not rely on the old upstream README's update descriptions as a taucode distribution promise.

## Follow development

- [Commits](https://github.com/aiden0rchad/taucode/commits/main/) show landed changes.
- [Actions](https://github.com/aiden0rchad/taucode/actions) show documentation and Windows checks.
- [Issues](https://github.com/aiden0rchad/taucode/issues) track reproducible problems and requests.
- [Engineering references](engineering.md) link deeper implementation and acceptance notes.
