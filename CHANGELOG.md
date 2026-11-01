# Changelog

## 0.7.69 — November 1, 2026

### Desktop reliability and clearer controls

- **Computer Use:** improved app targeting, focus recovery, typing into rebuilt fields, and progress and approval controls. Automatic execution without per-action approvals is now the default; approval mode remains configurable.
- **Chat:** queue or steer follow-ups while a response is running, with improved conversation history, images and titles.
- **Browser:** recover failed navigation and browser creation, restore tabs more reliably, and keep loading and recovery states clear.
- **Tasks and services:** clearer OMP service history, task progress and app-quit stop states.
- **Settings:** keep Codex roles scoped to their project and preserve exact role names and other settings values on macOS.
- **Updates and support:** check GitHub for new releases and prepare editable issue reports with redacted diagnostics.
- **Documentation:** expanded onboarding, installation steps, first-task tutorials and platform guidance.

### Downloads

| Platform | Package |
| --- | --- |
| macOS · Apple Silicon | [Download DMG](https://github.com/aiden0rchad/taucode/releases/download/v0.7.69/taucode_0.7.69_aarch64.dmg) |
| Windows · Intel/AMD 64-bit | [Download installer](https://github.com/aiden0rchad/taucode/releases/download/v0.7.69/taucode_0.7.69_x64-setup.exe) |
| Integrity | [SHA-256 checksums](https://github.com/aiden0rchad/taucode/releases/download/v0.7.69/SHA256SUMS) |

Packages include Node, npm, uv and the app's helper executables. The macOS build manifest records exact bundled runtime versions and the DMG checksum. Source archives are available on the [release page](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.69).

### Before installing

This is an **early-development release**, with no stable-release or cross-platform feature-parity guarantee. The macOS package is development-signed and **not notarized**; the Windows installer is **unsigned**. Native Computer Use has the broadest verification on macOS. Intel Mac, Linux and iOS installers are not included.

Read [Installation & first run](docs/getting-started.md) and [Builds & platform status](docs/releases.md). Keep your existing app data when updating. Downloads are installed manually; taucode does not replace itself.

### Earlier builds

- [October 30 desktop test build](https://github.com/aiden0rchad/taucode/releases/tag/build-2026-10-30-b6848c7)
- [Initial Windows x64 test build](https://github.com/aiden0rchad/taucode/releases/tag/windows-x64-v0.7.68)
