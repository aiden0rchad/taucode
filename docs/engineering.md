# Engineering references

The user guides describe the current documented behavior. The repository also retains original design specifications, upstream navigation notes, fixtures, and dated verification records.

These engineering files are available on GitHub and are intentionally separate from the published user-guide navigation. A proposal, checklist, or dated instruction is not proof that a feature is currently shipped or supported on every platform.

## Start with current acceptance

| Reference | Use it for |
| --- | --- |
| [Dogfood acceptance](https://github.com/aiden0rchad/taucode/blob/main/docs/dogfood-acceptance.md) | October 30, 2026 integration evidence, checked behaviors, and remaining limitations. |
| [App agent guide](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/AGENTS.md) | Backend routing, frontend boundaries, invariants, and validation rules. |
| [Codebase map](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/docs/codebase-map.md) | Task-oriented source navigation. |
| [UI preview harness](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/src/dev/uiPreview/README.md) | Credential-free fixture controls and screenshot recipes. |
| [App-server events](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/docs/app-server-events.md) | Event and integration notes. |

## Feature design records

| Area | Original reference |
| --- | --- |
| Provider registry and engine-neutral core | [M1-C](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m1-c-engine-neutral-core.md) |
| pi runtime and policy gate | [M1-D1](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m1-d1-pi-engine.md), [M1-D2](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m1-d2-pi-gate-and-subagents.md) |
| MCP connectors | [M1-E](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m1-e-mcp-connectors.md) |
| Computer use | [M3](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m3-computer-use.md) and its follow-up records |
| Chat | [M4-D](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m4d-simple-chat.md) |
| Work | [M4-F](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m4f-work-mode.md) |
| Checkpoints | [M4-B](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m4b-checkpoints-rewind.md) |
| Native browser | [M7](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m7-native-browser.md) |
| Chain of Command | [M9](https://github.com/aiden0rchad/taucode/blob/main/docs/specs/m9-chain-of-command.md) |
| Complete specification archive | [docs/specs](https://github.com/aiden0rchad/taucode/tree/main/docs/specs) |

Older records can contain local development paths, superseded options, and original implementation instructions. Prefer the live source, current guides, and more recent acceptance evidence when they disagree.

## Protocol and support crates

- [Protocol package](https://github.com/aiden0rchad/taucode/tree/main/packages/protocol): generated Codex app-server types pinned to 0.164.0.
- [MCP proxy](https://github.com/aiden0rchad/taucode/tree/main/crates/tau-mcp-proxy): proxy and code-intelligence support.
- [Diagnostics MCP reader](https://github.com/aiden0rchad/taucode/tree/main/crates/taucode-diag-mcp): standalone journal inspection.
- [Fixtures](https://github.com/aiden0rchad/taucode/tree/main/docs/fixtures): bounded test examples and redaction inputs.

## Working with upstream

taucode is derived from CodexMonitor. The [upstream sync runbook](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/docs/multi-agent-sync-runbook.md) records the inherited multi-agent/config synchronization workflow. Account/login assumptions and experimental feature defaults in upstream documents may differ from taucode's own provider registry and profiles.

Use [development](development.md) and [architecture](architecture.md) for the current contribution path. Preserve upstream copyright notices and distinguish imported design decisions from verified taucode behavior.
