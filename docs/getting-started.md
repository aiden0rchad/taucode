# Installation & first run

Start with a source build or the experimental Windows installer, connect one provider, and run a small task. You can explore the UI without credentials using the preview described below.

## Choose an installation path

| Path | Who it is for | What to expect |
| --- | --- | --- |
| Source build on macOS | Developers and early testers | Broadest native verification; signing and permissions may need local setup. |
| Windows x64 test installer | Early testers on Intel/AMD Windows | Unsigned installer; compilation, installation, runtime, and startup checks exist. Full feature verification is still needed. |
| Source build on Windows | Developers | Requires Windows build tools, LLVM/Clang, and WebView2. |
| Source build on Linux | Contributors | Tauri prerequisites and native dependencies required; full taucode feature parity is unverified. |
| UI preview | Anyone exploring the interface | Sample data only; no model requests, real filesystem tasks, or native app control. |

See [builds & status](releases.md) for the difference between a test artifact and a stable release.

## Build from source

### Prerequisites

Install these before starting:

| Dependency | Purpose |
| --- | --- |
| Node.js **22.19+** and npm | Frontend, runtime installation, and bundling. Node 22 is the CI baseline. |
| Stable Rust toolchain with Cargo | Tauri host, daemon, and native helpers. |
| Git CLI | Source checkout, branches, worktrees, and checkpoint storage. |
| CMake | Native dependencies, including dictation/Whisper. |
| Platform build tools | Follow the official [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). |

On **macOS**, install Xcode and its command-line tools. The computer-use implementation includes Swift/native components, so a complete working Apple toolchain matters beyond frontend development.

On **Windows**, install the Visual Studio C++ build tools, Windows SDK, WebView2, CMake, and LLVM/Clang. If bindgen cannot find libclang, configure `LIBCLANG_PATH` to the LLVM `bin` directory. The Windows CI uses `C:\Program Files\LLVM\bin`.

On **Linux**, install your distribution's Tauri/WebKitGTK and native build dependencies. Computer use is currently a macOS implementation; a successful Linux compile does not establish support for it.

Codex CLI and CodeGraph are **optional** integrations. They are not required for the default pi engine. GitHub CLI (`gh`) is optional for GitHub issue and pull-request features.

### macOS or Linux commands

```sh
git clone https://github.com/aiden0rchad/taucode.git
cd taucode/apps/taucode
npm ci
CARGO_NET_OFFLINE=false cargo fetch --locked --manifest-path src-tauri/Cargo.toml
npm run doctor:strict
npm run tauri:dev
```

`apps/taucode/src-tauri/.cargo/config.toml` defaults to offline mode. The network override on `cargo fetch` fills the dependency cache; subsequent builds use it. If Cargo says a package is unavailable offline, repeat the fetch rather than deleting lockfiles.

`doctor:strict` checks CMake on Unix. It does not verify the entire Rust, Swift, browser, provider, or signing setup.

### Windows commands

Run in PowerShell after installing the prerequisites:

```powershell
git clone https://github.com/aiden0rchad/taucode.git
Set-Location taucode/apps/taucode
npm ci
$env:CARGO_NET_OFFLINE = "false"
cargo fetch --locked --manifest-path src-tauri/Cargo.toml
Remove-Item Env:CARGO_NET_OFFLINE
npm run doctor:win
npm run tauri:dev:win
```

For the packaged test build, use the [Windows x64 test release](https://github.com/aiden0rchad/taucode/releases/tag/windows-x64-v0.7.68). Its executable and `SHA256SUMS` are release assets. Compare your downloaded installer's SHA-256 with the checksum file before running it:

```powershell
Get-FileHash .\taucode_0.7.68_x64-setup.exe -Algorithm SHA256
Get-Content .\SHA256SUMS
```

The installer is unsigned. An OS warning does not establish that a file is safe; use the official release or build from source.

## Explore without an API key

From `apps/taucode`:

```sh
npm ci
npm run preview:ui
```

Open `http://localhost:1421`. Useful sample states:

| URL suffix | Preview |
| --- | --- |
| `/?setup=1` | First-run checklist |
| `/?mode=code&thread=thread-rich` | Code conversation, agents, and plan |
| `/?mode=chat&chat=chat-1` | Chat conversation |
| `/?mode=work&task=task-running` | Work task, plan, and outputs |
| `/?settings=appearance` | Theme gallery and appearance settings |
| `/?settings=diagnostics` | Sample diagnostics inbox |

This renders the app with mocked native APIs and in-memory state. Reloading resets changes. It does not test a provider, native browser, real terminal, Keychain, or computer permissions. Sentry is disabled in this mode.

## Complete the setup checklist

The first-run checklist groups setup into appearance, provider, engine, project, features, and permissions. It is dismissible; reopen it through **Settings → About → Setup checklist**.

1. **Choose your appearance.** Start with a built-in light or dark theme. Custom themes can wait.
2. **Add one provider.** In **Settings → Providers**, choose the provider type, supply authentication, and use **Test & fetch models**.
3. **Enable a model.** Select an available model and configure a default in **Settings → Agents**.
4. **Prepare an engine.** pi is the default. Use **Settings → Engines → Install** if its runtime is missing. The first install needs internet access.
5. **Add a folder.** Add a small project in Code or create a Work space pointing at a test folder.
6. **Review features.** Enable browser, computer use, Git, or connectors when you need them. Computer use has separate macOS privacy permissions.

Provider model discovery and actual inference are different checks. A successful model-list request does not prove that every model supports tools, images, streaming, or your selected reasoning level.

## Run a first task

For **Code**, start with a repository you can safely experiment in:

```text
Explain this project's structure and how to run its checks. Do not change files.
```

Review the selected engine, model, and access policy before sending. Follow with one small implementation request and inspect its diff.

For **Chat**, choose a model and ask a question. Chat uses a restricted pi profile and does not inherit a Code project's file tools.

For **Work**, use a test folder and a bounded brief:

```text
Read the notes in this folder and create a one-page summary in outputs/.
Leave the source files unchanged.
```

Inspect the task's plan and outputs before using the generated files. Model output can be incorrect even when a task is marked complete.

## Build an app bundle

From `apps/taucode`, after fetching dependencies:

```sh
npm run tauri:build
```

Windows uses `npm run tauri:build:win`. Platform bundles are written under `apps/taucode/src-tauri/target/release/bundle/`.

macOS distribution builds need runtime staging, a signing identity, and optionally notarization credentials. Follow [macOS packaging](packaging.md); a successful development launch does not establish a distributable signed build.

## Where to go next

- [Providers](providers.md) for cloud APIs, local endpoints, authentication, and model discovery.
- [Code](code.md), [Chat](chat.md), or [Work](work.md) for the daily workflow.
- [Privacy & storage](privacy.md) before sending sensitive material or enabling desktop automation.
- [Troubleshooting](troubleshooting.md) for setup, engine, Keychain, and build failures.
