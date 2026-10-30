# Architecture & source map

This is a technical map for contributors. If you want to install or use the app, begin with [Start here](start-here.md) or [Your first Work task](first-task.md). The [glossary](glossary.md) explains terms such as backend, daemon, and protocol as you encounter them.

taucode is a Tauri v2 application with a React/TypeScript interface and a Rust host. The desktop adapter and headless daemon share backend domain code. Agent engines and optional helpers run as supervised child processes.

## Runtime at a glance

```text
React UI: Code / Chat / Work
          │
          │ Tauri IPC and app events
          ▼
Rust desktop adapter ───── shared backend cores ───── daemon JSON-RPC adapter
                                 │
                  ┌──────────────┼────────────────┐
                  ▼              ▼                ▼
           Engine adapters   Tool registries   Local state
           pi / OMP          MCP connectors    settings / sessions
           Codex             browser bridge   checkpoints / themes
           ACP agents        computer host    diagnostics / evals
                  │              │
                  ▼              ▼
           Model providers   Local or remote tools
```

The engine runs the agent loop. The provider serves model inference. taucode owns the surrounding workspace, registry, policy, and UI integration. These responsibilities should remain distinct when adding features.

## Repository layout

| Path | Responsibility |
| --- | --- |
| `apps/taucode/src/` | React interface, feature hooks, design system, IPC wrapper, and preview fixtures. |
| `apps/taucode/src-tauri/` | Rust host, app commands, daemon, platform configuration, and native helpers. |
| `apps/taucode/src-tauri/src/shared/` | Shared engine/provider/tool/domain logic used by runtime adapters. |
| `apps/taucode/engines/` | Locked runtime manifests, approval/subagent extensions, Work skills, and computer sidecar. |
| `apps/taucode/scripts/` | Development, packaging, signing, bundling, and verification helpers. |
| `crates/tau-mcp-proxy/` | MCP proxy and code-intelligence support. |
| `crates/taucode-diag-mcp/` | Standalone diagnostics MCP reader. |
| `packages/protocol/` | Generated Codex-shaped TypeScript protocol types. |
| `docs/` | User guides, reference pages, and engineering records. |
| `.github/workflows/` | Documentation deployment and Windows build automation. |

Some internal names remain `codex-monitor`, including the Cargo/npm package name and daemon binary names. Those are existing implementation identifiers; the application/repository is taucode.

## Frontend boundaries

`src/App.tsx` composes the main application, mode shell, diagnostic boundary, and native approval/window surfaces. ModeShell chooses Code, Chat, or Work and coordinates settings and the setup checklist.

| Path | Role |
| --- | --- |
| `src/features/app/` | App bootstrap, orchestration, and the Code shell. |
| `src/features/chat/` | Mode switching and restricted Chat UI. |
| `src/features/work/` | Spaces, tasks, and Work UI. |
| `src/features/session/` | Session context/rail presentation. |
| `src/features/threads/` | Thread lifecycle, messaging, and reducers. |
| `src/features/settings/` | Providers, engines, agents, connectors, and feature controls. |
| `src/features/design-system/` | Shared shells, primitives, and tokens. |
| `src/dev/uiPreview/` | Browser-only fixtures and mocked native transport. |
| `src/services/tauri.ts` | Frontend IPC contract wrapper. |
| `src/services/events.ts` | Event subscription fanout. |

Keep backend calls out of presentational components. When a transport shape changes, update its wrapper, callers, and backend handlers together.

## Desktop and daemon parity

The desktop command registry lives in `src-tauri/src/lib.rs`. The daemon entry point is `src-tauri/src/bin/codex_monitor_daemon.rs`, with RPC routing in its companion directory.

Shared behavior belongs in `src-tauri/src/shared/`, with both adapters delegating to it. Copying logic into the desktop command and daemon handler creates diverging behavior and makes remote support harder to verify.

A cross-runtime change normally touches:

1. Shared core behavior.
2. Desktop command adapter.
3. Frontend IPC wrapper when needed.
4. Daemon RPC surface.
5. Contract and behavior tests.

Native-only features can have explicit platform/runtime limitations. For example, macOS native browser control needs a desktop host; a daemon-only connection uses Chromium.

## Engine and provider flow

The provider registry stores service metadata, enabled models, capabilities, and auth references. The engine registry tracks built-in/custom runtime definitions and readiness.

Engine adapters translate their protocol into the app's canonical event representation. That lets the UI render messages, plans, tool calls, approvals, and child activity consistently without making the UI itself an agent loop.

Chat and Work are dedicated pi profiles with different tools and policies. A Code engine default is not a universal mode switch for every profile.

## Policy and trust boundaries

- Providers receive inference context through the chosen engine path.
- Stored secrets are supplied through managed credential/environment plumbing.
- Connectors have tool filters and their own approval rules.
- Computer input is enforced by the Rust host; the planner returns proposals.
- Browser content is separated from the app's privileged webview command surface.
- Custom executables remain trusted programs and must honor their permission protocol.

These boundaries have tests and implementation-specific checks. They do not amount to a universal sandbox or an independent security audit. See [privacy & trust](privacy.md).

## Persistence and recovery

Settings, engine/session state, checkpoints, themes, diagnostics, and evaluations have separate local stores. Workspace files remain in the selected project/space folder.

Checkpoints use a shadow Git repository independent of the user's repository metadata. Engine session persistence retains available output; the verified pi/OMP recovery path stops unfinished owned commands rather than replaying them automatically.

See [checkpoints](checkpoints.md), [diagnostics](diagnostics.md), and [Evals](evals.md) for data-specific limits.

## More detailed maps

The [app codebase map](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/docs/codebase-map.md) maps tasks to source areas. The [app agent guide](https://github.com/aiden0rchad/taucode/blob/main/apps/taucode/AGENTS.md) defines implementation rules. [Engineering references](engineering.md) separates current acceptance evidence from older design notes.
