# Frequently asked questions

## What is taucode?

A Tauri desktop app for AI coding, conversations, and folder-based document work. It builds on CodexMonitor and adds a provider registry, multiple engines, Chat/Work profiles, agent policies, browser/computer tools, checkpoints, and evaluations.

## What is the difference between a provider, a model, an engine, and a connector?

| Term | Meaning | Example |
| --- | --- | --- |
| Provider | The endpoint/service receiving inference requests | A cloud API, Ollama, or your own compatible gateway. |
| Model | The specific model served by that endpoint | An enabled exact model ID in the provider catalog. |
| Engine | The agent runtime managing the tool loop and session | pi, OMP, Codex, or an ACP agent. |
| Connector | A tool server the engine can call | A browser, code-intelligence, or third-party MCP server. |

Choose providers in [Providers](providers.md), runtimes in [Engines](engines.md), and tools in [Connectors](connectors.md).

## Is it free?

The repository's code is MIT licensed. Your provider can charge for API requests, and local models need hardware and electricity. A missing price in taucode means unknown, not free. Third-party engines, models, and assets retain their own licenses.

## Can I use my ChatGPT or Claude subscription?

taucode does not connect consumer subscriptions or expose an account-login flow. Use a provider API key, supported environment/auth command, or a local server.

## Does it run fully offline?

Some workflows can use a local server after runtimes and dependencies are installed. That does not automatically disable browser requests, HTTP connectors, package downloads, or the current inherited Sentry reporting. Read [privacy](privacy.md) for the exact boundaries and source-build telemetry setting.

## Which platforms are supported?

macOS has the broadest native verification. Windows has an unsigned x64 test installer with build, installation, runtime, and short startup checks. Linux and iOS need further validation. Desktop computer use is currently macOS-specific. See [builds & status](releases.md).

## Do I need Codex installed?

No. pi is the default engine. Install Codex separately only if you want that optional engine. CodeGraph is also optional and separately installed by default.

## Can I add another agent engine?

Yes, through a custom ACP or pi-RPC definition if its executable implements the required protocol and behavior. Use Test to check the handshake. Custom programs must be trusted and must honor the permission contract; protocol compatibility is not an OS sandbox.

## Does changing the engine change Chat and Work?

Code uses the selected enabled engine. Chat and Work use dedicated pi profiles. Work has its own tools and skills; Chat starts without project/shell tools and can enable only a restricted browser subset through Web.

## Can I use a local model for a team role?

Yes, if it is enabled in the provider registry and compatible with the selected engine. Assign it explicitly to the role. Inheritance does not automatically choose the cheapest model or route work between local and cloud models.

## Is Chain of Command available everywhere?

It is a Code feature on supported pi/OMP paths. Chat and Work use a single-model profile when inheriting that policy. Custom delegation is a separate mode. See [agents](agents.md).

## Are token allowances hard spending caps?

No. They use reported usage, and in-flight requests can exceed the threshold before usage arrives. Unknown prices stay unknown. Provider-side billing controls remain useful.

## Can the agent access my whole computer?

Access depends on the profile, selected policy, connector permissions, and enabled desktop tools. Chat is restricted; Code and Work can run tools with real effects. Full-access shells, trusted custom executables, and remote MCP tools are not universally confined by an OS sandbox. Read [privacy & trust](privacy.md).

## Can I undo everything with rewind?

No. Checkpoints cover eligible workspace files and supported conversation state. They exclude ignored/oversized files and cannot undo external messages, uploads, website submissions, or state inside another app. See [checkpoints](checkpoints.md).

## Why does Keychain ask again after a rebuild?

macOS evaluates app signing identity and saved key access. Use a stable development identity and the appropriate saved approval. Keychain, signing-tool private-key access, and Screen Recording/Accessibility are separate prompts. See [troubleshooting](troubleshooting.md#macos-keychain-prompts).

## Why does a website ignore the native browser's input?

Native DOM input is synthetic, and some sites require trusted events. Try the Headless Chromium backend when appropriate. Backend behavior and cross-origin iframe limits are documented in [Browser](browser.md).

## Can it run tasks after I quit?

Work can continue while the window is hidden and the runtime remains running. A separately running daemon can own remote work. This is not an always-on cloud scheduler, and quitting the process can interrupt tasks.

## How can I help?

Try a bounded workflow, report a reproducible problem without private material, improve a guide, or contribute a focused patch. Start with [development](development.md) and the [contribution guide](https://github.com/aiden0rchad/taucode/blob/main/.github/CONTRIBUTING.md).
