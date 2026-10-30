# MCP connectors & CodeGraph

A **connector** adds tools from another program or service. You can begin with ordinary Chat or the [first Work exercise](first-task.md) without adding a custom connector.

## Understand the choices before adding one

- **MCP** is the communication protocol used by these tool servers.
- **stdio** means taucode starts a local program and communicates with it.
- **HTTP** means taucode connects to a web address.
- **Ask** means tool calls can pause for your approval; **Auto** grants broader automatic use under that connector's policy.

Choose a specific tool you need and follow its own setup instructions. Then use **Test connection** to check that taucode can discover its tools. A successful test does not prove every tool works or is harmless. Keep keys in the marked secret fields. See the [glossary](glossary.md#connector-and-mcp).

Connectors expose tools from Model Context Protocol (MCP) servers. taucode keeps their configuration in its own registry so supported engines receive a consistent set of enabled tools.

## Before adding a server

A stdio connector runs a local executable. An HTTP connector talks to a service. Both can return untrusted content, and their tools can have effects beyond the project folder.

Install or connect only servers you trust. Check what their tools do, where they send data, and what credentials they require. A connector's description is not a security boundary.

## Add a stdio connector

Open **Settings → Connectors**, add a server, and choose the stdio transport. Configure:

| Field | What to supply |
| --- | --- |
| Name and stable ID | A recognizable label and unique identifier. |
| Command | An installed executable or an explicit executable path. |
| Arguments | The server's actual command arguments. Keep credentials out of them. |
| Working directory | A directory the process needs, if applicable. |
| Environment | Non-secret configuration or fields marked as secrets. |

Use the server's official installation instructions. Running a package-manager command such as an on-demand package runner can download and execute code; pin versions where practical and inspect the source before trusting it.

Select **Test connection**. The check starts the server, performs the MCP handshake, lists tools, and shows a result or an error. Successful discovery proves the handshake worked, not that every tool is safe or fully functional.

## Add an HTTP connector

Choose HTTP and enter the endpoint, necessary headers, and bearer token if the service supports one. Avoid putting credentials in the URL. Secret values use the credential-store path instead of ordinary settings values.

taucode does **not** implement OAuth account-connection flows for connectors. A server that requires an interactive OAuth sign-in needs a supported token/API-key alternative; an authorization error is not a reason to disable authentication.

## Control tool delivery and approval

| Control | Purpose |
| --- | --- |
| Enabled switch | Whether the connector is delivered to supported new sessions/turns. |
| Tool allowlist | Narrow the tool set to explicitly named operations. |
| Tool denylist | Exclude specific operations. |
| Ask approval | Pause for approval of connector tool calls. |
| Auto approval | Permit delivered tools under that server's policy; use only for trusted, suitably narrow tools. |

Workspace overrides can enable/disable servers or add workspace-specific entries. The global **Connectors** feature switch remains authoritative. Engine changes usually apply at session/turn boundaries, so use a fresh turn after reconfiguring tools.

A read-only conversation policy does not make an arbitrary remote tool read-only. Choose the tools and approval settings that match the work you intend to authorize.

## Built-in tools

taucode uses the same registry for its own integrations:

| Integration | Purpose | Additional requirements |
| --- | --- | --- |
| CodeGraph | Indexed code navigation through the proxy | Separately installed CodeGraph and a usable project index. |
| Browser | Navigation, snapshots, screenshots, and supported interaction | Browser feature/backend setup and its policies. |
| Computer | Desktop-app inspection and input | macOS support, model setup, permissions, and its policies. |
| Diagnostics reader | Agent-readable local journal | Opt-in diagnostics, connectors, and explicit exposure to taucode agents. |

Built-in entries are managed by the app and can have reserved IDs. Do not duplicate them with another server of the same ID.

## CodeGraph

CodeGraph is an optional separate code-intelligence tool. taucode does not bundle it by default. Install it separately and use its own documented indexing workflow for the projects you want indexed.

The proxy exposes the project's graph to supported agents and manages context-deduplication state. An index can reduce repeated source discovery, but it is not a compiler, test runner, or proof that an answer is correct.

If CodeGraph is absent or the project is not indexed, the default pi workflow can still use ordinary file tools. Do not assume a `.codegraph` directory has a healthy, current index simply because the directory exists.

## Secrets and configuration

Provider and connector secrets use the managed credential store and child environment/header plumbing. Generated files use references instead of copying stored values into ordinary command arguments or settings JSON.

The server itself receives the secrets needed for its transport. A malicious or compromised server can misuse them. Prefer narrowly scoped credentials and separate server entries for unrelated services.

## Troubleshoot a connector

1. Confirm the executable or URL works on the host running the engine.
2. Check the process working directory and required non-secret environment.
3. Run **Test connection** and read the actual handshake error.
4. Confirm the server and desired tool are enabled and not filtered out.
5. Start a new turn/session after configuration changes.
6. Check the feature switch and the engine's supported delivery path.

For remote daemons, local executable paths, `localhost`, and credential availability refer to the daemon host. See [remote setup](remote.md) and [privacy](privacy.md).
