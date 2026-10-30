# Plain-language glossary

You only need the first few terms to begin. The advanced terms are here so you can look them up when a setting or error mentions them.

## Provider

The service or local server that runs your AI request. A cloud provider runs it on its servers; a local model server runs on a computer you control. [Provider setup](providers.md).

## Model

The particular AI available from a provider. Different models can have different prices and abilities, such as handling images or using tools. Select one enabled model to start.

## Engine

The background program that manages a conversation and the model's tool requests. **pi** is taucode's default. An engine and a model are separate choices. [Engines](engines.md).

## API key

A secret credential that allows an app to use your provider account. An **API** is the connection an app uses to send requests to a service. The key can authorize billable requests. Enter it in provider settings and keep it out of messages, screenshots, and public bug reports.

## Space, project, and workspace

A **space** is a folder selected for Work. A **project** is a folder selected for Code. Technical references may call either kind a **workspace**. These are real folders on the computer running the task.

## Task, conversation, and turn

A **conversation** is the history of your messages and the agent's replies. A Work **task** is a conversation with a brief, progress, and outputs. A **turn** is one request and the work/reply that follows it. Technical pages may call a conversation a **thread**.

## Agent and tool

An **agent** is the model working through an engine with the tools its profile allows. A **tool** is an operation such as reading a file or running a command. It has real effects according to the permissions involved.

## Brief and output

A **brief** is your description of the work: input, desired result, destination, and constraints. An **output** is a file the task produces. Work normally puts deliverables in a folder named `outputs/`.

## Access and approval

An **access setting** controls which actions are allowed or need permission. An **approval** is a request for you to review a proposed action. **Ask** can pause for that review; **Full access** grants broader execution. Tools can have additional permissions of their own.

## Context and tokens

**Context** is the information supplied to a model: your messages and relevant history, attachments, or tool results. **Tokens** are the small pieces of text/data a provider counts for model input/output. They are not the same as words. Providers may use them for pricing and limits.

## Connector and MCP

A **connector** adds tools from another program or service. **MCP** is the protocol those tool servers use to describe and run their tools. You can start without adding a custom connector. [Connectors](connectors.md).

## Checkpoint and rewind

A **checkpoint** is a saved state of eligible project files. **Rewind** restores a selected earlier state. It has exclusions and cannot undo actions on websites or in other apps. [Checkpoints](checkpoints.md).

## Common file and development terms

| Term | Meaning |
| --- | --- |
| Markdown / `.md` | A text format using simple headings, lists, and links. |
| Path | A file or folder's location, such as `outputs/summary.md`. |
| Terminal / PowerShell | An app where you type commands to your computer. Commands are different from AI chat messages. |
| Source build | Making the app from its code using development tools, rather than installing a ready-made download. |
| Dependency / runtime | Extra software a feature needs to run; taucode prepares some runtimes on demand. |
| Prerelease / test build | An early version for testing, with known limits and no stable-release guarantee. |
| Checksum / SHA-256 | A value you compare to check that a downloaded file matches the published copy; it does not prove the program is safe. |
| Signed / notarized | Different software-distribution checks. The current macOS test package uses a development signature and is not notarized. |
| Git repository | A project whose change history is managed by Git. |
| Diff | A view showing what changed between versions of a file. |
| Branch / worktree | Git features for separate lines of work and separate checkouts. |
| Archive / Trash | Ways of moving conversations out of the active list; retention and restore rules depend on the mode. |

## Terms in the advanced guides

| Term | Meaning |
| --- | --- |
| UI / interface | The buttons, menus, and other controls you see and interact with. |
| Profile | A set of tools and settings for a particular mode or purpose. |
| Protocol | An agreed message format programs use to communicate. |
| Base URL / endpoint | The network address an app sends API requests to. Prefer a provider's default unless your setup needs a different one. |
| `localhost` | The computer running the service/client in that context. For a remote engine, it means the remote host. |
| Streaming | Displaying a reply as it arrives instead of waiting for the whole answer. |
| Reasoning level | A supported model setting affecting how it works on a request; capabilities and cost vary. |
| Subagent / delegation | Giving part of a task to a child agent. More agents can mean more provider usage. |
| Chain of Command | A Code team mode with ranked roles and review rules. Start with a single model before exploring it. |
| Concurrency | How many tasks or child agents can work at the same time. |
| Daemon | A background program that can run the backend separately from the desktop window. |
| Backend | The part of the app that manages state, engines, and tool operations behind the interface. |
| Native browser / Chromium | Alternative ways to display or automate websites inside taucode. |
| Headless | A browser or process running without its own ordinary desktop window. |
| ACP / pi-RPC / JSON-RPC | Communication protocols used between the app and supported agent programs. |
| stdio / HTTP | Local process communication through input/output streams, or communication with a web endpoint. |
| CDP / WKWebView / WebView2 | Browser-control or browser-embedding technologies mentioned in the technical reference. |
| Environment variable | A named value supplied to a program when it starts, often used for advanced configuration. |
| Keychain | macOS's credential store; it can ask before letting an app read a saved key. |
| Diagnostics | Optional local records that help explain errors and performance. |
| Evals | Repeatable tests for comparing model/engine configurations; real model runs can cost money. |

If an explanation still assumes something you do not know, [report a documentation issue](https://github.com/aiden0rchad/taucode/issues/new/choose) with the page and unclear term.
