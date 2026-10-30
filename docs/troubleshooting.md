# Troubleshooting

Start with the boundary that failed: installation, runtime, provider, tool, or UI. Preserve the error message and partial work before retrying.

## First things to try

1. **Read the visible error.** Write down which step failed and what the message says. Do not keep resending a paid request without checking it.
2. **Check the mode and model.** Is this Chat, Code, or Work? Is the provider/model enabled? For a file task, is the correct folder selected?
3. **Check the status.** Queued means waiting; Needs input means a question or approval needs attention. Neither necessarily means the app has frozen.
4. **Try a smaller example.** A short Chat message checks the model connection. The [first Work task](first-task.md) checks a simple file workflow with sample data.
5. **Open the matching section below.** If it is still unclear, report the exact steps and a sanitized error. Leave API keys and personal files out of the report.

**Provider** means the model service; **engine** means the background conversation/tool program; **UI** means the visible interface. Other terms are in the [glossary](glossary.md).

## Quick diagnosis

| Symptom | First check |
| --- | --- |
| App does not build | Node/Rust/native prerequisites and Cargo's offline cache. |
| Model picker is empty | Enabled provider, successful discovery, and enabled models. |
| Model list works but messages fail | Inference route, model access, tool/image support, quota. |
| Engine cannot start | Enabled/installed runtime and executable availability. |
| Missing Chat/Work/Git controls | Settings → Features. |
| Connector tools do not appear | Enabled server, tool filters, feature switch, and a fresh session/turn. |
| Native browser cannot interact | Backend limitations, stale snapshot, or synthetic input rejected by the site. |
| Computer capture/input fails | macOS permissions for the exact app build and allowed target. |
| Repeated Keychain prompts | Signing identity and saved Keychain approval. |
| Task remains in Needs input | Open its approval/question; being queued is a different state. |

## Source-build failures

Run from `apps/taucode`, not the repository root:

```sh
node --version
rustc --version
cmake --version
npm run doctor:strict
```

Node must be at least 22.19. On Windows, use `npm run doctor:win` and the `:win` development/build scripts.

### Cargo cannot find a package offline

The native app’s `src-tauri/.cargo/config.toml` defaults to offline operation. Populate the cache explicitly:

```sh
CARGO_NET_OFFLINE=false cargo fetch --locked --manifest-path src-tauri/Cargo.toml
```

PowerShell uses `$env:CARGO_NET_OFFLINE = "false"` before `cargo fetch`. Keep the lockfile; deleting it can introduce unrelated dependency changes.

### Native compilation fails

CMake is needed for native dependencies. Windows bindgen also needs libclang and a correct `LIBCLANG_PATH`; use the Visual Studio C++ build tools and Windows SDK. macOS needs a usable Xcode/Swift toolchain. Linux needs the distribution's Tauri/WebKitGTK dependencies.

`doctor:strict` on Unix checks CMake. It is not a complete environment audit. Read the first actual compiler error, rather than only the final Cargo/Tauri exit message.

### Port 1421 is occupied

The offline preview uses a strict port. Stop the process that owns that preview port or run a second preview with an explicit different port. Do not terminate an unidentified process just to free the port.

## Provider requests

### Authentication fails

Check the provider type, base URL, key, and model entitlement. An API key for one account or endpoint may not work with another. Consumer subscription sign-in is not supported.

If using environment authentication, confirm the variable exists in the app's launch environment. A GUI launch may not inherit terminal variables. For command authentication, test the trusted helper outside taucode without copying its secret output into an issue.

### Discovery succeeds, inference fails

The model-list endpoint does not verify streaming, tools, images, reasoning levels, or the selected engine's API. Test a short text-only request first, then add the failing capability.

A compatible chat-completions server may not provide the Responses endpoint needed by Codex. Try a compatible engine/provider pair instead of repeatedly changing an otherwise valid key.

### Rate limits or quotas

Read the provider's error details. A retry can incur another request and can repeat tools if the conversation continues. Wait for the provider's actual reset or select another configured model with available capacity.

The latest failure card may offer **Change model** and **Retry**. Retry preserves the current draft and files; it does not revert edits from the failed turn.

## Engine installation and startup

In **Settings → Engines**, check that the engine is enabled and ready. **Install/Retry** prepares its managed runtime; it can require internet and substantial disk space. OMP installation is approximately 1.1 GB.

Codex and CodeGraph are optional separately installed integrations. Custom engines need an executable that supports the selected protocol and taucode's required handshake. A successful handshake alone does not prove a custom engine supports every session operation.

For **Agent stopped unexpectedly**, inspect retained progress and use a follow-up. Unfinished commands are not restarted automatically. Do not use an old provider error's Retry action as a substitute for understanding a terminated engine.

## macOS Keychain prompts

Keychain access depends on the code signing identity and saved trust for the provider key. Switching builds or signing identities can require another approval.

For development, choose a stable existing Apple-issued identity using the signing workflow in [packaging](packaging.md). When macOS asks whether taucode can read the provider key, **Always Allow** saves that access; **Allow** authorizes only the current read.

The build-time prompt allowing **codesign** to use a signing private key is separate from taucode reading a provider key. Screen Recording and Accessibility permissions are separate again.

Verify a provider request after a full quit/relaunch. A successful request during a manually approved prompt does not establish prompt-free access after the next rebuild. Do not remove private keys or reset the entire Keychain to address a single app prompt.

## Browser problems

### A site ignores clicks or typing

The native macOS bridge dispatches synthetic DOM input; sites can reject it. Try **Settings → Browser → Engine → Headless Chromium** for a site that needs trusted automation input. Refresh stale snapshot references before retrying.

### The pane is blank or failed

Use the visible Retry control, confirm the URL and origin rules, and check that the backend is available. On macOS, native agent control needs the desktop host; daemon-only sessions should use Chromium.

Windows uses a native WebView2 pane with separate Chromium automation. Do not assume its automation operates on every visible native-pane state in the same way as macOS.

### Clear browsing data

Use the browser settings' clear-data control. macOS native per-site clearing follows WebKit's domain grouping; Windows currently uses the broader Settings clearing path. Clearing data can sign you out of sites.

## Computer-use permissions

Check **Settings → Computer Use** and the exact app entry in macOS Privacy & Security. Grant the required permission yourself and quit/reopen for Screen Recording changes. A checkbox in the wrong development build's entry will not fix the running app.

If permissions are correct, test a simple allowed application and verify planner/grounding compatibility. See [computer use](computer-use.md); not every target app supports background accessibility actions.

## Work tasks and output files

Check whether the task is Queued, Running, Needs input, Failed, or Stopped. A waiting approval is not a frozen queue. Open the task to answer its request.

For a missing output, inspect its actual reported path and the space folder. Files named in a response may not exist or may belong to an older run. A preview server is a running process, not a permanent hosted site.

Before retrying a failed task, check partial files. Repeating a brief can duplicate or overwrite outputs. Use a narrower follow-up or an eligible checkpoint when appropriate.

## Checkpoints unavailable

Inspect the turn's checkpoint status. Ignore rules, large files, total snapshot size, disallowed root folders, and timeouts can prevent complete recovery. Checkpoints intentionally do not cover every file or external action.

Read [checkpoints & rewind](checkpoints.md) before restoring. Keep the original data safe if the restore reports an error.

## Report a useful bug

Include the app/build identifier, OS and architecture, engine, provider type, model ID if safe to share, minimal steps, expected behavior, and actual error. Say whether it reproduces in the offline preview or only in the native app.

If diagnostics are enabled, attach only the relevant sanitized evidence after reviewing it. Never post keys, private endpoint credentials, cookies, or personal files. Use the [issue form](https://github.com/aiden0rchad/taucode/issues/new/choose); use the private [security policy](https://github.com/aiden0rchad/taucode/blob/main/.github/SECURITY.md) for vulnerabilities.
