# Privacy, storage & trust

taucode is a desktop app that stores working state locally, but the features you enable can contact model providers, package registries, websites, and connector services. Choose those services with the same care as the source material you give them.

## What can leave your computer

| Activity | Potential destination and content |
| --- | --- |
| Model conversation | Your selected provider receives the prompt and relevant conversation context. |
| Agent tools | File excerpts, command results, and connector output may enter subsequent model context. |
| Chat attachments | The explicitly attached text or image can be sent to the selected model provider. |
| Browser use | Websites receive normal navigation requests; page content or screenshots may reach the agent's model. |
| Computer use | The configured planner/grounding providers may receive screenshots and relevant app context. |
| Runtime installation | Package registries and runtime distribution services supply dependencies on first install or update. |
| MCP connector | The local server or remote endpoint receives the arguments and credentials required for its tools. |
| Remote daemon | Requests, tool results, and workspace operations cross the configured connection to that host. |
| Sentry reporting | The current frontend initializes inherited Sentry reporting unless disabled at build time. |

Using a local model changes the inference destination. It does not automatically disable website access, connector traffic, runtime downloads, or Sentry.

## Sentry and the local journal are separate

The current frontend entry point includes an inherited Sentry DSN and records an app-open metric. It initializes the Sentry SDK outside UI-preview mode. Do not describe the current source as telemetry-free or assume all error context stays local.

`VITE_SENTRY_DSN` overrides that default. An explicitly empty value disables initialization for a source build. Because this is a Vite variable, set it **when building**, not only when launching an already built app.

From `apps/taucode` on macOS/Linux:

```sh
VITE_SENTRY_DSN= npm run tauri:build
```

On Windows, add the line `VITE_SENTRY_DSN=` to `apps/taucode/.env.production.local` without overwriting any existing settings, then run `npm run tauri:build:win`. This avoids older PowerShell versions treating an empty environment variable as an unset variable. The local file is ignored by Git.

The offline UI preview disables Sentry regardless of the default DSN. A distributor can build with a different DSN; inspect the source/configuration for the exact build you use.

The **Diagnostics journal** is a separate, opt-in local feature. It has no network transport of its own and is off by default. It records bounded operational metadata and user-submitted notes; optional content snippets and any manual sharing require additional care. See [diagnostics](diagnostics.md).

## Credentials

Provider API keys and marked connector secrets use the managed credential-store integration. macOS uses Keychain. Settings and generated runtime configuration reference those credentials rather than embedding their values as ordinary provider settings or command arguments.

Environment authentication and auth commands are alternatives. Their security depends on the launch environment and trusted helper. Engine and connector child processes receive the credentials needed for their role; a compromised process can misuse credentials it is given.

Custom engine definitions reject secret-like environment keys and certain loader/path overrides. That validation reduces configuration mistakes; it does not establish that a custom executable is safe.

For Windows and Linux, native credential/runtime behavior needs platform-specific validation. The existing Windows build/startup checks are not a full credential-storage audit.

## Local storage

The desktop app uses Tauri's platform app-data location for the identifier `dev.taucode.desktop`. On macOS, that is normally:

```text
~/Library/Application Support/dev.taucode.desktop/
```

The standalone daemon can use an explicit `--data-dir`. Its files and credentials belong to the host/user running it.

| Data | Where or how it is kept |
| --- | --- |
| App/workspace settings | Managed app-data settings and workspace files. |
| Engine homes and sessions | Isolated managed directories, including per-engine/workspace profiles. |
| Chat history | A separate Chat store under app data. |
| Work sources and outputs | The folder you selected as the space, with generated deliverables normally in `outputs/`. |
| Checkpoints | Separate shadow Git stores under app data. |
| Custom themes | `themes/<id>.taucode-theme.json` under app data. |
| Diagnostics | `diagnostics/`, including event files, issue aggregation, annotations, and digest. |
| Evals | `evals/runs/` and associated retained test workdirs. |
| Browser data | Backend-specific profile/data stores; the native macOS store is separate from the app's webview. |

Local storage is not automatically encrypted by taucode. Use OS disk encryption and appropriate user-account protection for sensitive material. Back up both project files and any app state you need; do not assume a folder backup contains the separate checkpoint/session store.

## Browser boundaries

Browser content uses a separate webview/profile path rather than inheriting the application's privileged command surface. Native browser navigation rejects app/file/IPC schemes and applies configured origin rules. macOS 14+ supports a separate persistent native data store; older macOS and private browsing use nonpersistent stores.

Native DOM automation is synthetic, and a website can refuse it. Cross-origin iframe contents and some advanced inspection features have backend limitations. See the [browser reference](browser.md) before treating a blocked action as a policy problem.

## Tool policies are not a universal sandbox

taucode gates supported file, shell, connector, and desktop actions. Built-in engines receive isolated homes and a filtered environment. These controls matter, but they do not make every possible third-party executable or full-access shell process an operating-system sandbox.

Custom ACP agents must honor the protocol's permissions. HTTP tools can act on remote systems with the authority of their token. Read-only metadata cannot independently constrain a service that exposes destructive operations under the same credentials.

Use narrow credentials, limited tool sets, the appropriate access mode, and a test folder when trying a new integration.

## External actions and recovery

Checkpoints cover eligible project files. They cannot unsend messages, revoke an upload, undo a purchase, or restore state inside another app. An interrupted turn may already have made changes.

Review the actual action and destination before approving sensitive operations. Keep human review at the boundary where a tool would publish, transmit, or delete important information.

## Sharing logs and filing issues

Review screenshots, exported chats, diagnostics, model responses, and command output before attaching them publicly. Remove:

- Provider keys, connector tokens, cookies, and authorization headers.
- Private paths, hostnames, email addresses, personal files, and customer data.
- Secrets echoed by a command or included in an error body.

Automated redaction is bounded and cannot recognize every private value. Report vulnerabilities privately through the repository's [security policy](https://github.com/aiden0rchad/taucode/blob/main/.github/SECURITY.md).
