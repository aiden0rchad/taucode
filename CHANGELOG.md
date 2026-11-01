# Changelog

## 0.7.70 — November 1, 2026

### A more cohesive desktop interface

- **Typography:** use Codex Desktop's system-font stack across the interface and chat, with lighter macOS text smoothing. Existing default settings update automatically; custom fonts and monospace code remain intact.
- **Projects:** restore squircle project cards with room around their contents, tighter chat rows and clearer selection.
- **Session rail:** dock beside wide chats and float over narrower chats from one toolbar toggle. Share the conversation's gradient and translucency, use the full available height, and simplify section headers and reordering controls.
- **Scrolling and resizing:** quieter chat scrollbars, no visible rail scrollbar, and less work during sidebar and pane resizing. Preserve mounted panes and drafts while moving dividers.
- **Window and settings:** improve window-size restoration and titlebar spacing at reduced UI scales; add settings search and consistent dialog and feedback layouts.
- **Roles:** use the current project for role editing and remove the redundant global custom-agent editor. Existing global definitions remain unchanged.
- **Updates:** download the compatible GitHub release installer inside taucode, with progress, retry, size verification and SHA-256 verification when GitHub provides a digest. Open the completed installer through the operating system; installation remains manual.

### Release status

Published as a [macOS Apple Silicon release](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.70), with a [DMG installer](https://github.com/aiden0rchad/taucode/releases/download/v0.7.70/taucode_0.7.70_aarch64.dmg), build manifest and SHA-256 checksums. Windows remains on [0.7.69](https://github.com/aiden0rchad/taucode/releases/tag/v0.7.69). This remains an early-development release; the macOS package is development-signed and not notarized.

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
