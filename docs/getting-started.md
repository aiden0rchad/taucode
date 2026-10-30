# Installation & first run

Get taucode running, connect one AI model, and send your first message. If words such as **provider** and **engine** are new to you, read [Start here](start-here.md) first.

## Before you start

You need the app and a way to run an AI model:

- A provider API account and key, or an existing local model server you can connect to.
- Internet access for initial runtime installation and any cloud provider you use.
- A small amount of non-sensitive sample text for your first message.

An **API key** is a secret credential from the provider’s API dashboard. Cloud API use can cost money separately from a ChatGPT or Claude subscription. taucode does not sign in to those consumer subscriptions.

## 1. Install the app

The current downloads are **early test builds**, not a stable release. The [test release page](https://github.com/aiden0rchad/taucode/releases/tag/build-2026-10-30-b6848c7) has the files, checksums, and build notes.

| Your computer | Download or setup path |
| --- | --- |
| Mac with Apple Silicon | `taucode_0.7.68_aarch64.dmg` from the test release. |
| Windows on a 64-bit Intel/AMD PC | `taucode_0.7.68_x64-setup.exe` from the same test release. |
| Intel Mac or Linux | Use the advanced [source-build instructions](#build-from-source); further platform validation is needed. |
| iPhone/iPad | Experimental [remote/iOS setup](remote.md); not a beginner desktop-install path. |

### macOS: Apple Silicon

1. Choose **Apple menu → About This Mac**. A **Chip** entry naming Apple M-series hardware means Apple Silicon. An Intel processor needs a different build path.
2. On the [test release page](https://github.com/aiden0rchad/taucode/releases/tag/build-2026-10-30-b6848c7), expand **Assets** if needed and download **taucode_0.7.68_aarch64.dmg**. The source-code ZIP is for developers, not the installer.
3. Open the downloaded DMG and copy **taucode.app** into **Applications**.
4. Open taucode from Applications.

This package has a local development signature and is **not notarized by Apple**. If macOS blocks opening it, read the release notes and [Apple’s explanation of downloaded-app warnings](https://support.apple.com/en-us/102445). If you choose to trust this specific download, Apple describes the **System Settings → Privacy & Security → Open Anyway** path after an attempted launch. Do not disable Gatekeeper globally. A damaged/malware warning needs investigation, not the same assumption as an unidentified-developer warning.

**Expected result:** the taucode window opens. The first-run checklist may appear. You do not need Node, Rust, or a terminal to use this packaged download.

### Windows: Intel/AMD x64

1. On the [test release page](https://github.com/aiden0rchad/taucode/releases/tag/build-2026-10-30-b6848c7), expand **Assets** and download **taucode_0.7.68_x64-setup.exe**.
2. Open the downloaded installer and follow its installation steps.
3. Open taucode from its installed shortcut or Start menu entry.

The installer is **unsigned**. Review any OS warning and the release notes before deciding whether to run it. It has build/install/startup checks, but full feature testing is still needed. There is no published Windows ARM installer in this release.

**Expected result:** the taucode window opens. The download includes the supporting runtimes; you do not need to install the development tools listed later just to run it.

For checksum comparison, see the Windows commands under [Build from source](#windows-commands). A matching checksum confirms the published file matches your copy; it does not certify the software as safe.

## 2. Connect one provider and model

The setup checklist can take you through these steps. You can also open **Settings** yourself. To reopen the checklist later, use **Settings → About → Setup checklist**.

1. Open **Settings → Providers → Add provider**.
2. Select a provider you already have API access to. Use its default address unless your setup requires another one.
3. Give the entry a recognizable name and complete its authentication fields. For API-key authentication, enter the key in the key field, not in a chat message.
4. Save and choose **Test & fetch models**. This asks the provider which models are available.
5. Enable one model from the returned list.
6. Open **Settings → Agents** and choose that model as a default.

For an existing Ollama or LM Studio server, follow the [local provider instructions](providers.md#local-servers-and-gateways). You do not need both a cloud provider and a local server.

**Expected result:** your provider has a model list, and at least one enabled model appears in model pickers. Discovery alone does not prove a real message will succeed; test that next.

## 3. Prepare the default engine

Open **Settings → Engines**. Start with **pi**, the default background program that manages conversations and tools.

If pi needs installation, choose **Install** and let it finish. This can download dependencies the first time. You can leave custom engine commands, agent teams, and extra connectors for later.

**Expected result:** pi is enabled and ready rather than missing, installing, or showing an error. See [the engine guide](engines.md#start-with-the-default-engine) if it is not ready.

## 4. Send a first Chat message

1. Choose **Chat** in the sidebar, then **New chat**.
2. Check that your enabled model is selected.
3. Type this message and send it:

```text
Explain what a to-do list is in two short sentences.
```

**Expected result:** you see a written answer. The wording will vary. This test needs no attachments, project folder, browser access, or desktop permissions.

If the provider list works but this fails, check the [connection checklist](providers.md#check-that-it-works). A 401/403 error usually concerns authentication/access; a quota error concerns the provider's allowance. Avoid resending repeatedly without reading the error.

## 5. Choose your first real workflow

| You want to… | Next guide |
| --- | --- |
| Ask questions or discuss attached text/images | [Chat](chat.md) |
| Make a file from sample notes, without coding | [Your first Work task](first-task.md) |
| Explore or change a coding project | [Code](code.md) |

For your first file task, use the practice folder in the walkthrough. Open the resulting file and check its content yourself. An AI response or Done label is not proof that every fact or file is correct.

## Build from source

### Prerequisites

This section is for developers and early testers comfortable using a terminal. If you downloaded an installer, skip this section. A **source build** means turning the project’s code into a running app on your computer.

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

For the packaged test build, use the [Windows x64 test release](https://github.com/aiden0rchad/taucode/releases/tag/build-2026-10-30-b6848c7). Its executable and `SHA256SUMS` are release assets. Compare your downloaded installer's SHA-256 with the checksum file before running it:

```powershell
Get-FileHash .\taucode_0.7.68_x64-setup.exe -Algorithm SHA256
Get-Content .\SHA256SUMS
```

The installer is unsigned. An OS warning does not establish that a file is safe; use the official release or build from source.

## What the source-build commands mean

Run terminal commands in a terminal app, not in taucode’s message box. Use Terminal on macOS/Linux or PowerShell on Windows. Run one line at a time and wait for it to finish before the next; if a command fails, read the error before continuing.

| Command | What it does |
| --- | --- |
| `git clone` | Downloads the source into a folder named `taucode`. |
| `cd` / `Set-Location` | Moves the terminal into the application’s folder. |
| `npm ci` | Installs the JavaScript dependencies from the committed list. |
| `cargo fetch` | Downloads the Rust dependencies for later builds. |
| `doctor:strict` / `doctor:win` | Checks some local build prerequisites. |
| `tauri:dev` / `tauri:dev:win` | Builds and starts the development app; keep the terminal running while using it. |

The words after `npm run` are the project’s script names. Copy commands exactly and leave out any prompt symbol your terminal displays before them.

## Explore without an API key

This is a **developer preview**, not a public click-to-try demo. It requires the Node/npm source setup above. From `apps/taucode`:

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

## Build an app bundle

From `apps/taucode`, after fetching dependencies:

```sh
npm run tauri:build
```

Windows uses `npm run tauri:build:win`. Platform bundles are written under `apps/taucode/src-tauri/target/release/bundle/`.

macOS distribution builds need runtime staging, a signing identity, and optionally notarization credentials. Follow [macOS packaging](packaging.md); a successful development launch does not establish a distributable signed build.

## Where to go next

- [Start here](start-here.md) for the basic concepts and which mode to choose.
- [Your first Work task](first-task.md) for a complete practice exercise.
- [Glossary](glossary.md) for unfamiliar words.
- [Troubleshooting](troubleshooting.md) if a step does not match what you see.
- [Privacy & storage](privacy.md) before sending sensitive material or enabling desktop automation.
